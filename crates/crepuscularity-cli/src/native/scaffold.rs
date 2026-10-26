use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use console::style;

use crate::scaffold;
use crate::ui;

/// Each entry is `(relative path within the scaffold root, file content)`.
///
/// Embedding via `include_str!` keeps the templates next to the published
/// crate source without an explicit `[package].include` list — `cargo
/// publish` walks the source tree by default and picks them up.
pub const TEMPLATE_FILES: &[(&str, &str)] = &[
    ("README.md", include_str!("../../templates/native/README.md")),
    ("crepus.toml", include_str!("../../templates/native/crepus.toml")),
    (
        "views/main.crepus",
        include_str!("../../templates/native/views/main.crepus"),
    ),
    ("fixture.json", include_str!("../../templates/native/fixture.json")),
    ("ios/Package.swift", include_str!("../../templates/native/ios/Package.swift")),
    (
        "ios/project.yml",
        include_str!("../../templates/native/ios/project.yml"),
    ),
    (
        "ios/crepus.toml",
        include_str!("../../templates/native/ios/crepus.toml"),
    ),
    (
        "ios/App/CrepusMobileApp.swift",
        include_str!("../../templates/native/ios/App/CrepusMobileApp.swift"),
    ),
    (
        "ios/App/ContentView.swift",
        include_str!("../../templates/native/ios/App/ContentView.swift"),
    ),
    (
        "ios/Sources/NativeShell/CrepusRustActions.swift",
        include_str!("../../templates/native/ios/Sources/NativeShell/CrepusRustActions.swift"),
    ),
    (
        "ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift",
        include_str!(
            "../../templates/native/ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift"
        ),
    ),
    (
        "android/build.gradle.kts",
        include_str!("../../templates/native/android/build.gradle.kts"),
    ),
    (
        "android/settings.gradle.kts",
        include_str!("../../templates/native/android/settings.gradle.kts"),
    ),
    (
        "android/gradle.properties",
        include_str!("../../templates/native/android/gradle.properties"),
    ),
    (
        "android/gradle/wrapper/gradle-wrapper.properties",
        include_str!("../../templates/native/android/gradle/wrapper/gradle-wrapper.properties"),
    ),
    (
        "android/app/build.gradle.kts",
        include_str!("../../templates/native/android/app/build.gradle.kts"),
    ),
    (
        "android/app/src/main/AndroidManifest.xml",
        include_str!("../../templates/native/android/app/src/main/AndroidManifest.xml"),
    ),
    (
        "android/app/src/main/res/values/themes.xml",
        include_str!("../../templates/native/android/app/src/main/res/values/themes.xml"),
    ),
    (
        "android/app/src/main/assets/fixture.json",
        include_str!("../../templates/native/android/app/src/main/assets/fixture.json"),
    ),
    (
        "android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt",
        include_str!(
            "../../templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt"
        ),
    ),
    (
        "android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt",
        include_str!(
            "../../templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt"
        ),
    ),
    (
        "android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt",
        include_str!(
            "../../templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt"
        ),
    ),
    (
        "rust/Cargo.toml",
        include_str!("../../templates/native/rust/Cargo.toml.template"),
    ),
    (
        "rust/src/lib.rs",
        include_str!("../../templates/native/rust/src/lib.rs"),
    ),
];

pub fn scaffold_native_app(name: &str) {
    let root = PathBuf::from(name);
    if root.exists() {
        ui::error(&format!(
            "destination '{}' already exists; pick a fresh name or remove it first",
            root.display()
        ));
    }

    scaffold::create_project_dir(&root).unwrap_or_else(|e| ui::error(&e));

    for (rel, content) in TEMPLATE_FILES {
        let target = root.join(rel);
        scaffold::write_file(&target, content).unwrap_or_else(|e| {
            ui::error(&format!("failed to write '{}': {e}", target.display()));
        });
    }

    let gitignore = "# Build outputs and IDE caches kept out of source control.\n\
                     ios/.build/\n\
                     ios/build/\n\
                     ios/*.xcodeproj/\n\
                     ios/*.xcworkspace/\n\
                     ios/xcuserdata/\n\
                     android/.gradle/\n\
                     android/build/\n\
                     android/app/build/\n\
                     android/local.properties\n\
                     .idea/\n\
                     *.iml\n";
    scaffold::write_file(&root.join(".gitignore"), gitignore)
        .unwrap_or_else(|e| ui::error(&format!("failed to write .gitignore: {e}")));

    ui::success(&format!(
        "scaffolded native app '{}' at '{}'",
        name,
        root.display()
    ));
    eprintln!();
    eprintln!("{}", style("Next steps").dim());
    eprintln!("  iOS:     crepus mobile build --platform ios --dir {name} --target simulator");
    eprintln!(
        "  Android: cd {dir}/android && gradle wrapper --gradle-version 8.10 && \\\n           ./gradlew :app:assembleDebug",
        dir = name
    );
    eprintln!(
        "  Build via crepus: crepus native build ios --dir {dir} --target simulator",
        dir = name
    );
    eprintln!(
        "                    crepus native build android --dir {dir}",
        dir = name
    );
}

