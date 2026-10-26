//! Tests the actual desktop Cargo runner without linking GPUI. The HUD data shim
//! checks returned/queued diagnostics only; it provides no visual HUD evidence.
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
            errors: Vec::new(),
            count: 0,
        },
    }));
    let outcome = builder::cargo_build(
        std::path::Path::new(&root),
        build_options::BuildOptions::debug(),
        Some(hud.clone()),
    );
    let hud = hud.lock().unwrap();
    let hud::DevStatus::Failed {
        errors: hud_errors, ..
    } = &hud.status;
    let requested = std::env::var("CREPUS_BUILDER_REQUESTED_BIN")
        .ok()
        .filter(|name| !name.is_empty());
    let selected_executable =
        builder::find_bin_selection(std::path::Path::new(&root), requested.as_deref())
            .and_then(|selection| outcome.executable_for(&selection).ok())
            .map(|(_, path)| path);
    let value = serde_json::json!({"selectedExecutable": selected_executable, "success": outcome.success, "hudErrors": hud_errors.iter().map(|error| &error.message).collect::<Vec<_>>(), "errors": outcome.errors.iter().map(|error| &error.message).collect::<Vec<_>>()});
    std::fs::write(
        std::path::Path::new(&root).join("outcome.json"),
        value.to_string(),
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
fn malformed_manifest_returns_real_cargo_stderr_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package\n").unwrap();
    let (outcome, stderr) = run(root.path(), None);
    assert!(
        stderr.contains("Cargo.toml"),
        "missing terminal diagnostic: {stderr}"
    );
    assert!(
        !outcome["hudErrors"].as_array().unwrap().is_empty(),
        "{outcome}"
    );
    assert_eq!(outcome["success"], false);
    let errors = outcome["errors"].as_array().unwrap();
    assert!(
        errors
            .iter()
            .any(|error| error.as_str().unwrap().contains("Cargo.toml")),
        "{outcome}"
    );
}

#[test]
#[cfg(unix)]
fn failed_process_status_overrides_successful_build_finished_message() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, "#!/bin/sh\nprintf '%s\\n' '{\"reason\":\"build-finished\",\"success\":true}'\nprintf '%s\\n' 'late cargo failure fixture' >&2\nexit 9\n").unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let (outcome, stderr) = run(root.path(), Some(path));
    assert_eq!(
        outcome["success"], false,
        "process exit status must be authoritative: {outcome}"
    );
    assert!(stderr.contains("late cargo failure fixture"), "{stderr}");
    assert!(
        outcome["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .unwrap()
                .contains("late cargo failure fixture")),
        "{outcome}"
    );
}

#[test]
fn real_cargo_success_and_json_compile_error_keep_existing_behavior() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='builder-fixture' # selected package\nversion='0.0.0'\nedition='2021'\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    let (success, _) = run(root.path(), None);
    let executable = success["selectedExecutable"]
        .as_str()
        .expect("inferred name must resolve the real Cargo executable");
    assert!(std::path::Path::new(executable).is_file(), "{success}");
    assert_eq!(
        std::path::Path::new(executable).file_stem().unwrap(),
        "builder-fixture"
    );
    assert_eq!(success["success"], true, "{success}");
    assert!(
        success["errors"].as_array().unwrap().is_empty(),
        "{success}"
    );
    std::fs::write(
        root.path().join("src/main.rs"),
        "fn main() { let _: u8 = \"wrong\"; }\n",
    )
    .unwrap();
    let (failure, stderr) = run(root.path(), None);
    assert_eq!(failure["success"], false);
    let errors = failure["errors"].as_array().unwrap();
    assert_eq!(
        errors.len(),
        1,
        "stderr must not duplicate compiler errors: {failure}"
    );
    assert!(
        errors[0].as_str().unwrap().contains("mismatched types"),
        "{failure}"
    );
    assert_eq!(failure["hudErrors"], failure["errors"]);
    assert!(stderr.contains("mismatched types"), "{stderr}");
}

#[test]
#[cfg(unix)]
fn successful_cargo_drains_large_stderr_before_json_stdout() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    // A watchdog makes a pipe-drain regression fail instead of hanging forever.
    let script = "#!/bin/sh\n(sleep 8; kill -TERM $$) >/dev/null 2>&1 &\nguard=$!\ni=0\nwhile [ $i -lt 8192 ]; do printf '%s\\n' 'cargo warning pressure fixture 012345678901234567890123456789' >&2; i=$((i+1)); done\nprintf '%s\\n' '{\"reason\":\"build-finished\",\"success\":true}'\nkill $guard 2>/dev/null\nexit 0\n";
    std::fs::write(&cargo, script).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let (outcome, stderr) = run(root.path(), Some(path));
    assert_eq!(outcome["success"], true, "{outcome}");
    assert!(outcome["errors"].as_array().unwrap().is_empty());
    assert_eq!(
        stderr.matches("cargo warning pressure fixture").count(),
        8192
    );
}

