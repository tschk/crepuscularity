use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn build(site: &Path, target: &str, entry: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(site)
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry, "--out-dir"])
        .arg(out)
        .output()
        .unwrap()
}

fn rejected(result: Output) {
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "unexpected success");
    assert!(
        diagnostic.contains("refusing to overwrite reachable template source"),
        "{diagnostic}"
    );
}

#[test]
fn entry_and_reachable_include_destinations_are_preserved() {
    for (target, entry, included, source, out) in [
        ("svelte", "CrepusEmit.svelte", None, "<p>Authored</p>", "."),
        (
            "vue",
            "index.crepus",
            Some("CrepusEmit.vue"),
            "<template><p>Authored</p></template>",
            ".",
        ),
        (
            "vue",
            "index.crepus",
            Some("a/CrepusEmit.vue"),
            "<template><p>Alias</p></template>",
            "a",
        ),
        (
            "imba",
            "index.crepus",
            Some("CrepusEmit.bindings.ts"),
            "text \"Bindings source\"",
            ".",
        ),
        (
            "react",
            "index.crepus",
            Some("crepus-view-ir.json"),
            "text \"IR source\"",
            ".",
        ),
    ] {
        let site = tempfile::tempdir().unwrap();
        let input = site.path().join(included.unwrap_or(entry));
        fs::create_dir_all(input.parent().unwrap()).unwrap();
        fs::write(&input, source).unwrap();
        if let Some(included) = included {
            let requested = if out == "a" {
                "missing/a/CrepusEmit.vue"
            } else {
                included
            };
            fs::write(site.path().join(entry), format!("include {requested}")).unwrap();
        }
        let entry_before = fs::read(site.path().join(entry)).unwrap();
        rejected(build(site.path(), target, entry, &site.path().join(out)));
        assert_eq!(fs::read(&input).unwrap(), source.as_bytes());
        assert_eq!(fs::read(site.path().join(entry)).unwrap(), entry_before);
        if included != Some("crepus-view-ir.json") {
            assert!(!site.path().join(out).join("crepus-view-ir.json").exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinked_output_files_and_directories_cannot_overwrite_sources() {
    use std::os::unix::fs::symlink;
    for directory in [false, true] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir(site.path().join("views")).unwrap();
        let entry = "views/CrepusEmit.svelte";
        let input = site.path().join(entry);
        fs::write(&input, "<p>Keep source</p>").unwrap();
        let out = site.path().join("out");
        if directory {
            symlink(site.path().join("views"), &out).unwrap();
        } else {
            fs::create_dir(&out).unwrap();
            symlink(&input, out.join("CrepusEmit.svelte")).unwrap();
        }
        rejected(build(site.path(), "svelte", entry, &out));
        assert_eq!(fs::read_to_string(&input).unwrap(), "<p>Keep source</p>");
        assert!(!out.join("crepus-view-ir.json").exists());
    }
}

#[test]
fn repeated_root_outputs_allow_unreferenced_generated_templates() {
    let site = tempfile::tempdir().unwrap();
    let source = site.path().join("index.crepus");
    for text in ["First", "Second"] {
        fs::write(&source, format!("text \"{text}\"")).unwrap();
        let result = build(site.path(), "svelte", "index.crepus", site.path());
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(fs::read_to_string(site.path().join("CrepusEmit.svelte"))
            .unwrap()
            .contains(text));
    }
}

#[cfg(unix)]
#[test]
fn generated_component_aliasing_ir_is_rejected_before_any_output_changes() {
    use std::os::unix::fs::symlink;

    for (target, component) in [
        ("react", "CrepusEmit.react.tsx"),
        ("svelte", "CrepusEmit.svelte"),
        ("vue", "CrepusEmit.vue"),
        ("solid", "CrepusEmit.solid.tsx"),
        ("imba", "CrepusEmit.imba"),
        ("moonshine", "crepus-emit.moonshine.tsx"),
        ("moonshine-dom", "CrepusEmit.moonshine-dom.ts"),
        ("moonshine-ssr", "CrepusEmit.moonshine-ssr.ts"),
    ] {
        let site = tempfile::tempdir().unwrap();
        let source = "text \"Static\"";
        fs::write(site.path().join("Entry.crepus"), source).unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        fs::write(dist.join("crepus-view-ir.json"), "previous IR").unwrap();
        fs::write(dist.join("unrelated.txt"), "unrelated").unwrap();
        symlink("crepus-view-ir.json", dist.join(component)).unwrap();
        let before_count = fs::read_dir(&dist).unwrap().count();
        let result = build(site.path(), target, "Entry.crepus", &dist);
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "aliased outputs accepted: {target}"
        );
        assert!(
            diagnostic.contains("resolve to the same destination"),
            "{diagnostic}"
        );
        assert_eq!(
            fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap(),
            "previous IR"
        );
        assert_eq!(
            fs::read_to_string(dist.join(component)).unwrap(),
            "previous IR"
        );
        assert_eq!(
            fs::read_link(dist.join(component)).unwrap(),
            Path::new("crepus-view-ir.json")
        );
        assert_eq!(
            fs::read_to_string(dist.join("unrelated.txt")).unwrap(),
            "unrelated"
        );
        assert_eq!(fs::read_dir(&dist).unwrap().count(), before_count);
        assert_eq!(
            fs::read_to_string(site.path().join("Entry.crepus")).unwrap(),
            source
        );
    }
}

#[cfg(unix)]
#[test]
fn imba_bindings_aliasing_another_planned_output_are_rejected_atomically() {
    use std::os::unix::fs::symlink;

    for target_name in ["crepus-view-ir.json", "CrepusEmit.imba"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.crepus"), "text \"Static\"").unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        fs::write(dist.join("crepus-view-ir.json"), "previous IR").unwrap();
        fs::write(dist.join("CrepusEmit.imba"), "previous component").unwrap();
        symlink(target_name, dist.join("CrepusEmit.bindings.ts")).unwrap();
        let result = build(site.path(), "imba", "Entry.crepus", &dist);
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success(), "aliased Imba bindings accepted");
        assert!(
            diagnostic.contains("resolve to the same destination"),
            "{diagnostic}"
        );
        assert_eq!(
            fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap(),
            "previous IR"
        );
        assert_eq!(
            fs::read_to_string(dist.join("CrepusEmit.imba")).unwrap(),
            "previous component"
        );
        assert_eq!(
            fs::read_link(dist.join("CrepusEmit.bindings.ts")).unwrap(),
            Path::new(target_name)
        );
        assert_eq!(fs::read_dir(&dist).unwrap().count(), 3);
    }
}
