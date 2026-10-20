use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Output},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("crepus.toml"), "[[targets]]\ntype = 'webext'\nid = 'extension'\napp = '.'\nbrowsers = ['chromium']\n[targets.extension]\nname = 'Output test'\nversion = '1.0.0'\n").unwrap();
    fs::create_dir_all(dir.path().join("pages")).unwrap();
    fs::write(dir.path().join("pages/popup.crepus"), "span \"Popup\"").unwrap();
    dir
}
fn build(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(root)
        .args(["webext", "build", "--browser", "chromium"])
        .output()
        .unwrap()
}
fn ok(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        if !at.exists() {
            return;
        }
        for entry in fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
#[test]
fn repeat_build_is_clean_and_removed_sources_do_not_leave_outputs() {
    let dir = fixture();
    let root = dir.path();
    for folder in ["src", "resources", "icons", "dist/firefox"] {
        fs::create_dir_all(root.join(folder)).unwrap();
    }
    fs::write(
        root.join("src/content.css.crepus"),
        "div\n<style>\n.unique-output-test{color:#123}\n</style>",
    )
    .unwrap();
    fs::write(root.join("pages/old.crepus"), "span \"Old\"").unwrap();
    fs::write(root.join("src/old.js"), "old()").unwrap();
    fs::write(root.join("resources/old.txt"), "resource").unwrap();
    fs::write(root.join("icons/icon16.png"), b"icon").unwrap();
    fs::write(root.join("dist/firefox/sentinel"), "other browser").unwrap();
    ok(build(root));
    let dist = root.join("dist/chromium");
    let first = snapshot(&dist);
    ok(build(root));
    assert_eq!(snapshot(&dist), first);
    let css = fs::read_to_string(dist.join("src/content.css")).unwrap();
    assert_eq!(css.matches(".unique-output-test").count(), 1);
    fs::rename(root.join("pages/old.crepus"), root.join("pages/new.crepus")).unwrap();
    fs::remove_file(root.join("src/old.js")).unwrap();
    fs::remove_file(root.join("resources/old.txt")).unwrap();
    fs::remove_dir_all(root.join("icons")).unwrap();
    ok(build(root));
    for path in ["pages/old.html", "src/old.js", "resources/old.txt", "icons"] {
        assert!(!dist.join(path).exists(), "stale {path}");
    }
    assert!(dist.join("pages/new.html").exists());
    assert_eq!(
        fs::read_to_string(root.join("dist/firefox/sentinel")).unwrap(),
        "other browser"
    );
    assert!(root.join("src/content.css.crepus").exists());
}
#[test]
fn invalid_page_keeps_entire_previous_output() {
    let dir = fixture();
    ok(build(dir.path()));
    let dist = dir.path().join("dist/chromium");
    let before = snapshot(&dist);
    fs::write(
        dir.path().join("pages/broken.crepus"),
        "include missing.crepus",
    )
    .unwrap();
    let output = build(dir.path());
    assert!(!output.status.success());
    assert_eq!(snapshot(&dist), before);
}
#[test]
fn invalid_authored_popup_keeps_entire_previous_output() {
    let dir = fixture();
    fs::create_dir_all(dir.path().join("views")).unwrap();
    let popup = dir.path().join("views/popup.crepus");
    fs::write(&popup, "span \"Original popup\"").unwrap();
    ok(build(dir.path()));
    let dist = dir.path().join("dist/chromium");
    let before = snapshot(&dist);
    fs::write(&popup, "include missing-popup-proof.crepus").unwrap();
    let output = build(dir.path());
    assert!(
        !output.status.success(),
        "invalid authored popup was silently replaced"
    );
    assert_eq!(snapshot(&dist), before);
}
#[test]
fn missing_explicit_manifest_assets_keep_previous_output() {
    for extra in [
        "[targets.options]\naction_popup = 'pages/missing.crepus'\n",
        "[targets.options.icons]\n16 = 'icons/missing.png'\n",
    ] {
        let dir = fixture();
        ok(build(dir.path()));
        let dist = dir.path().join("dist/chromium");
        let before = snapshot(&dist);
        let config = dir.path().join("crepus.toml");
        let mut text = fs::read_to_string(&config).unwrap();
        text.push_str(extra);
        fs::write(config, text).unwrap();
        let output = build(dir.path());
        assert!(!output.status.success(), "missing asset accepted: {extra}");
        assert_eq!(snapshot(&dist), before);
    }
}
#[cfg(unix)]
#[test]
fn symlink_output_is_rejected_without_touching_external_directory() {
    let dir = fixture();
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("sentinel"), "external").unwrap();
    fs::create_dir_all(dir.path().join("dist")).unwrap();
    std::os::unix::fs::symlink(external.path(), dir.path().join("dist/chromium")).unwrap();
    let before = snapshot(external.path());
    let output = build(dir.path());
    assert!(!output.status.success());
    assert_eq!(snapshot(external.path()), before);
    assert!(fs::symlink_metadata(dir.path().join("dist/chromium"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
#[cfg(target_os = "macos")]
fn safari_configuration_failures_preserve_previous_output() {
    for case in [
        "overlapping project",
        "missing config",
        "existing project",
        "invalid platforms",
    ] {
        let existing_project = case == "existing project";
        let dir = fixture();
        let root = dir.path();
        let manifest = root.join("crepus.toml");
        if case != "missing config" {
            let source = fs::read_to_string(&manifest).unwrap();
            fs::write(&manifest, format!("{source}\n[targets.safari]\nbundle_identifier = 'dev.crepus.output-test'\nproject_location = 'dist/safari-app'\n")).unwrap();
            if case == "overlapping project" {
                let source = fs::read_to_string(&manifest).unwrap();
                fs::write(
                    &manifest,
                    source.replace("dist/safari-app", "dist/safari/generated-app"),
                )
                .unwrap();
            }
            if case == "invalid platforms" {
                let source = fs::read_to_string(&manifest).unwrap();
                fs::write(&manifest, format!("{source}platforms = ['windows']\n")).unwrap();
            }
        }
        if existing_project {
            fs::create_dir_all(root.join("dist/safari-app")).unwrap();
            fs::write(root.join("dist/safari-app/owned.txt"), "existing project").unwrap();
        }
        let dist = root.join("dist/safari");
        fs::create_dir_all(dist.join("nested")).unwrap();
        fs::write(dist.join("manifest.json"), "previous manifest").unwrap();
        fs::write(dist.join("nested/sentinel"), "previous output").unwrap();
        let before = snapshot(&root.join("dist"));
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .current_dir(root)
            .args(["webext", "build", "--browser", "safari"])
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "Safari failure accepted");
        let expected = match case {
            "existing project" => "already exists",
            "invalid platforms" => "platforms must be",
            "overlapping project" => "overlap",
            _ => "Safari builds need",
        };
        assert!(error.contains(expected), "{case}: {error}");
        assert_eq!(snapshot(&root.join("dist")), before, "{case}");
        assert!(
            fs::read_dir(root.join("dist")).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".crepus-")
            }),
            "Safari failure leaked staging output"
        );
    }
}

