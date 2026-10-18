use std::process::Command;

#[test]
fn invalid_desktop_names_fail_before_creating_files() {
    for name in ["bad name", "1-demo", "bad\"quote", "../outside", "", "app²"] {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .current_dir(root.path())
            .args(["new", name])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name}");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0, "{name}");
    }
}

#[test]
fn valid_desktop_names_create_parseable_manifests() {
    for name in [
        "demo-app",
        "_demo",
        "éclair",
        "_",
        "雪",
        "cafe\u{301}",
        "_\u{301}a",
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .current_dir(root.path())
            .args(["new", name])
            .output()
            .unwrap();
        assert!(output.status.success(), "{name}");
        let project = root.path().join(name);
        let manifest = std::fs::read_to_string(project.join("Cargo.toml")).unwrap();
        let manifest: toml::Value = toml::from_str(&manifest).unwrap();
        assert_eq!(manifest["package"]["name"].as_str(), Some(name));
        let source = std::fs::read_to_string(project.join("src/main.rs")).unwrap();
        let generated_name = source
            .lines()
            .find_map(|line| line.strip_prefix("struct "))
            .and_then(|declaration| declaration.split_whitespace().next())
            .expect("generated struct name");
        let mut characters = generated_name.chars();
        assert!(
            characters
                .next()
                .is_some_and(|ch| ch == '_' || unicode_ident::is_xid_start(ch)),
            "{name:?}: {generated_name}"
        );
        assert!(
            characters.all(unicode_ident::is_xid_continue),
            "{name:?}: {generated_name}"
        );
    }
}
