//! `crepus native` — Native mobile applications for iOS and Android.
//!
//! Scaffold and build native iOS (SwiftUI) and Android (Jetpack Compose) apps
//! that use **View IR** (`crepuscularity-native::render_template_to_ir`) to
//! render `.crepus` templates.
//!
//! The scaffold is the same source tree as `examples/native-shells/` — a
//! SwiftPM package under `<dir>/ios/` and a Gradle module under
//! `<dir>/android/`, sharing a common `fixture.json`. We *don't* embed the
//! Gradle wrapper jar; users run `gradle wrapper --gradle-version 8.10`
//! (or just open the project in Android Studio, which regenerates it
//! automatically) before the first `./gradlew` invocation.

use std::fs;
use std::path::Path;

use crate::cli::NativeCommands;
use crate::error::CrepusCliError;

#[cfg(test)]
use crepuscularity_native::{render_template_to_ir, to_json, HotReloadEnvelope, HotReloadMessage};
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::atomic::AtomicU64;
#[cfg(test)]
use std::sync::RwLock;

pub mod build;
pub mod capabilities;
pub mod ir;
pub mod mobile;
pub mod scaffold;

pub use build::IosBuildTarget;
pub use ir::{CodegenArgs, IrArgs, SyncArgs};
pub use mobile::execute_mobile;
pub use scaffold::scaffold_native_app_at;

#[cfg(test)]
pub(crate) use build::{
    android_main_activity_component, apply_android_config, gradle_kts_value,
    load_mobile_ios_config, MobileAndroidConfig,
};
#[cfg(test)]
pub(crate) use ir::sync_native_fixture_inner;
#[cfg(test)]
pub(crate) use mobile::{
    mobile_dev_response, refresh_mobile_state, MobileDevState, MobilePlatform,
};
#[cfg(test)]
pub(crate) use scaffold::{
    main_bundle_id, share_extension_target, ShareExtensionPlatform, TEMPLATE_FILES,
};

pub fn execute(cmd: NativeCommands) -> Result<(), CrepusCliError> {
    match cmd {
        NativeCommands::New { name } => {
            scaffold::scaffold_native_app(&name);
            Ok(())
        }
        NativeCommands::Add { capability, dir } => capabilities::add_capability(&capability, &dir),
        NativeCommands::Extension { extension } => {
            build::handle_extension(extension);
            Ok(())
        }
        NativeCommands::Ir { args } => match ir::run_ir_parsed(args) {
            Ok(out) => {
                print!("{out}");
                Ok(())
            }
            Err(e) => {
                let payload = serde_json::json!({ "error": e.to_string() });
                eprintln!("{payload}");
                std::process::exit(1);
            }
        },
        NativeCommands::Sync { args } => ir::sync_native_fixture_inner(args),
        NativeCommands::Codegen { args } => ir::codegen_native_source_inner(args).map(|_| ()),
        NativeCommands::Build { platform } => {
            build::handle_build(platform);
            Ok(())
        }
        NativeCommands::Run { platform } => {
            build::handle_run(platform);
            Ok(())
        }
    }
}

pub(crate) fn prepend_kotlin_package(path: &Path) {
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };
    if source.starts_with("package ") {
        return;
    }
    let updated = format!("package dev.crepuscularity.nativeshell\n\n{source}");
    let _ = fs::write(path, updated);
}

pub(crate) fn capitalize_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    if let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
    }
    out.extend(chars);
    out
}

/// ponytail: cap template source at 10 MB
pub(crate) const MAX_TEMPLATE_SIZE: usize = 10_000_000;

pub(crate) fn check_template_size(len: usize) -> Result<(), String> {
    if len > MAX_TEMPLATE_SIZE {
        return Err(format!(
            "template too large ({} bytes, max {})",
            len, MAX_TEMPLATE_SIZE
        ));
    }
    Ok(())
}

#[cfg(test)]
mod mobile_tests {
    use super::*;
    use crepuscularity_core::context::TemplateContext;

    fn test_state() -> MobileDevState {
        let ir = render_template_to_ir("div\n  \"Hi\"", &TemplateContext::new()).unwrap();
        let ir_json = to_json(&ir).unwrap();
        MobileDevState {
            sequence: AtomicU64::new(7),
            root: PathBuf::from("."),
            platform: MobilePlatform::All,
            template_path: PathBuf::from("views/main.crepus"),
            ctx: TemplateContext::new(),
            last_template: RwLock::new("div\n  \"Hi\"".to_string()),
            last_ir_json: RwLock::new(ir_json),
            last_event: RwLock::new(HotReloadEnvelope {
                sequence: 7,
                message: HotReloadMessage::FullReload {
                    ir,
                    reason: "test".to_string(),
                },
            }),
        }
    }

