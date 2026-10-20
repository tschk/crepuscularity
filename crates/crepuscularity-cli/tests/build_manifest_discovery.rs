//! Bare builds must distinguish absent targets from an invalid discovered manifest.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn bare_build(manifest: Option<&str>) -> (std::process::Output, bool) {
    let directory = tempfile::tempdir().unwrap();
    if let Some(source) = manifest {
        std::fs::write(directory.path().join("crepus.toml"), source).unwrap();
    }
    let bin = directory.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(
        &cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CREPUS_TEST_CARGO_LOG\"\nprintf '%s\\n' '{\"reason\":\"build-finished\",\"success\":true}'\n",
    )
    .unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let log = directory.path().join("cargo.log");
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(directory.path())
        .env("PATH", &bin)
        .env("CREPUS_TEST_CARGO_LOG", &log)
        .arg("build")
        .output()
        .unwrap();
    (output, log.exists())
}

#[test]
fn invalid_discovered_manifest_fails_without_running_cargo() {
    for source in ["[[targets]", "targets = 'wrong type'"] {
        let (output, cargo_ran) = bare_build(Some(source));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{stderr}");
        assert!(stderr.contains("TOML parse error"), "{stderr}");
        assert!(!cargo_ran, "invalid manifest fell back to Cargo");
    }
}

#[test]
fn absent_manifest_or_valid_manifest_without_targets_keeps_cargo_fallback() {
    for source in [None, Some("[ios]\nscheme = 'LegacyApp'\n")] {
        let (output, cargo_ran) = bare_build(source);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(cargo_ran);
    }
}
