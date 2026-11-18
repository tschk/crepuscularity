use crepuscularity_core::TemplateContext;
use crepuscularity_web::{render_from_files, render_template_to_html};
use std::{collections::HashMap, fs};

fn renders(
    files: &HashMap<String, String>,
    entry: &str,
) -> Vec<Result<String, crepuscularity_web::CrepusError>> {
    let ctx = TemplateContext::new();
    #[allow(unused_mut)]
    let mut out = vec![render_from_files(files, entry, &ctx)];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        out.push(crepuscularity_web::render_from_files_with_ssr(
            files, entry, &ctx, markers,
        ));
    }
    out
}

#[test]
fn missing_virtual_includes_never_read_neighboring_disk_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("missing.crepus"), "span \"DISK DECOY\"").unwrap();
    fs::write(
        dir.path().join("named.crepus"),
        "--- Card\nspan \"NAMED DISK DECOY\"",
    )
    .unwrap();
    let entry = dir
        .path()
        .join("entry.crepus")
        .to_string_lossy()
        .into_owned();
    let shell = dir
        .path()
        .join("shell.crepus")
        .to_string_lossy()
        .into_owned();
    for body in [
        "include missing.crepus",
        "include named.crepus#Card",
        "include shell.crepus\n  include missing.crepus",
    ] {
        for named_entry in [false, true] {
            let source = if named_entry {
                format!("--- Root\n{body}")
            } else {
                body.into()
            };
            let files = HashMap::from([(entry.clone(), source), (shell.clone(), "slot".into())]);
            let selected = if named_entry {
                format!("{entry}#Root")
            } else {
                entry.clone()
            };
            for result in renders(&files, &selected) {
                assert!(result.is_err(), "bundle read disk: {body}: {result:?}");
            }
        }
    }
}

#[test]
fn virtual_content_wins_and_normal_template_disk_fallback_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let disk = dir.path().join("part.crepus");
    fs::write(&disk, "span \"DISK VALUE\"").unwrap();
    let entry = dir
        .path()
        .join("entry.crepus")
        .to_string_lossy()
        .into_owned();
    let files = HashMap::from([
        (entry.clone(), "include part.crepus".into()),
        (
            disk.to_string_lossy().into_owned(),
            "span \"VIRTUAL VALUE\"".into(),
        ),
    ]);
    for result in renders(&files, &entry) {
        let html = result.unwrap();
        assert!(
            html.contains("VIRTUAL VALUE") && !html.contains("DISK VALUE"),
            "{html}"
        );
    }
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let html = render_template_to_html("include part.crepus", &ctx).unwrap();
    assert!(
        html.contains("DISK VALUE"),
        "normal disk fallback changed: {html}"
    );
    // A failed virtual-only render must not change a subsequent ordinary call.
    let files = HashMap::from([(entry.clone(), "include part.crepus".into())]);
    for result in renders(&files, &entry) {
        assert!(result.is_err());
    }
    assert!(render_template_to_html("include part.crepus", &ctx)
        .unwrap()
        .contains("DISK VALUE"));
}

#[test]
fn textarea_child_walkers_and_slots_keep_virtual_only_policy() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("missing.crepus"), "span \"DISK TEXTAREA\"").unwrap();
    let entry = dir
        .path()
        .join("entry.crepus")
        .to_string_lossy()
        .into_owned();
    let shell = dir
        .path()
        .join("shell.crepus")
        .to_string_lossy()
        .into_owned();
    for body in [
        "textarea\n  include missing.crepus",
        "include shell.crepus\n  include missing.crepus",
    ] {
        let files = HashMap::from([
            (entry.clone(), body.into()),
            (shell.clone(), "textarea\n  slot".into()),
        ]);
        for result in renders(&files, &entry) {
            assert!(result.is_err(), "textarea escaped policy: {result:?}");
        }
    }
}

#[test]
fn virtual_include_paths_still_reject_syntactic_escapes() {
    let files = HashMap::from([
        ("entry.crepus".into(), "include ../outside.crepus".into()),
        ("outside.crepus".into(), "span \"outside\"".into()),
    ]);
    for result in renders(&files, "entry.crepus") {
        assert!(result.unwrap_err().to_string().contains("outside base dir"));
    }
}

#[cfg(unix)]
#[test]
fn disk_symlinks_do_not_redirect_or_reject_virtual_bundle_keys() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let outside = external.path().join("outside.crepus");
    fs::write(&outside, "span \"DISK OUTSIDE\"").unwrap();
    let part = dir.path().join("part.crepus");
    std::os::unix::fs::symlink(&outside, &part).unwrap();
    let entry = dir
        .path()
        .join("entry.crepus")
        .to_string_lossy()
        .into_owned();
    let files = HashMap::from([
        (entry.clone(), "include part.crepus".into()),
        (
            part.to_string_lossy().into_owned(),
            "span \"VIRTUAL INSIDE\"".into(),
        ),
    ]);
    for result in renders(&files, &entry) {
        let html = result.unwrap();
        assert!(
            html.contains("VIRTUAL INSIDE") && !html.contains("DISK OUTSIDE"),
            "{html}"
        );
    }
}
