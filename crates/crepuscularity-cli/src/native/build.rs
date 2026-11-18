use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::ValueEnum;
use console::style;

use super::ir::{
    codegen_native_source_inner, sync_native_fixture_inner, CodegenArgs, CodegenPlatform, SyncArgs,
};
use super::scaffold::{scaffold_share_extension, ShareExtensionPlatform};
use super::{capitalize_ascii, prepend_kotlin_package};
use crate::cli::{NativeBuildCommands, NativeExtensionCommands, NativeRunCommands};
use crate::ui;

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum IosBuildTarget {
    Simulator,
    Device,
}

pub fn handle_extension(extension: NativeExtensionCommands) {
    match extension {
        NativeExtensionCommands::IosShare { dir, name } => {
            scaffold_share_extension(&dir, &name, ShareExtensionPlatform::Ios)
        }
        NativeExtensionCommands::MacosShare { dir, name } => {
            scaffold_share_extension(&dir, &name, ShareExtensionPlatform::Macos)
        }
    }
}

pub fn handle_build(platform: NativeBuildCommands) {
    match platform {
        NativeBuildCommands::Ios {
            dir,
            target,
            configuration,
        } => build_ios(
            &dir.unwrap_or_else(default_native_dir),
            target,
            &configuration,
        ),
        NativeBuildCommands::Android { dir, flavor } => {
            build_android(&dir.unwrap_or_else(default_native_dir), &flavor)
        }
    }
}

pub fn handle_run(platform: NativeRunCommands) {
    match platform {
        NativeRunCommands::Ios { dir } => {
            run_ios_help(&dir.unwrap_or_else(default_native_dir));
        }
        NativeRunCommands::Android { dir, flavor } => {
            run_android(&dir.unwrap_or_else(default_native_dir), &flavor);
        }
    }
}

pub fn default_native_dir() -> PathBuf {
    PathBuf::from(".")
}

impl IosBuildTarget {
    fn sdk(self) -> &'static str {
        match self {
            Self::Simulator => "iphonesimulator",
            Self::Device => "iphoneos",
        }
    }

    fn destination(self) -> &'static str {
        match self {
            Self::Simulator => "generic/platform=iOS Simulator",
            Self::Device => "generic/platform=iOS",
        }
    }
}

pub struct MobileIosConfig {
    pub(crate) scheme: String,
    pub(crate) bundle_id: String,
    pub(crate) development_team: Option<String>,
    pub(crate) code_sign_style: Option<String>,
    pub(crate) allow_provisioning_updates: bool,
}

pub struct MobileAndroidConfig {
    pub(crate) application_id: Option<String>,
    pub(crate) namespace: Option<String>,
}

pub fn build_ios(dir: &Path, target: IosBuildTarget, configuration: &str) {
    let ios_dir = dir.join("ios");
    if ios_dir.join("project.yml").exists() {
        build_ios_app(&ios_dir, target, configuration);
        return;
    }
    if !ios_dir.join("Package.swift").exists() {
        ui::error(&format!(
            "no Package.swift at '{}'. Pass --dir <path-to-scaffold-root> if the project lives elsewhere.",
            ios_dir.display()
        ));
    }
    let mut cmd = Command::new("swift");
    cmd.arg("build").current_dir(&ios_dir);
    if configuration == "Release" {
        cmd.args(["-c", "release"]);
    } else {
        cmd.args(["-c", "debug"]);
    }
    delegate(cmd, "swift build");
}

