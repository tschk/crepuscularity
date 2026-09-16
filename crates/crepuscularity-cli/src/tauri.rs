use std::fs;
use std::path::Path;

use crepuscularity_native::{generate_native_source, to_json_pretty, NativeCodegenTarget};
use crepuscularity_tauri::TauriProject;

use crate::cli::{TauriCommands, TauriTargetArg};

pub fn execute(command: TauriCommands) {
    match command {
        TauriCommands::Audit { dir } => audit(&dir).unwrap_or_else(|e| crate::ui::error(&e)),
        TauriCommands::Convert { dir, out, target } => {
            convert(&dir, &out, target).unwrap_or_else(|e| crate::ui::error(&e))
        }
    }
}

fn audit(dir: &Path) -> Result<(), String> {
    let report = TauriProject::open(dir)?.audit();
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    report.native_ready()
}

fn convert(dir: &Path, out: &Path, target: TauriTargetArg) -> Result<(), String> {
    let project = TauriProject::open(dir)?;
    project.audit().native_ready()?;
    let metadata = project.metadata();
    let windows = project.windows();
    let bundle = project.bundle()?;
    let webview = bundle.files.values().any(|source| {
        source.contains("adapter=\"webview\"")
            || source.contains("adapter='webview'")
            || source.contains("<iframe")
            || source
                .lines()
                .any(|line| line.trim_start().starts_with("iframe "))
    });
    if webview && target == TauriTargetArg::Desktop {
        return Err("WebView components cannot target desktop: GPUI has no WebView host".into());
    }
    let desktop = target != TauriTargetArg::Mobile && !webview;
    if webview && target == TauriTargetArg::All {
        crate::ui::warning("WebView components omit GPUI desktop output; generated iOS and Android projects embed them natively");
    }
    let ir = project.native_ir()?;
    let fixture = to_json_pretty(&ir).map_err(|e| e.to_string())?;
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "CrepusGeneratedView");
    let kotlin = format!(
        "package dev.crepuscularity.nativeshell\n\n{}",
        generate_native_source(&ir, NativeCodegenTarget::Compose, "CrepusGeneratedView")
    );
    let staging = out.with_extension(format!("crepus-tmp-{}", std::process::id()));
    if staging.exists() {
        return Err(format!(
            "staging directory '{}' already exists",
            staging.display()
        ));
    }
    let result = (|| {
        crate::native::scaffold_native_app_at(&staging)?;
        apply_metadata(&staging, &metadata)?;
        for (path, source) in &bundle.files {
            let destination = staging.join("views").join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(destination, source).map_err(|e| e.to_string())?;
            if desktop {
                let destination = staging.join("desktop/views").join(path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(destination, source).map_err(|e| e.to_string())?;
            }
        }
        fs::write(
            staging.join("views/main.crepus"),
            format!("include {}\n", bundle.entry),
        )
        .map_err(|e| e.to_string())?;
        if desktop {
            fs::write(
                staging.join("desktop/views/main.crepus"),
                format!("include {}\n", bundle.entry),
            )
            .map_err(|e| e.to_string())?;
            write_desktop_project(&staging.join("desktop"), &windows)?;
        }
        fs::write(staging.join("fixture.json"), &fixture).map_err(|e| e.to_string())?;
        if target != TauriTargetArg::Desktop {
            fs::write(
                staging.join("android/app/src/main/assets/fixture.json"),
                &fixture,
            )
            .map_err(|e| e.to_string())?;
            fs::write(
                staging.join("ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift"),
                swift,
            )
            .map_err(|e| e.to_string())?;
            fs::write(
            staging.join(
                "android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt",
            ),
            kotlin,
        )
        .map_err(|e| e.to_string())?;
        }
        if target == TauriTargetArg::Desktop {
            fs::remove_dir_all(staging.join("ios")).map_err(|e| e.to_string())?;
            fs::remove_dir_all(staging.join("android")).map_err(|e| e.to_string())?;
            fs::remove_dir_all(staging.join("rust")).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, out).map_err(|e| e.to_string())?;
    println!(
        "converted Tauri {:?} project to {}",
        project.version(),
        out.display()
    );
    Ok(())
}

fn apply_metadata(
    root: &Path,
    metadata: &crepuscularity_tauri::TauriMetadata,
) -> Result<(), String> {
    let product_name = metadata
        .window_title
        .as_deref()
        .or(metadata.product_name.as_deref())
        .unwrap_or("NativeShell");
    replace(
        root.join("android/app/src/main/AndroidManifest.xml"),
        "android:label=\"NativeShell\"",
        &format!("android:label=\"{product_name}\""),
    )?;
    if let Some(identifier) = &metadata.identifier {
        replace(
            root.join("android/app/build.gradle.kts"),
            "applicationId = \"dev.crepuscularity.nativeshell\"",
            &format!("applicationId = \"{identifier}\""),
        )?;
        replace(
            root.join("ios/project.yml"),
            "PRODUCT_BUNDLE_IDENTIFIER: dev.crepuscularity.mobile",
            &format!("PRODUCT_BUNDLE_IDENTIFIER: {identifier}"),
        )?;
    }
    if let Some(version) = &metadata.version {
        replace(
            root.join("android/app/build.gradle.kts"),
            "versionName = \"1.0\"",
            &format!("versionName = \"{version}\""),
        )?;
    }
    Ok(())
}

fn write_desktop_project(
    root: &Path,
    windows: &[crepuscularity_tauri::TauriWindowSpec],
) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"crepus-tauri-desktop\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ngpui = {{ package = \"gpui-ce\", version = \"0.2.2\", default-features = false }}\n\n[target.'cfg(target_os = \"macos\")'.dependencies]\ncrepuscularity-gpui = {{ version = \"0.5.10\", features = [\"macos\"] }}\n\n[target.'cfg(target_os = \"linux\")'.dependencies]\ncrepuscularity-gpui = {{ version = \"0.5.10\", features = [\"x11\"] }}\n\n[target.'cfg(target_os = \"windows\")'.dependencies]\ncrepuscularity-gpui = {{ version = \"0.5.10\", features = [\"windows\"] }}\n",
    )
    .map_err(|e| e.to_string())?;
    let opens = windows
        .iter()
        .map(|window| {
            let bounds = match (window.width, window.height) {
                (Some(width), Some(height)) => format!(
                    "Some(WindowBounds::Windowed(bounds(point(px(0.), px(0.)), size(px({width}.), px({height}.)))))"
                ),
                _ => "None".into(),
            };
            format!(
                "cx.open_window(gpui_window_options({:?}, {:?}, {bounds}, None), |_window, cx| cx.new(|_| CrepusView)).expect(\"open configured window\");",
                window.label, window.title,
            )
        })
        .collect::<Vec<_>>()
        .join("\n        ");
    fs::write(
        root.join("src/main.rs"),
        format!(
            "use crepuscularity_gpui::prelude::*;\nuse crepuscularity_gpui::{{bounds, point, size, view_file, WindowBounds}};\n\nstruct CrepusView;\n\nimpl Render for CrepusView {{\n    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {{\n        view_file!(\"views/main.crepus\")\n    }}\n}}\n\nfn main() {{\n    crepuscularity_gpui::application().run(|cx: &mut App| {{\n        {opens}\n    }});\n}}\n"
        ),
    )
    .map_err(|e| e.to_string())
}

fn replace(path: impl AsRef<Path>, from: &str, to: &str) -> Result<(), String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    fs::write(path, source.replace(from, to)).map_err(|e| e.to_string())
}