#[test]
fn extension_pages_include_supported_frontends_without_emitting_dependency_pages() {
    let dir = fixture();
    let root = dir.path();
    fs::create_dir_all(root.join("pages/parts/nested")).unwrap();
    let frontends = [
        ("card.vue", "<template><p>VUE INCLUDED</p></template>"),
        ("card.svelte", "<p>SVELTE INCLUDED</p>"),
        ("card.astro", "<p>ASTRO INCLUDED</p>"),
        ("card.ng", "<p>ANGULAR INCLUDED</p>"),
        ("card.jsx", "<p>JSX INCLUDED</p>"),
    ];
    for (filename, content) in frontends {
        fs::write(root.join("pages/parts/nested").join(filename), content).unwrap();
    }
    fs::write(
        root.join("pages/parts/section.crepus"),
        frontends
            .iter()
            .map(|(filename, _)| format!("include nested/{filename}\n"))
            .collect::<String>(),
    )
    .unwrap();
    fs::write(
        root.join("pages/options.crepus"),
        "include parts/section.crepus",
    )
    .unwrap();
    fs::write(
        root.join("pages/static.html"),
        "<!doctype html><p>STATIC ASSET</p>",
    )
    .unwrap();
    fs::write(root.join("pages/legacy.html"), b"<p>caf\xe9</p>").unwrap();
    fs::write(root.join("pages/opaque.js"), [0xff, 0x00, 0xc3, 0x28]).unwrap();
    ok(build(root));
    let dist = root.join("dist/chromium");
    let html = fs::read_to_string(dist.join("pages/options.html")).unwrap();
    for marker in [
        "VUE INCLUDED",
        "SVELTE INCLUDED",
        "ASTRO INCLUDED",
        "ANGULAR INCLUDED",
        "JSX INCLUDED",
    ] {
        assert!(html.contains(marker), "missing {marker}: {html}");
    }
    assert!(dist.join("src/__crepus_pages/options.js").is_file());
    assert_eq!(
        fs::read_to_string(dist.join("pages/static.html")).unwrap(),
        "<!doctype html><p>STATIC ASSET</p>"
    );
    assert_eq!(
        fs::read(dist.join("pages/opaque.js")).unwrap(),
        [0xff, 0x00, 0xc3, 0x28]
    );
    assert_eq!(
        fs::read(dist.join("pages/legacy.html")).unwrap(),
        b"<p>caf\xe9</p>"
    );
    assert!(!dist.join("src/__crepus_pages/legacy.js").exists());
    assert!(!dist.join("src/__crepus_pages/static.js").exists());
    assert!(!dist.join("src/__crepus_pages/opaque.js").exists());
    for (filename, _) in frontends {
        let html = dist
            .join("pages/parts/nested")
            .join(filename)
            .with_extension("html");
        assert!(
            !html.exists(),
            "template dependency became a page: {}",
            html.display()
        );
        let script = dist
            .join("src/__crepus_pages/parts/nested")
            .join(filename)
            .with_extension("js");
        assert!(
            !script.exists(),
            "template dependency became a client: {}",
            script.display()
        );
    }
    let previous = snapshot(&dist);
    fs::write(root.join("pages/options.crepus"), "include legacy.html").unwrap();
    let rejected = build(root);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("legacy.html"));
    assert_eq!(snapshot(&dist), previous, "failed include replaced output");
}

