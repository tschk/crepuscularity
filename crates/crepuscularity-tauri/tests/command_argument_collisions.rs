use std::process::Command;

#[test]
fn renamed_arguments_cannot_share_a_wire_key() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("Cargo.toml"), format!(
        "[package]\nname='tauri-command-collisions'\nversion='0.1.0'\nedition='2021'\n[dependencies]\ntauri={{package='crepuscularity-tauri',path={:?}}}\n",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    )).unwrap();
    for (case, key) in [("camelCase", "fooBar"), ("snake_case", "foo_bar")] {
        std::fs::write(root.path().join("src/main.rs"), format!(
            "#[tauri::command(rename_all={case:?})]\n#[allow(non_snake_case)]\nfn pair(foo_bar:i32,fooBar:i32)->(i32,i32){{(foo_bar,fooBar)}}\nfn main(){{}}\n"
        )).unwrap();
        let output = Command::new("cargo")
            .args(["check", "--quiet", "--offline"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "accepted ambiguous {case} arguments"
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("duplicate command argument wire key") && diagnostic.contains(key),
            "{diagnostic}"
        );
    }
}
