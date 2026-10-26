use std::{fs, process::Command};

#[test]
fn imba_file_picker_aliases_build_with_native_file_events() {
    for (tag, attributes, event) in [
        ("file-picker", "", "pick"),
        ("filepicker", "", "pick"),
        ("media-picker", "", "pick"),
        ("input", "type=file", "change"),
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("index.crepus"),
            format!("{tag} {attributes} label=\"Upload\" accept=\"image/*,.txt\" multiple=true name=files @{event}=files.pick\n"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["web", "build", "--emit", "imba", "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{tag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dist = site.path().join("dist");
        let ir: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap())
                .unwrap();
        assert_eq!(ir["root"][0]["kind"], "filePicker", "{tag}");
        assert_eq!(ir["root"][0]["onPick"], "files.pick", "{tag}");
        assert_eq!(
            ir["root"][0]["accept"],
            serde_json::json!(["image/*", ".txt"]),
            "{tag}"
        );
        assert_eq!(ir["root"][0]["multiple"], true, "{tag}");
        assert_eq!(ir["root"][0]["style"]["formName"], "files", "{tag}");
        let source = fs::read_to_string(dist.join("CrepusEmit.imba")).unwrap();
        assert!(source.contains("type=(\"file\")"), "{tag}: {source}");
        assert!(
            source.contains("@change=(bindings.dispatch(handlers,\"files.pick\",e))"),
            "{tag}: {source}"
        );
        assert!(dist.join("CrepusEmit.bindings.ts").is_file());
    }
}