pub fn scaffold_native_app_at(root: &Path) -> Result<(), String> {
    if root.exists() {
        return Err(format!("destination '{}' already exists", root.display()));
    }
    scaffold::create_project_dir(root)?;
    for (rel, content) in TEMPLATE_FILES {
        let target = root.join(rel);
        scaffold::write_file(&target, content)?;
    }
    scaffold::write_file(
        &root.join(".gitignore"),
        "ios/.build/\nandroid/.gradle/\nandroid/build/\nandroid/app/build/\n",
    )
}

pub const IOS_SHARE_INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSExtension</key>
	<dict>
		<key>NSExtensionPointIdentifier</key>
		<string>com.apple.share-services</string>
		<key>NSExtensionPrincipalClass</key>
		<string>$(PRODUCT_MODULE_NAME).ShareViewController</string>
	</dict>
</dict>
</plist>
"#;

pub const IOS_SHARE_VIEW_CONTROLLER: &str = r#"import NativeShell
import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        Task {
            let payload = await Self.payload(from: extensionContext)
            let data = try? JSONSerialization.data(withJSONObject: [
                "action": "share.receive",
                "value": payload,
            ])
            if let data, let json = String(data: data, encoding: .utf8) {
                _ = CrepusRustActions.dispatchStored(json)
            }
            extensionContext?.completeRequest(returningItems: nil)
        }
    }

    private static func payload(from context: NSExtensionContext?) async -> [String: Any] {
        var items: [[String: String]] = []
        for case let item as NSExtensionItem in context?.inputItems ?? [] {
            for provider in item.attachments ?? [] {
                if let text = await loadString(provider, .plainText) {
                    items.append(["kind": "text", "value": text])
                } else if let url = await loadString(provider, .url) {
                    items.append(["kind": "url", "value": url])
                }
            }
        }
        return ["items": items]
    }

    private static func loadString(_ provider: NSItemProvider, _ type: UTType) async -> String? {
        guard provider.hasItemConformingToTypeIdentifier(type.identifier) else {
            return nil
        }
        return await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: type.identifier) { item, _ in
                if let text = item as? String {
                    continuation.resume(returning: text)
                } else if let url = item as? URL {
                    continuation.resume(returning: url.absoluteString)
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }
}
"#;

pub const MACOS_SHARE_VIEW_CONTROLLER: &str = r#"import AppKit
import NativeShell
import UniformTypeIdentifiers

final class ShareViewController: NSViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        Task {
            let payload = await Self.payload(from: extensionContext)
            let data = try? JSONSerialization.data(withJSONObject: [
                "action": "share.receive",
                "value": payload,
            ])
            if let data, let json = String(data: data, encoding: .utf8) {
                _ = CrepusRustActions.dispatchStored(json)
            }
            extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
        }
    }

    private static func payload(from context: NSExtensionContext?) async -> [String: Any] {
        var items: [[String: String]] = []
        for case let item as NSExtensionItem in context?.inputItems ?? [] {
            for provider in item.attachments ?? [] {
                if let text = await loadString(provider, .plainText) {
                    items.append(["kind": "text", "value": text])
                } else if let url = await loadString(provider, .url) {
                    items.append(["kind": "url", "value": url])
                }
            }
        }
        return ["items": items]
    }

    private static func loadString(_ provider: NSItemProvider, _ type: UTType) async -> String? {
        guard provider.hasItemConformingToTypeIdentifier(type.identifier) else {
            return nil
        }
        return await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: type.identifier) { item, _ in
                if let text = item as? String {
                    continuation.resume(returning: text)
                } else if let url = item as? URL {
                    continuation.resume(returning: url.absoluteString)
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }
}
"#;

#[derive(Clone, Copy)]
pub enum ShareExtensionPlatform {
    Ios,
    Macos,
}

