use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn build(site: &Path, target: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .output()
        .unwrap()
}

fn unsupported() -> &'static str {
    "input disabled"
}

/// Include directories as well as byte contents, so newly created empty folders
/// also count as an observable change after a failed compilation.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let key = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                result.insert(key, None);
                visit(root, &path, result);
            } else {
                result.insert(key, Some(fs::read(path).unwrap()));
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn unsupported_targets_fail_before_creating_a_fresh_output_directory() {
    for target in ["imba", "moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("index.crepus"), unsupported()).unwrap();
        let result = build(site.path(), target);
        assert!(!result.status.success(), "{target} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("unsupported View IR control state"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            !site.path().join("dist").exists(),
            "{target} left partial output: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn emission_errors_preserve_all_existing_artifacts_and_directory_entries() {
    for target in ["imba", "moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("index.crepus"), unsupported()).unwrap();
        let dist = site.path().join("dist");
        fs::create_dir_all(dist.join("preserved-empty-directory")).unwrap();
        for (filename, bytes) in [
            ("crepus-view-ir.json", b"previous IR\0\xff".as_slice()),
            ("CrepusEmit.imba", b"previous Imba".as_slice()),
            ("CrepusEmit.bindings.ts", b"previous bindings".as_slice()),
            ("CrepusEmit.moonshine-dom.ts", b"previous DOM".as_slice()),
            ("CrepusEmit.moonshine-ssr.ts", b"previous SSR".as_slice()),
            (
                "crepus-module-manifest.json",
                b"previous manifest".as_slice(),
            ),
        ] {
            fs::write(dist.join(filename), bytes).unwrap();
        }
        let before = snapshot(&dist);
        let result = build(site.path(), target);
        assert!(!result.status.success(), "{target} unexpectedly succeeded");
        assert_eq!(snapshot(&dist), before, "{target} changed prior artifacts");
    }
}

#[test]
fn successful_imba_build_publishes_all_files_and_a_failed_rebuild_preserves_them() {
    let site = tempfile::tempdir().unwrap();
    let source = site.path().join("index.crepus");
    fs::write(&source, "text \"Published before failure\"").unwrap();
    let result = build(site.path(), "imba");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let dist = site.path().join("dist");
    for filename in [
        "crepus-view-ir.json",
        "CrepusEmit.imba",
        "CrepusEmit.bindings.ts",
    ] {
        assert!(dist.join(filename).is_file(), "missing {filename}");
    }
    assert!(fs::read_to_string(dist.join("CrepusEmit.imba"))
        .unwrap()
        .contains("Published before failure"));
    let before = snapshot(&dist);
    fs::write(&source, unsupported()).unwrap();
    let result = build(site.path(), "imba");
    assert!(!result.status.success());
    assert_eq!(
        snapshot(&dist),
        before,
        "failed rebuild replaced working artifacts"
    );
}

#[test]
fn existing_component_directories_fail_before_any_ir_or_artifact_writes() {
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
        for existing_ir in [false, true] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("index.crepus"), "text \"Static\"").unwrap();
            let dist = site.path().join("dist");
            fs::create_dir_all(dist.join(component)).unwrap();
            fs::write(
                dist.join(component).join("child-sentinel"),
                b"preserved\0\xff",
            )
            .unwrap();
            fs::write(dist.join("unrelated.txt"), "unrelated").unwrap();
            if existing_ir {
                fs::write(dist.join("crepus-view-ir.json"), "previous IR").unwrap();
            }
            let before = snapshot(&dist);
            let result = build(site.path(), target);
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(
                !result.status.success(),
                "directory output accepted: {target}"
            );
            assert!(diagnostic.contains("existing directory"), "{diagnostic}");
            assert_eq!(
                snapshot(&dist),
                before,
                "{target}, existing IR: {existing_ir}"
            );
        }
    }
}

