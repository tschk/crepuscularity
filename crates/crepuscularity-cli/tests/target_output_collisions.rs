//! Actual CLI manifest builds: collision admission happens before any target writes.
use std::path::Path;
use std::process::{Command, Output};

fn build(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["build", "--all", "--manifest"])
        .arg(root.join("crepus.toml"))
        .current_dir(root)
        .output()
        .unwrap()
}

fn fixture(root: &Path, second_output: &str) {
    std::fs::write(root.join("entry.crepus"), "span \"Fixture\"").unwrap();
    std::fs::write(
        root.join("crepus.toml"),
        format!(
            r#"[[targets]]
type = "native"
id = "early"
entry = "entry.crepus"
out = "early.json"
[[targets]]
type = "native"
id = "first"
entry = "entry.crepus"
out = "shared.json"
[[targets]]
type = "native"
id = "second"
entry = "entry.crepus"
out = "{second_output}"
"#
        ),
    )
    .unwrap();
    std::fs::write(root.join("shared.json"), "existing sentinel").unwrap();
}

fn unchanged(root: &Path) {
    assert_eq!(
        std::fs::read_to_string(root.join("shared.json")).unwrap(),
        "existing sentinel"
    );
    assert!(
        !root.join("early.json").exists(),
        "an earlier noncolliding target wrote before preflight"
    );
}

#[test]
fn all_targets_reject_file_ancestors_before_writing_any_target() {
    for (first, second) in [
        ("new/result", "new/result/view.json"),
        ("new/result/view.json", "new/result"),
    ] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), second);
        let manifest = root.path().join("crepus.toml");
        let source = std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("out = \"shared.json\"", &format!("out = \"{first}\""));
        std::fs::write(manifest, source).unwrap();
        let output = build(root.path());
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("collision") && error.contains("first") && error.contains("second"),
            "{error}"
        );
        unchanged(root.path());
        assert!(!root.path().join("new").exists());
    }
}

#[test]
fn all_targets_reject_duplicate_outputs_before_writing_any_target() {
    for alias in ["shared.json", "missing/../shared.json"] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), alias);
        let output = build(root.path());
        assert!(!output.status.success(), "{alias}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("collision"), "{error}");
        unchanged(root.path());
        assert!(!root.path().join("missing").exists());
    }
}

#[cfg(unix)]
#[test]
fn all_targets_reject_symlink_parent_aliases_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(root.path(), root.path().join("alias")).unwrap();
    fixture(root.path(), "alias/shared.json");
    let output = build(root.path());
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("collision"), "{error}");
    unchanged(root.path());
}

#[cfg(unix)]
#[test]
fn output_symlink_can_create_its_missing_file_target() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("future.json", root.path().join("link.json")).unwrap();
    std::fs::write(root.path().join("entry.crepus"), "span \"Created\"").unwrap();
    std::fs::write(
        root.path().join("crepus.toml"),
        "[[targets]]\ntype=\"native\"\nid=\"linked\"\nentry=\"entry.crepus\"\nout=\"link.json\"\n",
    )
    .unwrap();
    let output = build(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_to_string(root.path().join("future.json"))
        .unwrap()
        .contains("Created"));
    assert!(std::fs::symlink_metadata(root.path().join("link.json"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn all_targets_allow_distinct_native_and_lvgl_files_in_one_directory() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("entry.crepus"), "span \"Fixture\"").unwrap();
    std::fs::write(
        root.path().join("crepus.toml"),
        r#"[[targets]]
type = "native"
id = "native"
entry = "entry.crepus"
out = "shared/view.json"
[[targets]]
type = "lvgl"
id = "lvgl"
entry = "entry.crepus"
out = "shared/view.xml"
"#,
    )
    .unwrap();
    let output = build(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.path().join("shared/view.json").is_file());
    assert!(root.path().join("shared/view.xml").is_file());
}
