use crepuscularity_tauri::{Coverage, TauriProject};
use std::path::Path;

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src-tauri")).unwrap();
    std::fs::write(
        root.path().join("src-tauri/tauri.conf.json"),
        r#"{"build":{"frontendDist":"../dist"}}"#,
    )
    .unwrap();
    root
}

#[test]
fn audit_discovers_bare_option_and_whitespace_command_attributes() {
    let root = project();
    let attributes = [
        "#[tauri::command]",
        "#[command]",
        "#[tauri::command(rename_all = \"camelCase\")]",
        "#[command(rename_all = \"snake_case\")]",
        "# [ tauri :: command ( rename_all = \"camelCase\" ) ]",
        "#[\n command\n(\n rename_all = \"snake_case\"\n)\n]",
        "#[tauri::command ]",
    ];
    for (index, attribute) in attributes.iter().enumerate() {
        std::fs::write(
            root.path().join(format!("src-tauri/command{index}.rs")),
            format!("{attribute}\nfn invoke_name() {{}}\n"),
        )
        .unwrap();
    }
    let report = TauriProject::open(root.path()).unwrap().audit();
    for (index, attribute) in attributes.iter().enumerate() {
        let filename = format!("command{index}.rs");
        assert!(
            report.uses.iter().any(|entry| entry.api == "command"
                && entry.coverage == Coverage::Backend
                && Path::new(&entry.source).file_name().unwrap() == filename.as_str()),
            "missing {}: {:?}",
            attribute,
            report.uses
        );
    }
}

#[test]
fn audit_does_not_treat_longer_attribute_names_as_commands() {
    let root = project();
    std::fs::write(root.path().join("src-tauri/other.rs"),"#[tauri::command_extra]\nfn one() {}\n#[command_extra(rename_all = \"camelCase\")]\nfn two() {}\n#[other::command]\nfn three() {}\n").unwrap();
    let report = TauriProject::open(root.path()).unwrap().audit();
    assert!(
        !report.uses.iter().any(|entry| entry.api == "command"),
        "{:?}",
        report.uses
    );
}