#[test]
fn existing_imba_bindings_directory_preserves_component_and_ir() {
    let site = tempfile::tempdir().unwrap();
    fs::write(site.path().join("index.crepus"), "text \"Static\"").unwrap();
    let dist = site.path().join("dist");
    fs::create_dir_all(dist.join("CrepusEmit.bindings.ts")).unwrap();
    fs::write(
        dist.join("CrepusEmit.bindings.ts/child-sentinel"),
        "preserved",
    )
    .unwrap();
    fs::write(dist.join("CrepusEmit.imba"), "previous component").unwrap();
    fs::write(dist.join("crepus-view-ir.json"), "previous IR").unwrap();
    let before = snapshot(&dist);
    let result = build(site.path(), "imba");
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(diagnostic.contains("existing directory"), "{diagnostic}");
    assert_eq!(snapshot(&dist), before);
}

#[cfg(unix)]
#[test]
fn socket_outputs_fail_preflight_without_changing_ir_or_components() {
    use std::os::unix::{fs::FileTypeExt, net::UnixListener};

    for (target, blocked) in [
        ("react", "CrepusEmit.react.tsx"),
        ("imba", "CrepusEmit.bindings.ts"),
    ] {
        let site = tempfile::Builder::new()
            .prefix("s")
            .tempdir_in("/tmp")
            .unwrap();
        fs::write(site.path().join("index.crepus"), "text \"Static\"").unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        fs::write(dist.join("crepus-view-ir.json"), "previous IR").unwrap();
        fs::write(dist.join("unrelated.txt"), "unrelated").unwrap();
        if target == "imba" {
            fs::write(dist.join("CrepusEmit.imba"), "previous component").unwrap();
        }
        let socket = dist.join(blocked);
        let _listener = UnixListener::bind(&socket).unwrap();
        let count = fs::read_dir(&dist).unwrap().count();
        let result = build(site.path(), target);
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success());
        assert!(diagnostic.contains("not a regular file"), "{diagnostic}");
        assert!(fs::symlink_metadata(&socket)
            .unwrap()
            .file_type()
            .is_socket());
        assert_eq!(
            fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap(),
            "previous IR"
        );
        assert_eq!(
            fs::read_to_string(dist.join("unrelated.txt")).unwrap(),
            "unrelated"
        );
        if target == "imba" {
            assert_eq!(
                fs::read_to_string(dist.join("CrepusEmit.imba")).unwrap(),
                "previous component"
            );
        }
        assert_eq!(fs::read_dir(&dist).unwrap().count(), count);
    }
}

#[test]
fn unsupported_form_modes_preserves_outputs_for_all_ir_emitters() {
    for target in [
        "react",
        "svelte",
        "vue",
        "solid",
        "imba",
        "moonshine",
        "moonshine-dom",
        "moonshine-ssr",
    ] {
        for (source, diagnostic) in [
            (
                "button type=submit label=Action",
                "unsupported View IR button type",
            ),
            (
                "button type=reset label=Action",
                "unsupported View IR button type",
            ),
            (
                "select multiple\n  option value=one \"One\"",
                "multiple selection",
            ),
            (
                "select\n  optgroup label=Group\n    option value=one \"One\"",
                "unsupported View IR picker child",
            ),
            (
                "select\n  option value=one selected=true \"One\"",
                "selected defaults",
            ),
        ] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("index.crepus"), source).unwrap();
            let dist = site.path().join("dist");
            let fresh = build(site.path(), target);
            assert!(!fresh.status.success(), "{target}/{source} succeeded");
            assert!(
                String::from_utf8_lossy(&fresh.stderr).contains(diagnostic),
                "{target}/{source}: {}",
                String::from_utf8_lossy(&fresh.stderr)
            );
            assert!(!dist.exists(), "{target}/{source} created partial output");

            fs::create_dir_all(dist.join("preserved-empty-directory")).unwrap();
            fs::write(dist.join("crepus-view-ir.json"), b"previous IR\0\xff").unwrap();
            fs::write(dist.join("unrelated.txt"), "preserved").unwrap();
            let before = snapshot(&dist);
            let rebuild = build(site.path(), target);
            assert!(
                !rebuild.status.success(),
                "{target}/{source} rebuild succeeded"
            );
            assert_eq!(snapshot(&dist), before, "{target}/{source} changed output");
        }
    }
}