#[test]
fn authored_popup_resolves_nested_frontends_for_all_prerendered_states() {
    let dir = fixture();
    let root = dir.path();
    fs::create_dir_all(root.join("views/parts/nested")).unwrap();
    fs::write(
        root.join("views/popup.crepus"),
        "include parts/panel.crepus show_help={show_help} show_crepus={show_crepus}",
    )
    .unwrap();
    fs::write(root.join("views/parts/panel.crepus"), "if show_help\n  p \"HELP STATE\"\nelse\n  if show_crepus\n    p \"CREPUS STATE\"\n  else\n    p \"MAIN STATE\"\ninclude nested/footer.crepus").unwrap();
    fs::write(
        root.join("views/parts/nested/footer.crepus"),
        "include footer.vue",
    )
    .unwrap();
    let frontend = root.join("views/parts/nested/footer.vue");
    fs::write(&frontend, "<template><p>VUE FOOTER</p></template>").unwrap();
    ok(build(root));
    let dist = root.join("dist/chromium");
    let html = fs::read_to_string(dist.join("src/popup.html")).unwrap();
    let main = html
        .split_once("<div id=\"view-main\">")
        .unwrap()
        .1
        .split_once("<div id=\"view-help\"")
        .unwrap()
        .0;
    let help = html
        .split_once("<div id=\"view-help\" hidden>")
        .unwrap()
        .1
        .split_once("<div id=\"view-crepus\"")
        .unwrap()
        .0;
    let crepus = html
        .split_once("<div id=\"view-crepus\" hidden>")
        .unwrap()
        .1
        .split_once("<script type=\"module\"")
        .unwrap()
        .0;
    for (body, expected) in [
        (main, "MAIN STATE"),
        (help, "HELP STATE"),
        (crepus, "CREPUS STATE"),
    ] {
        assert!(body.contains(expected), "{body}");
        assert!(body.contains("VUE FOOTER"), "{body}");
        for other in ["MAIN STATE", "HELP STATE", "CREPUS STATE"] {
            assert_eq!(body.contains(other), other == expected, "{body}");
        }
    }
    let previous = snapshot(&dist);
    fs::write(frontend, "<template><p></template>").unwrap();
    let failed = build(root);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("popup pre-render failed"));
    assert_eq!(snapshot(&dist), previous);
}

#[test]
fn authored_popup_title_escapes_manifest_display_name() {
    let dir = fixture();
    fs::create_dir(dir.path().join("views")).unwrap();
    fs::write(dir.path().join("views/popup.crepus"), "p \"Popup\"").unwrap();
    let config = dir.path().join("crepus.toml");
    let name = "</title><script>bad()</script> & \"Name\"";
    let manifest = fs::read_to_string(&config).unwrap().replace(
        "name = 'Output test'",
        &format!("name = {}", toml::Value::String(name.into())),
    );
    fs::write(config, manifest).unwrap();
    ok(build(dir.path()));
    let html = fs::read_to_string(dir.path().join("dist/chromium/src/popup.html")).unwrap();
    assert!(html.contains("<title>&lt;/title&gt;&lt;script&gt;bad()&lt;/script&gt; &amp; &quot;Name&quot;</title>"), "{html}");
    assert!(!html.contains("<script>bad()</script>"));
}