    fn temp_state(template: &str) -> (tempfile::TempDir, MobileDevState) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let views = root.join("views");
        std::fs::create_dir_all(&views).unwrap();
        let template_path = views.join("main.crepus");
        std::fs::write(&template_path, template).unwrap();
        let ctx = TemplateContext::new();
        let ir = render_template_to_ir(template, &ctx).unwrap();
        let ir_json = to_json(&ir).unwrap();
        (
            temp,
            MobileDevState {
                sequence: AtomicU64::new(0),
                root,
                platform: MobilePlatform::Ios,
                template_path,
                ctx,
                last_template: RwLock::new(template.to_string()),
                last_ir_json: RwLock::new(ir_json),
                last_event: RwLock::new(HotReloadEnvelope {
                    sequence: 0,
                    message: HotReloadMessage::FullReload {
                        ir,
                        reason: "test".to_string(),
                    },
                }),
            },
        )
    }

    #[test]
    fn mobile_http_waits_for_a_complete_request_line() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            mobile::handle_mobile_dev_stream(&mut stream, &test_state()).unwrap();
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        client.write_all(b"GET /i").unwrap();
        let mut byte = [0];
        let error = client
            .read(&mut byte)
            .expect_err("must not respond to a partial request line");
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ));
        client.write_all(b"r HTTP/1.1\r\n\r\n").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("Hi"), "{response}");
    }

    #[test]
    fn mobile_http_rejects_truncated_and_oversized_request_lines() {
        use std::io::{Read, Write};
        use std::net::{Shutdown, TcpListener, TcpStream};
        for request in [
            "GET /health HTTP/1.1".to_owned(),
            format!("GET /health {}\r\n", "x".repeat(4096)),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                mobile::handle_mobile_dev_stream(&mut stream, &test_state()).unwrap();
            });
            let mut client = TcpStream::connect(address).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client.write_all(request.as_bytes()).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            server.join().unwrap();
            assert!(
                response.starts_with("HTTP/1.1 400 Bad Request"),
                "{response}"
            );
        }
    }

    #[test]
    fn health_response_reports_sequence() {
        let response = mobile_dev_response("/health", &test_state());
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"ok\":true"));
        assert!(response.contains("\"sequence\":7"));
    }

    #[test]
    fn ir_response_returns_latest_ir() {
        let expected = test_state()
            .last_ir_json
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let response = mobile_dev_response("/ir", &test_state());
        assert!(response.contains(&expected));
    }

    #[test]
    fn events_response_streams_hot_reload_envelope() {
        let response = mobile_dev_response("/events", &test_state());
        assert!(response.contains("Content-Type: text/event-stream"));
        assert!(response.contains("event: crepus-mobile"));
        assert!(response.contains("\"sequence\":7"));
        assert!(response.contains("\"kind\":\"fullReload\""));
    }

    #[test]
    fn refresh_included_template_uses_last_good_ir() {
        let (_temp, state) = temp_state("div\n  \"Before\"");
        let mut state = state;
        state.ctx.base_dir = state
            .template_path
            .parent()
            .map(std::path::Path::to_path_buf);
        let child = state.template_path.with_file_name("child.crepus");
        std::fs::write(&child, "\"Before\"").unwrap();
        let source = "div\n  include child.crepus";
        std::fs::write(&state.template_path, source).unwrap();
        *state.last_template.write().unwrap() = source.into();
        let mut previous: crepuscularity_native::ViewIr =
            serde_json::from_str(&state.last_ir_json.read().unwrap()).unwrap();
        std::fs::write(&child, "\"After\"").unwrap();
        refresh_mobile_state(&state);
        assert!(
            state.last_ir_json.read().unwrap().contains("After"),
            "{:?}",
            state.last_event.read().unwrap()
        );
        assert!(matches!(
            state.last_event.read().unwrap().message,
            HotReloadMessage::Patch { .. }
        ));
        if let HotReloadMessage::Patch { mutations } = &state.last_event.read().unwrap().message {
            crepuscularity_native::apply_mutations(&mut previous, mutations).unwrap();
        }
        let cached: crepuscularity_native::ViewIr =
            serde_json::from_str(&state.last_ir_json.read().unwrap()).unwrap();
        assert_eq!(previous, cached);
        assert!(std::fs::read_to_string(state.root.join("fixture.json"))
            .unwrap()
            .contains("After"));
        std::fs::remove_file(&child).unwrap();
        refresh_mobile_state(&state);
        assert!(matches!(
            state.last_event.read().unwrap().message,
            HotReloadMessage::Error { .. }
        ));
        assert!(state.last_ir_json.read().unwrap().contains("After"));
        std::fs::write(&child, "\"Recovered\"").unwrap();
        refresh_mobile_state(&state);
        assert!(state.last_ir_json.read().unwrap().contains("Recovered"));
    }

    #[test]
    fn refresh_emits_patch_for_literal_change() {
        let (_temp, state) = temp_state("div\n  \"Hi\"");
        std::fs::write(&state.template_path, "div\n  \"Bye\"").unwrap();
        refresh_mobile_state(&state);
        let event = state
            .last_event
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(matches!(event.message, HotReloadMessage::Patch { .. }));
        assert!(state
            .last_ir_json
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains("Bye"));
    }

    #[test]
    fn refresh_error_keeps_last_good_ir() {
        let (_temp, state) = temp_state("div\n  \"Hi\"");
        std::fs::write(&state.template_path, "include missing.crepus").unwrap();
        refresh_mobile_state(&state);
        let event = state
            .last_event
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(matches!(event.message, HotReloadMessage::Error { .. }));
        let ir = state
            .last_ir_json
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(ir.contains("Hi"));
        assert!(!ir.contains("bad indent"));
    }

    #[test]
    fn refresh_render_error_does_not_emit_patch() {
        let (_temp, state) = temp_state("div\n  \"Hi\"");
        std::fs::write(&state.template_path, "div\n  include missing.crepus").unwrap();
        refresh_mobile_state(&state);
        let event = state
            .last_event
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(matches!(event.message, HotReloadMessage::Error { .. }));
        let ir = state
            .last_ir_json
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(ir.contains("Hi"));
        assert!(!ir.contains("missing.crepus"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_ascii_basic() {
        assert_eq!(capitalize_ascii("debug"), "Debug");
        assert_eq!(capitalize_ascii("Release"), "Release");
        assert_eq!(capitalize_ascii(""), "");
        assert_eq!(capitalize_ascii("a"), "A");
    }

    #[test]
    fn gradle_kts_value_reads_application_id() {
        let src = r#"
android {
    defaultConfig {
        applicationId = "dev.crepuscularity.nativeshell"
    }
}
"#;
        assert_eq!(
            gradle_kts_value(src, "applicationId"),
            Some("dev.crepuscularity.nativeshell".to_string())
        );
    }

    #[test]
    fn gradle_kts_value_edge_cases() {
        // Missing equal sign
        assert_eq!(
            gradle_kts_value("applicationId \"dev.app\"", "applicationId"),
            None
        );
        // Key as prefix of another key
        assert_eq!(
            gradle_kts_value("applicationIdSuffix = \"-dev\"", "applicationId"),
            None
        );
        // Different spacing
        assert_eq!(
            gradle_kts_value("  applicationId   =   \"dev.app\"  ", "applicationId"),
            Some("dev.app".to_string())
        );
        // No spacing
        assert_eq!(
            gradle_kts_value("applicationId=\"dev.app\"", "applicationId"),
            Some("dev.app".to_string())
        );
        // Missing quotes
        assert_eq!(
            gradle_kts_value("applicationId = dev.app", "applicationId"),
            Some("dev.app".to_string())
        );
        // Empty value inside quotes
        assert_eq!(
            gradle_kts_value("applicationId = \"\"", "applicationId"),
            Some("".to_string())
        );
        // Not found
        assert_eq!(gradle_kts_value("versionCode = 1", "applicationId"), None);
        // First match wins
        let src = "applicationId = \"dev.app1\"\napplicationId = \"dev.app2\"";
        assert_eq!(
            gradle_kts_value(src, "applicationId"),
            Some("dev.app1".to_string())
        );
    }

    #[test]
    fn gradle_kts_value_ignores_only_unquoted_line_comments() {
        for (source, expected) in [
            (
                r#"applicationId = "dev.app" // release identity"#,
                "dev.app",
            ),
            (r#"applicationId="dev.app"//comment"#, "dev.app"),
            (r#"applicationId = "" // empty"#, ""),
            ("applicationId = dev.app // legacy bare value", "dev.app"),
            (
                r#"applicationId = "https://example.test" // suffix"#,
                "https://example.test",
            ),
            (
                r#"applicationId = "dev.quoted\"//value" // suffix"#,
                r#"dev.quoted\"//value"#,
            ),
        ] {
            assert_eq!(
                gradle_kts_value(source, "applicationId").as_deref(),
                Some(expected),
                "{source}"
            );
        }
    }

    #[test]
    fn android_component_uses_generated_shell_package() {
        assert_eq!(
            android_main_activity_component("hk.tsc.acme"),
            "hk.tsc.acme/dev.crepuscularity.nativeshell.MainActivity"
        );
    }

    #[test]
    fn ios_share_target_uses_main_bundle_suffix() {
        let project = r#"
targets:
  App:
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: hk.tsc.acme
"#;
        assert_eq!(main_bundle_id(project), "hk.tsc.acme");
        let target = share_extension_target(
            "AcmeShare",
            &main_bundle_id(project),
            ShareExtensionPlatform::Ios,
        );
        assert!(target.contains("  \"AcmeShare\":"));
        assert!(target.contains("type: app-extension"));
        assert!(target.contains("platform: iOS"));
        assert!(target.contains("path: ShareExtension"));
        assert!(target.contains("PRODUCT_BUNDLE_IDENTIFIER: hk.tsc.acme.share"));
        assert!(target.contains("Build Rust Actions"));
    }

    #[test]
    fn macos_share_target_uses_macos_sources_and_rust_target() {
        let target =
            share_extension_target("AcmeMacShare", "hk.tsc.acme", ShareExtensionPlatform::Macos);
        assert!(target.contains("  \"AcmeMacShare\":"));
        assert!(target.contains("platform: macOS"));
        assert!(target.contains("path: MacShareExtension"));
        assert!(target.contains("aarch64-apple-darwin"));
        assert!(target.contains("x86_64-apple-darwin"));
    }

    #[test]
    fn mobile_ios_config_reads_root_manifest_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("ios")).unwrap();
        fs::write(
            root.join("crepus.toml"),
            r#"
[ios]
bundle_id = "hk.tsc.acme"
development_team = "LZ3NL5434Q"
code_sign_style = "Automatic"
allow_provisioning_updates = true
"#,
        )
        .unwrap();
        fs::write(
            root.join("ios/crepus.toml"),
            r#"
[ios]
scheme = "CrepusMobileApp"
"#,
        )
        .unwrap();
        fs::write(
            root.join("ios/project.yml"),
            "name: CrepusMobileApp\nPRODUCT_BUNDLE_IDENTIFIER: dev.crepuscularity.mobile\n",
        )
        .unwrap();

        let cfg = load_mobile_ios_config(&root.join("ios"));
        assert_eq!(cfg.scheme, "CrepusMobileApp");
        assert_eq!(cfg.bundle_id, "hk.tsc.acme");
        assert_eq!(cfg.development_team.as_deref(), Some("LZ3NL5434Q"));
        assert_eq!(cfg.code_sign_style.as_deref(), Some("Automatic"));
        assert!(cfg.allow_provisioning_updates);
    }

    #[test]
    fn apply_android_config_rewrites_gradle_identity() {
        let temp = tempfile::tempdir().unwrap();
        let android_dir = temp.path().join("android");
        fs::create_dir_all(android_dir.join("app")).unwrap();
        fs::write(
            android_dir.join("app/build.gradle.kts"),
            r#"android {
    namespace = "dev.crepuscularity.nativeshell"
    defaultConfig {
        applicationId = "dev.crepuscularity.nativeshell"
    }
}
"#,
        )
        .unwrap();

        apply_android_config(
            &android_dir,
            &MobileAndroidConfig {
                application_id: Some("hk.tsc.acme".to_string()),
                namespace: Some("hk.tsc.acme".to_string()),
            },
        );
        let gradle = fs::read_to_string(android_dir.join("app/build.gradle.kts")).unwrap();
        assert!(gradle.contains("namespace = \"hk.tsc.acme\""));
        assert!(gradle.contains("applicationId = \"hk.tsc.acme\""));
    }

    #[test]
    fn android_identity_rewrites_preserve_suffix_and_comments() {
        let temp = tempfile::tempdir().unwrap();
        let android = temp.path().join("android");
        fs::create_dir_all(android.join("app")).unwrap();
        let path = android.join("app/build.gradle.kts");
        fs::write(&path, "android {\n  namespace = \"old.namespace\" // namespace note\n  defaultConfig {\n    applicationIdSuffix = \".debug\" // suffix note\n    applicationId = \"old.app\" // identity note\n  }\n}\n").unwrap();
        apply_android_config(
            &android,
            &MobileAndroidConfig {
                application_id: Some("new.app".into()),
                namespace: Some("new.namespace".into()),
            },
        );
        let result = fs::read_to_string(path).unwrap();
        assert!(
            result.contains("applicationIdSuffix = \".debug\" // suffix note"),
            "{result}"
        );
        assert!(
            result.contains("applicationId = \"new.app\" // identity note"),
            "{result}"
        );
        assert!(
            result.contains("namespace = \"new.namespace\" // namespace note"),
            "{result}"
        );
        assert_eq!(result.matches("applicationId =").count(), 1, "{result}");
    }

    #[test]
    fn sync_fixture_writes_native_shell_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app");
        fs::create_dir_all(root.join("views")).unwrap();
        fs::create_dir_all(root.join("ios/Sources/NativeShell")).unwrap();
        fs::create_dir_all(root.join("android/app/src/main/assets")).unwrap();
        fs::write(
            root.join("views/main.crepus"),
            "div flex flex-col\n  span\n    \"Hello {name}\"",
        )
        .unwrap();

        sync_native_fixture_inner(SyncArgs {
            template: root.join("views/main.crepus"),
            dir: root.clone(),
            out: vec![root.join("linux/share/dashboard.view-ir.json")],
            no_defaults: false,
            component: None,
            ctx: None,
            vars: vec!["name=Acme".into()],
            pretty: true,
        })
        .unwrap();

        let root_fixture = fs::read_to_string(root.join("fixture.json")).unwrap();
        let ios_fixture =
            fs::read_to_string(root.join("ios/Sources/NativeShell/fixture.json")).unwrap();
        let android_fixture =
            fs::read_to_string(root.join("android/app/src/main/assets/fixture.json")).unwrap();
        let linux_fixture =
            fs::read_to_string(root.join("linux/share/dashboard.view-ir.json")).unwrap();

        assert_eq!(root_fixture, ios_fixture);
        assert_eq!(root_fixture, android_fixture);
        assert_eq!(root_fixture, linux_fixture);
        assert!(root_fixture.contains("Hello Acme"));
        assert!(root_fixture.contains("\"kind\": \"stack\""));
    }

    #[test]
    fn sync_fixture_can_write_only_explicit_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app");
        fs::create_dir_all(root.join("views")).unwrap();
        fs::write(
            root.join("views/main.crepus"),
            "div flex flex-col\n  span\n    \"Hello {name}\"",
        )
        .unwrap();

        sync_native_fixture_inner(SyncArgs {
            template: root.join("views/main.crepus"),
            dir: root.clone(),
            out: vec![root.join("desktop/dashboard.view-ir.json")],
            no_defaults: true,
            component: None,
            ctx: None,
            vars: vec!["name=Acme".into()],
            pretty: true,
        })
        .unwrap();

        let desktop_fixture =
            fs::read_to_string(root.join("desktop/dashboard.view-ir.json")).unwrap();
        assert!(!root.join("fixture.json").exists());
        assert!(desktop_fixture.contains("Hello Acme"));
    }

    #[test]
    fn template_files_present() {
        // Smoke-test: every embedded file is non-empty so we know `include_str!`
        // is wired correctly to existing template files.
        for (rel, content) in TEMPLATE_FILES {
            assert!(!content.is_empty(), "empty template content at {rel}");
        }
        assert!(!TEMPLATE_FILES.iter().any(|(rel, content)| {
            rel.contains("btleplug")
                || rel.contains("gedgygedgy")
                || content.contains("android.permission.BLUETOOTH_SCAN")
                || content.contains("CoreBluetooth")
        }));
        assert!(TEMPLATE_FILES.iter().any(|(rel, _)| {
            *rel == "android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt"
        }));
        assert!(TEMPLATE_FILES
            .iter()
            .any(|(rel, _)| { *rel == "ios/Sources/NativeShell/CrepusRustActions.swift" }));
    }

    #[test]
    fn template_files_have_unique_paths() {
        use std::collections::BTreeSet;
        let mut seen = BTreeSet::new();
        for (rel, _) in TEMPLATE_FILES {
            assert!(seen.insert(*rel), "duplicate template entry: {rel}");
        }
    }
}
