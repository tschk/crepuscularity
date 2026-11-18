use std::process::Command;

#[test]
fn package_alias_compiles_without_direct_serde_json_dependency() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"tauri-alias-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ntauri = {{ package = \"crepuscularity-tauri\", path = {:?} }}\n",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        ),
    )
    .unwrap();
    std::fs::write(
        root.path().join("src/main.rs"),
        "#[tauri::command]\nfn greet(name: String) -> String { format!(\"Hello, {name}\") }\n\nfn main() { let _ = tauri::Builder::default().invoke_handler(tauri::generate_handler![greet]).run(tauri::generate_context!()); }\n",
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
