//! Public CLI configuration acceptance with a labelled Gradle stub; no Android build.
#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn android_config_updates_only_exact_assignments_and_keeps_inline_comments() {
    let root = tempfile::tempdir().unwrap();
    let android = root.path().join("android");
    std::fs::create_dir_all(android.join("app")).unwrap();
    std::fs::write(root.path().join("crepus.toml"), "[android]\napplication_id='com.example.updated'\nnamespace='com.example.updated.namespace'\n").unwrap();
    std::fs::write(android.join("settings.gradle.kts"), "").unwrap();
    let before = "// Keep unrelated configuration\nandroid {\n    namespaceSuffix = \".namespace-decoy\"\n    namespace = \"com.example.old\" // namespace ownership\n    defaultConfig {\n        applicationIdSuffix = \".debug\" // suffix ownership\n        applicationIdName = \"do-not-replace\"\n        applicationId = \"com.example.old\" // release identity\n        applicationId == \"reference-only\"\n        versionCode = 7\n    }\n}\n";
    std::fs::write(android.join("app/build.gradle.kts"), before).unwrap();
    let stub = android.join("gradlew");
    std::fs::write(&stub, "#!/bin/sh\n# Labelled Gradle transport stub: capture input without compiling.\ncat app/build.gradle.kts > \"$CREPUS_TEST_CONFIG_CAPTURE\"\nprintf '%s\\n' \"$@\" > \"$CREPUS_TEST_GRADLE_ARGS\"\n").unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let capture = root.path().join("captured.gradle.kts");
    let args = root.path().join("gradle-args");
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["native", "build", "android", "--dir"])
        .arg(root.path())
        .env("CREPUS_TEST_CONFIG_CAPTURE", &capture)
        .env("CREPUS_TEST_GRADLE_ARGS", &args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(args).unwrap(),
        ":app:assembleDebug\n--quiet\n"
    );
    let configured = std::fs::read_to_string(capture).unwrap();
    let expected = before
        .replace(
            "namespace = \"com.example.old\"",
            "namespace = \"com.example.updated.namespace\"",
        )
        .replace(
            "applicationId = \"com.example.old\"",
            "applicationId = \"com.example.updated\"",
        );
    // Existing writer normalizes the final newline; all other content stays exact.
    assert_eq!(configured, expected.trim_end_matches('\n'));
    assert_eq!(
        std::fs::read_to_string(android.join("app/build.gradle.kts")).unwrap(),
        configured
    );
}