pub fn build_ios_app(ios_dir: &Path, target: IosBuildTarget, configuration: &str) {
    let spec = ios_dir.join("project.yml");
    if !spec.exists() {
        ui::error(&format!("no project.yml at '{}'", spec.display()));
    }
    if let Some(root) = ios_dir.parent() {
        sync_default_mobile_artifacts(root, true, false);
    }
    let cfg = load_mobile_ios_config(ios_dir);
    let mut xcodegen = Command::new("xcodegen");
    xcodegen
        .current_dir(ios_dir)
        .args(["generate", "--spec", "project.yml"]);
    delegate(xcodegen, "xcodegen generate");

    let project = ios_dir.join(format!("{}.xcodeproj", cfg.scheme));
    let project_name = if project.exists() {
        project
    } else {
        find_xcodeproj(ios_dir).unwrap_or_else(|| {
            ui::error(&format!(
                "no .xcodeproj generated in '{}'",
                ios_dir.display()
            ));
        })
    };
    let mut build = Command::new("xcodebuild");
    build.current_dir(ios_dir).args([
        "-project",
        project_name
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("CrepusMobileApp.xcodeproj"),
        "-target",
        &cfg.scheme,
        "-sdk",
        target.sdk(),
        "-configuration",
        configuration,
        "-destination",
        target.destination(),
        "build",
        "SYMROOT=build",
    ]);
    build.arg(format!("PRODUCT_BUNDLE_IDENTIFIER={}", cfg.bundle_id));
    if cfg.allow_provisioning_updates {
        build.arg("-allowProvisioningUpdates");
    }
    if let Some(development_team) = &cfg.development_team {
        build.arg(format!("DEVELOPMENT_TEAM={development_team}"));
    }
    if let Some(code_sign_style) = &cfg.code_sign_style {
        build.arg(format!("CODE_SIGN_STYLE={code_sign_style}"));
    }
    delegate(build, "xcodebuild");
}

pub fn build_android(dir: &Path, flavor: &str) {
    sync_default_mobile_artifacts(dir, false, true);
    let android_dir = dir.join("android");
    apply_android_config(&android_dir, &load_mobile_android_config(dir));
    let gradlew = android_dir.join("gradlew");
    if !android_dir.join("settings.gradle.kts").exists() {
        ui::error(&format!(
            "no settings.gradle.kts at '{}'. Pass --dir <path-to-scaffold-root> if the project lives elsewhere.",
            android_dir.display()
        ));
    }

    let task = format!(":app:assemble{}", capitalize_ascii(flavor));
    let mut cmd = if gradlew.exists() {
        let mut c = Command::new("./gradlew");
        c.current_dir(&android_dir);
        c.arg(&task);
        c
    } else {
        // Fall back to system `gradle`. Print a hint either way so users know
        // why we're not invoking ./gradlew.
        eprintln!(
            "{} no ./gradlew at {}; using system `gradle` (run `gradle wrapper --gradle-version 8.10` to generate the wrapper)",
            style("note:").yellow(),
            gradlew.display()
        );
        let mut c = Command::new("gradle");
        c.current_dir(&android_dir);
        c.arg(&task);
        c
    };
    configure_gradle_java(&mut cmd);
    cmd.arg("--quiet"); // don't drown the user in gradle log spam
    delegate(cmd, "gradle build");
}

pub fn run_ios_help(dir: &Path) {
    let ios_dir = dir.join("ios");
    if !ios_dir.join("project.yml").exists() {
        eprintln!(
            "{}",
            style("crepus native run ios — open in Xcode").cyan().bold()
        );
        eprintln!();
        eprintln!("  open {dir}/ios/Package.swift", dir = dir.display());
        eprintln!();
        eprintln!(
            "{} SwiftPM-only scaffold has no installable app target.",
            style("note:").yellow()
        );
        return;
    }
    build_ios_app(&ios_dir, IosBuildTarget::Simulator, "Debug");
    run_ios_app(&ios_dir);
}