impl ShareExtensionPlatform {
    fn xcode_platform(self) -> &'static str {
        match self {
            Self::Ios => "iOS",
            Self::Macos => "macOS",
        }
    }

    fn source_dir(self) -> &'static str {
        match self {
            Self::Ios => "ShareExtension",
            Self::Macos => "MacShareExtension",
        }
    }

    fn controller(self) -> &'static str {
        match self {
            Self::Ios => IOS_SHARE_VIEW_CONTROLLER,
            Self::Macos => MACOS_SHARE_VIEW_CONTROLLER,
        }
    }

    fn rust_target_script(self) -> &'static str {
        match self {
            Self::Ios => {
                "if [ \"${PLATFORM_NAME:-iphonesimulator}\" = \"iphoneos\" ]; then\n            rust_target=aarch64-apple-ios\n          else\n            rust_target=aarch64-apple-ios-sim\n          fi"
            }
            Self::Macos => {
                "case \"${CURRENT_ARCH:-$(uname -m)}\" in\n            arm64) rust_target=aarch64-apple-darwin ;;\n            x86_64) rust_target=x86_64-apple-darwin ;;\n            *) echo \"unsupported macOS arch: ${CURRENT_ARCH:-$(uname -m)}\" >&2; exit 1 ;;\n          esac"
            }
        }
    }

    fn library_search_paths(self) -> &'static str {
        match self {
            Self::Ios => {
                "        LIBRARY_SEARCH_PATHS[sdk=iphoneos*]: \"$(PROJECT_DIR)/build/rust/aarch64-apple-ios\"\n        LIBRARY_SEARCH_PATHS[sdk=iphonesimulator*]: \"$(PROJECT_DIR)/build/rust/aarch64-apple-ios-sim\""
            }
            Self::Macos => {
                "        LIBRARY_SEARCH_PATHS: \"$(PROJECT_DIR)/build/rust/aarch64-apple-darwin $(PROJECT_DIR)/build/rust/x86_64-apple-darwin\""
            }
        }
    }

    fn output_files(self) -> &'static str {
        match self {
            Self::Ios => {
                "          - \"$(PROJECT_DIR)/build/rust/aarch64-apple-ios/libcrepus_mobile_actions.a\"\n          - \"$(PROJECT_DIR)/build/rust/aarch64-apple-ios-sim/libcrepus_mobile_actions.a\""
            }
            Self::Macos => {
                "          - \"$(PROJECT_DIR)/build/rust/aarch64-apple-darwin/libcrepus_mobile_actions.a\"\n          - \"$(PROJECT_DIR)/build/rust/x86_64-apple-darwin/libcrepus_mobile_actions.a\""
            }
        }
    }
}

pub fn scaffold_share_extension(dir: &Path, name: &str, platform: ShareExtensionPlatform) {
    let ios_dir = dir.join("ios");
    if !ios_dir.is_dir() {
        ui::error(&format!(
            "{} is not a native scaffold with ios/",
            dir.display()
        ));
    }
    let extension_dir = ios_dir.join(platform.source_dir());
    fs::create_dir_all(&extension_dir).unwrap_or_else(|e| {
        ui::error(&format!(
            "failed to create '{}': {e}",
            extension_dir.display()
        ));
    });
    write_new_file(&extension_dir.join("Info.plist"), IOS_SHARE_INFO_PLIST);
    write_new_file(
        &extension_dir.join("ShareViewController.swift"),
        platform.controller(),
    );
    let project = ios_dir.join("project.yml");
    let mut text = fs::read_to_string(&project)
        .unwrap_or_else(|e| ui::error(&format!("read {}: {e}", project.display())));
    // shortcut: scaffold-style root mappings; other authored YAML styles need a parser.
    let mut targets = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if !line.starts_with([' ', '\t', '#']) && !line.trim().is_empty() {
            if let Some((start, _)) = targets {
                targets = Some((start, offset));
                break;
            }
            if line.strip_prefix("targets:").is_some_and(|suffix| {
                suffix.trim().is_empty() || suffix.trim_start().starts_with('#')
            }) {
                targets = Some((offset + line.len(), text.len()));
            }
        }
        offset += line.len();
    }
    let (start, end) =
        targets.unwrap_or_else(|| ui::error("project.yml needs a top-level targets mapping"));
    let mut headers = vec![format!("  {}:", share_extension_target_key(name))];
    // shortcut: legacy plain keys only; use a YAML parser when admitting other authored key styles.
    let legacy_plain = !name.is_empty()
        && name.trim() == name
        && !name.starts_with([
            '#', '"', '\'', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '%', '@', '`',
        ])
        && !name.contains(" #")
        && !name.contains(": ")
        && !name.starts_with("- ")
        && !name.starts_with("? ")
        && !name.starts_with(": ")
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'));
    if legacy_plain {
        headers.push(format!("  {name}:"));
    }
    let exists = text[start..end].lines().any(|line| {
        headers.iter().any(|header| {
            line.strip_prefix(header).is_some_and(|suffix| {
                suffix.trim().is_empty() || suffix.trim_start().starts_with('#')
            })
        })
    });
    if !exists {
        text.insert_str(
            end,
            &share_extension_target(name, &main_bundle_id(&text), platform),
        );
        fs::write(&project, text)
            .unwrap_or_else(|e| ui::error(&format!("write {}: {e}", project.display())));
    }
    ui::success(&format!(
        "added {} share extension target '{name}'",
        platform.xcode_platform()
    ));
}

