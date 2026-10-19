use std::process::Command;

#[test]
fn tui_new_preserves_existing_project_files() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("existing");
    std::fs::create_dir_all(project.join("src")).unwrap();
    for file in ["Cargo.toml", "src/main.rs", "app.crepus"] {
        std::fs::write(project.join(file), format!("keep {file}")).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(root.path())
        .args(["tui", "new", "existing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    for file in ["Cargo.toml", "src/main.rs", "app.crepus"] {
        assert_eq!(
            std::fs::read_to_string(project.join(file)).unwrap(),
            format!("keep {file}")
        );
    }
}

#[test]
fn tui_new_still_creates_a_new_project() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(root.path())
        .args(["tui", "new", "fresh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(root.path().join("fresh/Cargo.toml").is_file());
    assert!(root.path().join("fresh/src/main.rs").is_file());
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.path().join("fresh/Cargo.toml")).unwrap())
            .unwrap();
    let workspace: toml::Value = toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
    let renderer: toml::Value =
        toml::from_str(include_str!("../../crepuscularity-tui/Cargo.toml")).unwrap();
    assert_eq!(
        manifest["dependencies"]["ratatui"]["version"],
        workspace["workspace"]["dependencies"]["ratatui"]["version"]
    );
    assert_eq!(
        manifest["dependencies"]["crossterm"],
        renderer["dependencies"]["crossterm"]
    );
}

#[test]
fn tui_new_rejects_invalid_names_before_creating_files() {
    for name in ["bad name", "1-demo", "bad\"quote", "../outside", "", "app²"] {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("cwd");
        std::fs::create_dir(&cwd).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .current_dir(&cwd)
            .args(["tui", "new", name])
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {name:?}");
        assert_eq!(std::fs::read_dir(&cwd).unwrap().count(), 0);
        assert!(!root.path().join("outside").exists());
    }
}

#[test]
fn tui_new_preserves_valid_unicode_and_combining_identifier_names() {
    for name in ["雪", "cafe\u{301}", "_\u{301}a"] {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .current_dir(root.path())
            .args(["tui", "new", name])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let project = root.path().join(name);
        let manifest: toml::Value =
            toml::from_str(&std::fs::read_to_string(project.join("Cargo.toml")).unwrap()).unwrap();
        assert_eq!(manifest["package"]["name"].as_str(), Some(name));
        assert!(project.join("src/main.rs").is_file());
        assert!(project.join("app.crepus").is_file());
    }
}
