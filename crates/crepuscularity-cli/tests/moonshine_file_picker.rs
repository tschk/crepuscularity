use std::{fs, process::Command};

#[test]
fn native_hosts_publish_file_picker_components_from_standard_inputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("index.crepus"),
            "input type=file name=files multiple=true @change=files.pick",
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["web", "build", "--emit", target, "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        let ir: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap())
                .unwrap();
        assert_eq!(ir["root"][0]["kind"], "filePicker");
        let component = fs::read_to_string(dist.join(format!("CrepusEmit.{target}.ts"))).unwrap();
        assert!(component.contains("type: \"file\""), "{component}");
        assert!(component.contains("\"onChange\""), "{component}");
        assert!(component.contains("\"files.pick\""), "{component}");
    }
}
