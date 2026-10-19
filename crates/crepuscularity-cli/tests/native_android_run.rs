//! Public CLI transport acceptance with labelled Gradle/ADB stubs; no device build.
#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn executable(path: &Path, source: &str) {
    std::fs::write(path, source).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn android_run_preserves_application_id_before_inline_comments() {
    for assignment in [
        "applicationId = \"com.example.app\" // release identity",
        "applicationId = \"com.example.app\"",
        "applicationId   =   \"com.example.app\"   // comment",
    ] {
        let root = tempfile::tempdir().unwrap();
        let android = root.path().join("android");
        std::fs::create_dir_all(android.join("app")).unwrap();
        std::fs::write(android.join("app/build.gradle.kts"), format!("android {{\n defaultConfig {{\n applicationIdSuffix = \".decoy\"\n {assignment}\n }}\n}}\n")).unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        executable(
            &android.join("gradlew"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CREPUS_TEST_GRADLE_ARGS\"\n",
        );
        executable(
            &bin.join("adb"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CREPUS_TEST_ADB_ARGS\"\n",
        );
        let gradle_args = root.path().join("gradle-args");
        let adb_args = root.path().join("adb-args");
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["native", "run", "android", "--dir"])
            .arg(root.path())
            .env("PATH", path)
            .env("CREPUS_TEST_GRADLE_ARGS", &gradle_args)
            .env("CREPUS_TEST_ADB_ARGS", &adb_args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(gradle_args).unwrap(),
            ":app:installDebug\n--quiet\n"
        );
        assert_eq!(
            std::fs::read_to_string(adb_args).unwrap(),
            "shell\nam\nstart\n-n\ncom.example.app/dev.crepuscularity.nativeshell.MainActivity\n",
            "{assignment}"
        );
    }
}