pub fn write_new_file(path: &Path, content: &str) {
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return,
        Err(e) => ui::error(&format!("failed to create '{}': {e}", path.display())),
    };
    file.write_all(content.as_bytes())
        .unwrap_or_else(|e| ui::error(&format!("failed to write '{}': {e}", path.display())));
}

pub fn main_bundle_id(project_yml: &str) -> String {
    project_yml
        .lines()
        .find_map(|line| line.trim().strip_prefix("PRODUCT_BUNDLE_IDENTIFIER: "))
        .unwrap_or("dev.crepuscularity.mobile")
        .trim_matches('"')
        .to_string()
}

fn share_extension_target_key(name: &str) -> String {
    let mut key = serde_json::to_string(name).expect("target name string");
    // YAML normalizes literal Unicode line breaks and forbids raw C1 controls.
    for character in name
        .chars()
        .filter(|c| matches!(c, '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}'))
    {
        key = key.replace(character, &format!("\\u{:04x}", character as u32));
    }
    key
}

pub fn share_extension_target(
    name: &str,
    main_bundle_id: &str,
    platform: ShareExtensionPlatform,
) -> String {
    let name = share_extension_target_key(name);
    format!(
        "\n  {name}:\n    type: app-extension\n    platform: {xcode_platform}\n    sources:\n      - path: {source_dir}\n    settings:\n      base:\n{library_search_paths}\n        OTHER_LDFLAGS: \"-lcrepus_mobile_actions\"\n        EXCLUDED_ARCHS[sdk=iphonesimulator*]: \"x86_64\"\n        INFOPLIST_FILE: {source_dir}/Info.plist\n        PRODUCT_BUNDLE_IDENTIFIER: {main_bundle_id}.share\n    dependencies:\n      - package: NativeShell\n        product: NativeShell\n    preBuildScripts:\n      - name: Build Rust Actions\n        outputFiles:\n{output_files}\n        script: |\n          set -euo pipefail\n          export PATH=\"$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"\n          {rust_target_script}\n          rustup target add \"$rust_target\" >/dev/null\n          cargo_profile=debug\n          if [ \"${{CONFIGURATION:-Debug}}\" = \"Release\" ]; then\n            cargo_profile=release\n          fi\n          cargo build --manifest-path \"$PROJECT_DIR/../rust/Cargo.toml\" \\\n            --target \"$rust_target\" \\\n            $([ \"$cargo_profile\" = release ] && echo \"--release\") \\\n            --no-default-features\n          mkdir -p \"$PROJECT_DIR/build/rust/$rust_target\"\n          cp \"$PROJECT_DIR/../rust/target/$rust_target/$cargo_profile/libcrepus_mobile_actions.a\" \"$PROJECT_DIR/build/rust/$rust_target/\"\n",
        xcode_platform = platform.xcode_platform(),
        source_dir = platform.source_dir(),
        library_search_paths = platform.library_search_paths(),
        output_files = platform.output_files(),
        rust_target_script = platform.rust_target_script(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scaffold_native_app_at() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("my_app");

        // First run: should succeed
        assert!(scaffold_native_app_at(&root).is_ok());

        // Check that .gitignore was created with correct contents
        let gitignore_path = root.join(".gitignore");
        assert!(gitignore_path.exists());
        let gitignore_contents = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(
            gitignore_contents,
            "ios/.build/\nandroid/.gradle/\nandroid/build/\nandroid/app/build/\n"
        );

        // Check that all template files were created
        for (rel, _content) in TEMPLATE_FILES {
            let target = root.join(rel);
            assert!(target.exists(), "template file {} missing", rel);
        }

        // Second run: should fail because destination already exists
        let err = scaffold_native_app_at(&root).err().unwrap();
        assert!(err.contains("already exists"));
    }
}

#[cfg(test)]
mod new_file_tests {
    use super::*;

    #[test]
    fn new_files_preserve_existing_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("controller.swift");
        write_new_file(&path, "original");
        write_new_file(&path, "replacement");
        assert_eq!(fs::read_to_string(path).unwrap(), "original");
    }

    #[cfg(unix)]
    #[test]
    fn new_files_do_not_follow_dangling_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("outside.swift");
        let path = dir.path().join("controller.swift");
        std::os::unix::fs::symlink(&destination, &path).unwrap();
        write_new_file(&path, "replacement");
        assert!(!destination.exists());
        assert_eq!(fs::read_link(path).unwrap(), destination);
    }
}
