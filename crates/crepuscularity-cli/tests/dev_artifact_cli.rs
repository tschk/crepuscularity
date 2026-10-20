//! Dev must launch Cargo's reported artifact, including custom target directories.
#![cfg(all(feature = "desktop", unix))]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn dev_launches_custom_target_artifact_instead_of_stale_default_binary() {
    for use_env in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let app = root.join("ui/tui");
        std::fs::create_dir_all(app.join("src")).unwrap();
        std::fs::write(
            app.join("Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            app.join("src/main.rs"),
            "fn main() { std::fs::write(\"launched\", \"fresh\").unwrap(); }\n",
        )
        .unwrap();
        let stale = app.join("target/debug/fixture");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "#!/bin/sh\nprintf stale > launched\n").unwrap();
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o755)).unwrap();

        let custom_target = app.join("custom-artifacts");
        let mut command = Command::new(env!("CARGO_BIN_EXE_crepus"));
        command
            .current_dir(&root)
            .args(["dev", "tui", "--bin", "fixture", "--emit-events"])
            .env("CARGO_NET_OFFLINE", "true")
            .env_remove("CARGO_BUILD_TARGET")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if use_env {
            command.env("CARGO_TARGET_DIR", &custom_target);
        } else {
            command.env_remove("CARGO_TARGET_DIR");
            std::fs::create_dir_all(app.join(".cargo")).unwrap();
            std::fs::write(
                app.join(".cargo/config.toml"),
                "[build]\ntarget-dir='custom-artifacts'\n",
            )
            .unwrap();
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let timed_out = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                break true;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !timed_out && output.status.success(),
            "use_env={use_env}, timed_out={timed_out}\n{stdout}\n{stderr}"
        );
        assert_eq!(
            std::fs::read_to_string(app.join("launched")).unwrap(),
            "fresh"
        );
        let expected = custom_target.join("debug/fixture");
        let events: Vec<serde_json::Value> = stdout
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        for (event_name, path_key) in [
            ("compilation_success", "output"),
            ("process_launched", "binary"),
        ] {
            let matching: Vec<_> = events
                .iter()
                .filter(|event| event["event"] == event_name)
                .collect();
            assert_eq!(matching.len(), 1, "{stdout}");
            assert_eq!(
                Path::new(matching[0][path_key].as_str().unwrap()),
                expected,
                "use_env={use_env}: {stdout}"
            );
        }
    }
}
