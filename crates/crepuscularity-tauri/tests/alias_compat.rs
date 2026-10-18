use std::process::Command;

#[test]
fn package_alias_supports_naming_and_rejects_invalid_options_without_direct_serde_json() {
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
        r#"#[tauri::command(rename_all = "camelCase")]
fn greet(first_name: String) -> String { format!("Hello, {first_name}") }
fn main() {
    let app = tauri::Builder::default().invoke_handler(tauri::generate_handler![greet]).build();
    assert_eq!(app.invoke("greet", tauri::serde_json::json!({"firstName":"Ada"})).unwrap(), "Hello, Ada");
}"#,
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--offline"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (options, diagnostic) in [
        (
            r#"rename_all = "kebab-case""#,
            "expected \"camelCase\" or \"snake_case\"",
        ),
        ("rename_all = 42", "expected string literal"),
        (
            r#"rename_all = "camelCase", rename_all = "snake_case""#,
            "duplicate rename_all",
        ),
        (r#"rename = "custom""#, "unsupported command option"),
        ("async", "unsupported command option"),
        ("rename_all", "expected `=`"),
        (
            r#"rename_all = "camelCase", unknown = true"#,
            "unsupported command option",
        ),
    ] {
        std::fs::write(root.path().join("src/main.rs"), format!(
            "#[tauri::command({options})]\nfn greet(first_name: String) -> String {{ first_name }}\nfn main() {{}}\n"
        )).unwrap();
        let output = Command::new("cargo")
            .args(["check", "--quiet", "--offline"])
            .current_dir(root.path())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "accepted {options}");
        assert!(stderr.contains(diagnostic), "{options}: {stderr}");
    }
}
