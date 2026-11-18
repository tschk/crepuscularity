//! Exercise the desktop Cargo runner without linking a desktop window.
#![allow(dead_code)]

#[path = "../src/build_options.rs"]
mod build_options;
#[path = "../src/builder.rs"]
mod builder;
mod ui {
    pub fn error(message: &str) -> ! {
        panic!("{message}")
    }
}
mod hud {
    #[derive(Clone, Debug, Default)]
    pub struct BuildError {
        pub level: String,
        pub message: String,
        pub file: String,
        pub line: u32,
        pub rendered: Option<String>,
    }
    pub enum DevStatus {
        Failed {
            errors: Vec<BuildError>,
            count: usize,
        },
    }
    pub struct HudState {
        pub status: DevStatus,
    }
}

#[test]
fn builder_child() {
    let Some(root) = std::env::var_os("CREPUS_BUILDER_TEST_ROOT") else {
        return;
    };
    let hud = std::sync::Arc::new(std::sync::Mutex::new(hud::HudState {
        status: hud::DevStatus::Failed {
            errors: vec![],
            count: 0,
        },
    }));
    let outcome = builder::cargo_build(
        std::path::Path::new(&root),
        build_options::BuildOptions::debug(),
        Some(hud.clone()),
    );
    let hud = hud.lock().unwrap();
    let hud::DevStatus::Failed { errors, .. } = &hud.status;
    std::fs::write(
        std::path::Path::new(&root).join("outcome.json"),
        serde_json::json!({
            "success": outcome.success,
            "errors": outcome.errors.iter().map(|e| &e.message).collect::<Vec<_>>(),
            "hudErrors": errors.iter().map(|e| &e.message).collect::<Vec<_>>()
        })
        .to_string(),
    )
    .unwrap();
}

fn run(root: &std::path::Path, path: Option<std::ffi::OsString>) -> (serde_json::Value, String) {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "builder_child", "--nocapture"])
        .env("CREPUS_BUILDER_TEST_ROOT", root)
        .env("CARGO_TARGET_DIR", root.join("target"));
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        serde_json::from_slice(&std::fs::read(root.join("outcome.json")).unwrap()).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn missing_cargo_reaches_terminal_and_hud() {
    let root = tempfile::tempdir().unwrap();
    let (outcome, stderr) = run(root.path(), Some(root.path().as_os_str().to_owned()));
    assert_eq!(outcome["success"], false);
    let errors = outcome["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{outcome}");
    assert!(errors[0]
        .as_str()
        .unwrap()
        .contains("failed to spawn cargo"));
    assert_eq!(outcome["hudErrors"], outcome["errors"]);
    assert!(stderr.contains("Failed to spawn cargo"));
}

#[test]
fn malformed_manifest_reaches_terminal_and_hud_without_duplicate_compiler_errors() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package\n").unwrap();
    let (failure, stderr) = run(root.path(), None);
    assert_eq!(failure["success"], false);
    assert!(stderr.contains("Cargo.toml"), "{stderr}");
    assert!(
        failure["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("Cargo.toml")),
        "{failure}"
    );
    assert_eq!(failure["hudErrors"], failure["errors"]);

    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='builder-fixture'\nversion='0.0.0'\nedition='2021'\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    let (success, _) = run(root.path(), None);
    assert_eq!(success["success"], true, "{success}");
    assert!(success["errors"].as_array().unwrap().is_empty());
    std::fs::write(
        root.path().join("src/main.rs"),
        "fn main() { let _: u8 = \"wrong\"; }\n",
    )
    .unwrap();
    let (failure, stderr) = run(root.path(), None);
    assert_eq!(failure["success"], false);
    let errors = failure["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{failure}");
    assert!(errors[0].as_str().unwrap().contains("mismatched types"));
    assert_eq!(failure["hudErrors"], failure["errors"]);
    assert!(stderr.contains("mismatched types"));
}

#[test]
#[cfg(unix)]
fn cargo_process_status_is_authoritative_and_large_stderr_is_drained() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    for exit in [0, 9] {
        std::fs::write(&cargo, format!("#!/bin/sh\n(sleep 8; kill -TERM $$) >/dev/null 2>&1 &\nguard=$!\ni=0\nwhile [ $i -lt 8192 ]; do printf '%s\\n' 'cargo diagnostic pressure fixture 012345678901234567890123456789' >&2; i=$((i+1)); done\nprintf '%s\\n' '{{\"reason\":\"build-finished\",\"success\":true}}'\nkill $guard 2>/dev/null\nexit {exit}\n")).unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::join_paths(
            std::iter::once(bin.clone())
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        let (outcome, stderr) = run(root.path(), Some(path));
        assert_eq!(outcome["success"], exit == 0, "{outcome}");
        assert_eq!(
            stderr.matches("cargo diagnostic pressure fixture").count(),
            8192
        );
        assert_eq!(outcome["hudErrors"], outcome["errors"]);
        assert_eq!(outcome["errors"].as_array().unwrap().is_empty(), exit == 0);
    }
}
