#![cfg(target_os = "macos")]

use std::process::Command;

#[test]
#[ignore = "requires installed XcodeGen"]
fn share_target_names_round_trip_yaml_and_repeat_without_project_changes() {
    for platform in ["ios-share", "macos-share"] {
        for name in [
            "Share: Docs #1",
            "Quote\"Share",
            "雪 Share",
            "Plain",
            "Line\u{85}\u{2028}\u{2029}\u{7f}Share",
            "#Note",
            "\"Plain\"",
            "NativeShell",
        ] {
            let root = tempfile::tempdir().unwrap();
            let created = Command::new(env!("CARGO_BIN_EXE_crepus"))
                .current_dir(root.path())
                .args(["native", "new", "project"])
                .output()
                .unwrap();
            assert!(
                created.status.success(),
                "{}",
                String::from_utf8_lossy(&created.stderr)
            );
            let project = root.path().join("project");
            let spec = project.join("ios/project.yml");
            let text = std::fs::read_to_string(&spec)
                .unwrap()
                .replace("targets:\n", "targets:\n  #Note:\n");
            std::fs::write(&spec, text + "\nschemes: {}\n").unwrap();
            if name == "\"Plain\"" {
                let seeded = Command::new(env!("CARGO_BIN_EXE_crepus"))
                    .args(["native", "extension", platform, "--dir"])
                    .arg(&project)
                    .args(["--name", "Plain"])
                    .output()
                    .unwrap();
                assert!(seeded.status.success());
            }
            for attempt in 0..2 {
                let before = std::fs::read_to_string(&spec).unwrap();
                let added = Command::new(env!("CARGO_BIN_EXE_crepus"))
                    .args(["native", "extension", platform, "--dir"])
                    .arg(&project)
                    .args(["--name", name])
                    .output()
                    .unwrap();
                assert!(
                    added.status.success(),
                    "{}",
                    String::from_utf8_lossy(&added.stderr)
                );
                if attempt == 1 {
                    assert_eq!(std::fs::read_to_string(&spec).unwrap(), before);
                }
                let parsed = Command::new("xcodegen")
                    .args(["dump", "--quiet", "--no-env", "--type", "json", "--spec"])
                    .arg(&spec)
                    .arg("--file")
                    .arg(root.path().join("resolved.json"))
                    .output()
                    .unwrap();
                assert!(
                    parsed.status.success(),
                    "{platform} {name}: {}",
                    String::from_utf8_lossy(&parsed.stderr)
                );
                let json: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(root.path().join("resolved.json")).unwrap(),
                )
                .unwrap();
                assert!(json["targets"].get(name).is_some(), "{json}");
                if name == "Plain" && attempt == 0 {
                    let text = std::fs::read_to_string(&spec)
                        .unwrap()
                        .replace("  \"Plain\":", "  Plain: # existing target");
                    std::fs::write(&spec, text).unwrap();
                }
            }
        }
    }
}