pub fn run_ios_app(ios_dir: &Path) {
    let cfg = load_mobile_ios_config(ios_dir);
    let app = find_built_ios_app(ios_dir, &cfg).unwrap_or_else(|| {
        ui::error(&format!(
            "no built .app under '{}'",
            ios_dir.join("build").display()
        ));
    });
    let device = booted_or_available_ios_device().unwrap_or_else(|| {
        ui::error("no available iOS simulator device found");
    });
    let _ = Command::new("xcrun")
        .args(["simctl", "boot", &device])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let bootstatus = Command::new("xcrun")
        .args(["simctl", "bootstatus", &device, "-b"])
        .status()
        .unwrap_or_else(|e| ui::error(&format!("simctl bootstatus: {e}")));
    if !bootstatus.success() {
        ui::error("iOS simulator failed to boot");
    }
    let install = Command::new("xcrun")
        .args(["simctl", "install", &device])
        .arg(&app)
        .status()
        .unwrap_or_else(|e| ui::error(&format!("simctl install: {e}")));
    if !install.success() {
        ui::error("simctl install failed");
    }
    let launch = Command::new("xcrun")
        .args(["simctl", "launch", &device, &cfg.bundle_id])
        .status()
        .unwrap_or_else(|e| ui::error(&format!("simctl launch: {e}")));
    if !launch.success() {
        ui::error("simctl launch failed");
    }
    eprintln!(
        "{} installed and launched {} on {}",
        style("ios:").green(),
        app.display(),
        device
    );
}

pub fn load_mobile_ios_config(ios_dir: &Path) -> MobileIosConfig {
    let toml = fs::read_to_string(ios_dir.join("crepus.toml")).unwrap_or_default();
    let root_toml = ios_dir
        .parent()
        .map(|root| fs::read_to_string(root.join("crepus.toml")).unwrap_or_default())
        .unwrap_or_default();
    let project = fs::read_to_string(ios_dir.join("project.yml")).unwrap_or_default();
    let root_manifest = crate::crepus_toml::CrepusManifest::parse(&root_toml)
        .ok()
        .and_then(|manifest| manifest.ios);
    let manifest = crate::crepus_toml::CrepusManifest::parse(&toml)
        .ok()
        .and_then(|manifest| manifest.ios);
    MobileIosConfig {
        scheme: manifest
            .as_ref()
            .map(|ios| ios.scheme.clone())
            .or_else(|| root_manifest.as_ref().map(|ios| ios.scheme.clone()))
            .or_else(|| toml_value(&toml, "scheme"))
            .or_else(|| project_name(&project))
            .unwrap_or_else(|| "CrepusMobileApp".to_string()),
        bundle_id: manifest
            .as_ref()
            .and_then(|ios| ios.bundle_id.clone())
            .or_else(|| root_manifest.as_ref().and_then(|ios| ios.bundle_id.clone()))
            .or_else(|| project_bundle_id(&project))
            .unwrap_or_else(|| "dev.crepuscularity.mobile".to_string()),
        development_team: manifest
            .as_ref()
            .and_then(|ios| ios.development_team.clone())
            .or_else(|| {
                root_manifest
                    .as_ref()
                    .and_then(|ios| ios.development_team.clone())
            }),
        code_sign_style: manifest
            .as_ref()
            .and_then(|ios| ios.code_sign_style.clone())
            .or_else(|| {
                root_manifest
                    .as_ref()
                    .and_then(|ios| ios.code_sign_style.clone())
            }),
        allow_provisioning_updates: manifest
            .as_ref()
            .is_some_and(|ios| ios.allow_provisioning_updates)
            || root_manifest
                .as_ref()
                .is_some_and(|ios| ios.allow_provisioning_updates),
    }
}

pub fn load_mobile_android_config(root: &Path) -> MobileAndroidConfig {
    let toml = fs::read_to_string(root.join("crepus.toml")).unwrap_or_default();
    let manifest = crate::crepus_toml::CrepusManifest::parse(&toml).ok();
    let android = manifest.and_then(|manifest| manifest.android);
    MobileAndroidConfig {
        application_id: android
            .as_ref()
            .and_then(|android| android.application_id.clone()),
        namespace: android.and_then(|android| android.namespace),
    }
}