#[test]
fn binary_selection_parses_exact_toml_name_keys_and_strings() {
    for manifest in [
        "[package]\nname = 'actual-app'\nversion = '0.0.0'\n",
        "[package]\nname = \"actual\\u002dapp\"\nversion = '0.0.0'\n",
        "[package]\nname = \"actual-app\" # selected package\nversion = '0.0.0'\n",
        "[package]\nname-extra = 'wrong-name'\nname = 'actual-app'\nversion = '0.0.0'\n",
        "[package]\nname = 'package-name'\nversion = '0.0.0'\n[[bin]]\nname = 'actual-app' # selected binary\npath = 'src/main.rs'\n",
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
        assert_eq!(builder::find_bin_name(root.path(), None).as_deref(), Some("actual-app"), "{manifest}");
    }
}

#[test]
fn binary_selection_refreshes_manifest_edits_and_deletion() {
    let root = tempfile::tempdir().unwrap();
    let manifest = root.path().join("Cargo.toml");
    std::fs::write(&manifest, "[package]\nname='first'\nversion='0.0.0'\n").unwrap();
    assert_eq!(
        builder::find_bin_name(root.path(), None).as_deref(),
        Some("first")
    );
    std::fs::write(&manifest, "[package]\nname='renamed'\nversion='0.0.0'\n").unwrap();
    assert_eq!(
        builder::find_bin_name(root.path(), None).as_deref(),
        Some("renamed")
    );
    std::fs::remove_file(manifest).unwrap();
    assert_eq!(builder::find_bin_name(root.path(), None), None);
    assert_eq!(
        builder::find_bin_name(root.path(), Some("explicit-bin")).as_deref(),
        Some("explicit-bin")
    );
}

#[test]
fn explicit_binary_override_precedes_manifest_values() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='package-name'\n[[bin]]\nname='other-bin'\n",
    )
    .unwrap();
    assert_eq!(
        builder::find_bin_name(root.path(), Some("explicit-bin")).as_deref(),
        Some("explicit-bin")
    );
}

#[test]
fn malformed_manifest_has_no_inferred_binary() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package\nname='invalid'").unwrap();
    assert_eq!(builder::find_bin_name(root.path(), None), None);
    assert_eq!(
        builder::find_bin_name(root.path(), Some("explicit")).as_deref(),
        Some("explicit")
    );
}

#[test]
fn binary_selection_honors_package_default_run_before_first_named_binary() {
    for manifest in [
        "[package]\nname='package-name'\nversion='0.0.0'\ndefault-run='chosen'\n[[bin]]\nname='first'\npath='src/first.rs'\n[[bin]]\nname='chosen'\npath='src/chosen.rs'\n",
        "[package]\nname='package-name'\nversion='0.0.0'\ndefault-run='chosen'\n",
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/bin")).unwrap();
        std::fs::write(root.path().join("src/bin/chosen.rs"), "fn main() {}\n").unwrap();
        std::fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
        assert_eq!(builder::find_bin_name(root.path(), None).as_deref(), Some("chosen"));
        assert_eq!(builder::find_bin_name(root.path(), Some("explicit")).as_deref(), Some("explicit"));
    }
}

#[test]
fn real_cargo_default_run_resolves_the_selected_second_binary() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package]\nname='default-run-fixture'\nversion='0.0.0'\nedition='2021'\ndefault-run='chosen'\n[workspace]\n[[bin]]\nname='first'\npath='src/first.rs'\n[[bin]]\nname='chosen'\npath='src/chosen.rs'\n").unwrap();
    for name in ["first", "chosen"] {
        std::fs::write(root.path().join(format!("src/{name}.rs")), "fn main() {}\n").unwrap();
    }
    let (outcome, stderr) = run(root.path(), None);
    assert_eq!(outcome["success"], true, "{stderr}");
    let path = std::path::Path::new(outcome["selectedExecutable"].as_str().unwrap());
    assert_eq!(path.file_stem().unwrap(), "chosen", "{outcome}");
    assert!(path.is_file());
}

#[test]
fn real_cargo_auto_discovered_binary_selection_is_unambiguous_and_strict() {
    for (files, expected, requested) in [
        (vec!["src/bin/worker.rs"], Some("worker"), None),
        (vec!["src/main.rs", "src/bin/helper.rs"], Some("host"), None),
        (vec!["src/bin/first.rs", "src/bin/second.rs"], None, None),
        (vec!["src/bin/worker.rs"], None, Some("missing")),
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname='host'\nversion='0.0.0'\nedition='2021'\n[workspace]\n",
        )
        .unwrap();
        for file in files {
            let path = root.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "fn main() {}\n").unwrap();
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "builder_child", "--nocapture"])
            .env("CREPUS_BUILDER_TEST_ROOT", root.path())
            .env("CARGO_TARGET_DIR", root.path().join("target"))
            .env("CREPUS_BUILDER_REQUESTED_BIN", requested.unwrap_or(""))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let outcome: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("outcome.json")).unwrap())
                .unwrap();
        assert_eq!(outcome["success"], true, "{outcome}");
        let selected = outcome["selectedExecutable"].as_str().map(|path| {
            std::path::Path::new(path)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
        });
        assert_eq!(selected, expected, "requested {requested:?}: {outcome}");
    }
}

#[test]
fn virtual_workspace_does_not_infer_a_package_binary() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    assert!(builder::find_bin_selection(root.path(), None).is_none());
    assert_eq!(
        builder::find_bin_selection(root.path(), Some("explicit"))
            .unwrap()
            .name(),
        "explicit"
    );
}
