use std::{fs, process::Command};

#[test]
fn null_control_booleans_stay_false_through_every_framework_build() {
    for target in [
        "react",
        "solid",
        "svelte",
        "vue",
        "imba",
        "moonshine-dom",
        "moonshine-ssr",
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("index.crepus"), "checkbox checked={null}\ntoggle checked={missing.enabled}\nfile-picker multiple={null} class=multiple\ninput type=file multiple={null}").unwrap();
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
        let ir: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap(),
        )
        .unwrap();
        for (index, property) in [
            (0, "checked"),
            (1, "checked"),
            (2, "multiple"),
            (3, "multiple"),
        ] {
            assert_eq!(ir["root"][index][property], false, "{target}: {ir}");
        }
    }
}