pub fn apply_android_config(android_dir: &Path, cfg: &MobileAndroidConfig) {
    if cfg.application_id.is_none() && cfg.namespace.is_none() {
        return;
    }
    let gradle_path = android_dir.join("app/build.gradle.kts");
    let Ok(mut gradle) = fs::read_to_string(&gradle_path) else {
        return;
    };
    if let Some(namespace) = &cfg.namespace {
        gradle = replace_kotlin_string_assignment(&gradle, "namespace", namespace);
    }
    if let Some(application_id) = &cfg.application_id {
        gradle = replace_kotlin_string_assignment(&gradle, "applicationId", application_id);
    }
    fs::write(&gradle_path, gradle).unwrap_or_else(|e| {
        ui::error(&format!("failed to write '{}': {e}", gradle_path.display()));
    });
}

pub fn replace_kotlin_string_assignment(src: &str, key: &str, value: &str) -> String {
    src.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(key) {
                let indent_len = line.len() - trimmed.len();
                format!("{}{} = \"{}\"", &line[..indent_len], key, value)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn toml_value(src: &str, key: &str) -> Option<String> {
    src.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(key)?.trim_start();
        let rest = rest.strip_prefix('=')?.trim();
        Some(rest.trim_matches('"').to_string())
    })
}

pub fn project_name(src: &str) -> Option<String> {
    src.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("name:")?.trim();
        if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        }
    })
}

pub fn project_bundle_id(src: &str) -> Option<String> {
    src.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("PRODUCT_BUNDLE_IDENTIFIER:")?.trim();
        if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        }
    })
}

pub fn find_xcodeproj(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "xcodeproj") {
            Some(path)
        } else {
            None
        }
    })
}

pub fn find_built_ios_app(ios_dir: &Path, cfg: &MobileIosConfig) -> Option<PathBuf> {
    let direct = ios_dir
        .join("build/Debug-iphonesimulator")
        .join(format!("{}.app", cfg.scheme));
    if direct.exists() {
        return Some(direct);
    }
    find_app_under(&ios_dir.join("build"))
}

pub fn find_app_under(dir: &Path) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "app") {
            return Some(path.to_path_buf());
        }
    }
    None
}

pub fn booted_or_available_ios_device() -> Option<String> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "available"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut first = None;
    for line in text.lines() {
        if !line.contains("(Booted)") && !line.contains("(Shutdown)") {
            continue;
        }
        let Some(id) = simulator_id_from_line(line) else {
            continue;
        };
        if line.contains("(Booted)") {
            return Some(id);
        }
        if first.is_none() {
            first = Some(id);
        }
    }
    first
}

pub fn simulator_id_from_line(line: &str) -> Option<String> {
    let (_, rest) = line.split_once('(')?;
    let (candidate, _) = rest.split_once(')')?;
    if candidate.chars().filter(|c| *c == '-').count() == 4 {
        Some(candidate.to_string())
    } else {
        None
    }
}

pub fn run_android(dir: &Path, flavor: &str) {
    sync_default_mobile_artifacts(dir, false, true);
    let android_dir = dir.join("android");
    let gradlew = android_dir.join("gradlew");
    let task = format!(":app:install{}", capitalize_ascii(flavor));
    let application_id = load_android_application_id(&android_dir)
        .unwrap_or_else(|| "dev.crepuscularity.nativeshell".to_string());

    let mut cmd = if gradlew.exists() {
        let mut c = Command::new("./gradlew");
        c.current_dir(&android_dir);
        c.arg(&task);
        c
    } else {
        let mut c = Command::new("gradle");
        c.current_dir(&android_dir);
        c.arg(&task);
        c
    };
    configure_gradle_java(&mut cmd);
    cmd.arg("--quiet");
    delegate(cmd, "gradle install");

    let component = android_main_activity_component(&application_id);
    let mut launch = Command::new("adb");
    launch.args(["shell", "am", "start", "-n", &component]);
    delegate(launch, "adb launch");

    eprintln!(
        "\n{} installed and launched {}",
        style("android:").green(),
        component
    );
}

pub fn load_android_application_id(android_dir: &Path) -> Option<String> {
    let gradle = fs::read_to_string(android_dir.join("app/build.gradle.kts")).ok()?;
    gradle_kts_value(&gradle, "applicationId")
}

pub fn android_main_activity_component(application_id: &str) -> String {
    format!("{application_id}/dev.crepuscularity.nativeshell.MainActivity")
}

pub fn gradle_kts_value(src: &str, key: &str) -> Option<String> {
    src.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(key)?.trim_start();
        let rest = rest.strip_prefix('=')?.trim();
        Some(rest.trim_matches('"').to_string())
    })
}

pub fn sync_default_mobile_artifacts(root: &Path, ios: bool, android: bool) {
    let template = root.join("views/main.crepus");
    if !template.exists() {
        return;
    }
    sync_native_fixture_inner(SyncArgs {
        template: template.clone(),
        dir: root.to_path_buf(),
        out: Vec::new(),
        no_defaults: false,
        component: None,
        ctx: None,
        vars: Vec::new(),
        pretty: true,
    })
    .unwrap_or_else(|e| ui::error(&e.to_string()));
    if ios {
        codegen_native_source_inner(CodegenArgs {
            template: Some(template.clone()),
            platform: Some(CodegenPlatform::SwiftUi),
            out: Some(root.join("ios/Sources/NativeShell/Generated")),
            view_name: Some("CrepusGeneratedView".to_string()),
            component: None,
            ctx: None,
            vars: Vec::new(),
        })
        .unwrap_or_else(|e| ui::error(&e.to_string()));
    }
    if android {
        let out_dir =
            root.join("android/app/src/main/java/dev/crepuscularity/nativeshell/generated");
        codegen_native_source_inner(CodegenArgs {
            template: Some(template.clone()),
            platform: Some(CodegenPlatform::Compose),
            out: Some(out_dir.clone()),
            view_name: Some("CrepusGeneratedView".to_string()),
            component: None,
            ctx: None,
            vars: Vec::new(),
        })
        .unwrap_or_else(|e| ui::error(&e.to_string()));
        prepend_kotlin_package(&out_dir.join("CrepusGeneratedView.kt"));
    }
}

pub fn delegate(mut cmd: Command, label: &str) {
    match cmd.status() {
        Ok(status) if status.success() => {}
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => ui::error(&format!(
            "failed to invoke `{label}`: {e}. Is the toolchain installed and on PATH?"
        )),
    }
}

pub fn configure_gradle_java(cmd: &mut Command) {
    match std::env::var("JAVA_HOME") {
        Ok(raw) if PathBuf::from(&raw).join("bin/java").exists() => {}
        _ => {
            if let Some(java_home) = discover_java_home() {
                cmd.env("JAVA_HOME", java_home);
            } else {
                cmd.env_remove("JAVA_HOME");
            }
        }
    }
}

pub fn discover_java_home() -> Option<String> {
    for candidate in [
        "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home",
        "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home",
    ] {
        let path = Path::new(candidate);
        if path.join("bin/java").exists() {
            return Some(candidate.to_string());
        }
    }
    if let Ok(out) = Command::new("/usr/libexec/java_home")
        .args(["-v", "17"])
        .output()
    {
        if out.status.success() {
            let path = String::from_utf8(out.stdout).ok()?;
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let java = fs::canonicalize("/opt/homebrew/bin/java")
        .or_else(|_| fs::canonicalize("/usr/bin/java"))
        .ok()?;
    let home = java.parent()?.parent()?;
    if home.join("bin/java").exists() {
        Some(home.display().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_find_xcodeproj_invalid_dir() {
        let non_existent = Path::new("this_path_does_not_exist_12345");
        assert_eq!(find_xcodeproj(non_existent), None);

        let file_path = Path::new("Cargo.toml");
        assert_eq!(find_xcodeproj(file_path), None);
    }
}
