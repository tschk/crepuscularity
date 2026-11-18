//! `crepus ios` — XcodeGen + SwiftPM **NativeShell** host app scaffolding (View IR demos).
//!
//! Requires [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`) for
//! `crepus ios generate` / `crepus ios build`.
//!
//! **`crepus.toml`** at the app root stores `[ios]` (`scheme`, `destination`).
//! `generate` / `build` walk up from the current directory until they find it (or `--dir` pins the root).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use console::style;

use crate::build_options::BuildOptions;
use crate::cli::IosCommands;
use crate::crepus_toml;
use crate::new::to_pascal_case;
use crate::scaffold;
use crate::ui;

#[derive(Debug, Clone)]
struct IosSection {
    scheme: String,
    ios_destination: String,
    bundle_id: Option<String>,
    development_team: Option<String>,
    code_sign_style: Option<String>,
    allow_provisioning_updates: bool,
}

pub fn execute(cmd: IosCommands) {
    match cmd {
        IosCommands::New { name } => scaffold_ios_app(&name),
        IosCommands::Generate { dir, spec } => {
            let (root, cfg) = resolve_ios_root_and_config(&dir);
            let _ = spec;
            generate_project(&root, &cfg);
        }
        IosCommands::Build {
            build,
            dir,
            spec,
            scheme,
            destination,
        } => {
            let options = build.into_options_or_exit();
            let (root, cfg) = resolve_ios_root_and_config(&dir);
            let _ = spec;
            let scheme = scheme.unwrap_or(cfg.scheme.clone());
            let destination = destination.unwrap_or(cfg.ios_destination.clone());
            generate_project(&root, &cfg);
            run_xcodebuild(&root, &scheme, &destination, &cfg, options);
        }
    }
}

/// If `explicit_dir` is set, resolve config only under that path. Otherwise walk up from cwd.
fn resolve_ios_root_and_config(explicit_dir: &Option<PathBuf>) -> (PathBuf, IosSection) {
    if let Some(root) = explicit_dir {
        let root = normalize_root(root);
        if let Some(cfg) = load_ios_config(&root) {
            return (root, cfg);
        }
        if let Some(cfg) = legacy_config_from_project_yml(&root) {
            return (root, cfg);
        }
        ui::error(&format!(
            "no crepus.toml [ios] or project.yml in {}",
            root.display()
        ));
    }

    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some((root, cfg)) = walk_up_for_ios_config(&start) {
        return (root, cfg);
    }

    ui::error(
        "no crepus.toml with [ios] found (or project.yml legacy) — run from inside the app, a parent folder, or pass --dir PATH.\n\
         Scaffold: crepus ios new my-app && cd my-app",
    );
}

fn normalize_root(p: &Path) -> PathBuf {
    if p.as_os_str().is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    }
}

/// Walk from `start` (file or dir) upward looking for `crepus.toml` with `[ios]`, else `project.yml`.
fn walk_up_for_ios_config(start: &Path) -> Option<(PathBuf, IosSection)> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if let Some(cfg) = load_ios_config(&dir) {
            return Some((dir, cfg));
        }
        if let Some(cfg) = legacy_config_from_project_yml(&dir) {
            return Some((dir, cfg));
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn load_ios_config(root: &Path) -> Option<IosSection> {
    let p = root.join("crepus.toml");
    let i = crepus_toml::try_load_ios(&p)?;
    Some(IosSection {
        scheme: i.scheme,
        ios_destination: i.destination,
        bundle_id: i.bundle_id,
        development_team: i.development_team,
        code_sign_style: i.code_sign_style,
        allow_provisioning_updates: i.allow_provisioning_updates,
    })
}

/// Older trees: only `project.yml` with leading `name:`
fn legacy_config_from_project_yml(root: &Path) -> Option<IosSection> {
    let yml = fs::read_to_string(root.join("project.yml")).ok()?;
    let name_line = yml.lines().find(|l| {
        let t = l.trim_start();
        t.starts_with("name:") && !t.contains("PRODUCT_BUNDLE_IDENTIFIER")
    })?;
    let rest = name_line.trim_start().strip_prefix("name:")?.trim();
    let scheme = rest.split_whitespace().next()?.to_string();
    Some(IosSection {
        scheme,
        ios_destination: crepus_toml::default_ios_destination(),
        bundle_id: None,
        development_team: None,
        code_sign_style: None,
        allow_provisioning_updates: false,
    })
}

fn scaffold_ios_app(name: &str) {
    let t0 = Instant::now();
    let dir = Path::new(name);
    if dir.exists() {
        ui::error(&format!("'{}' already exists", name));
    }

    let pascal = to_pascal_case(name);
    let app_target = app_target_name(&pascal);

    let native = dir.join("NativeShell");
    let sources = native.join("Sources").join("NativeShell");

    scaffold::ensure_dir(&sources).unwrap_or_else(|e| ui::error(&format!("create dirs: {e}")));
    scaffold::ensure_dir(&dir.join("App"))
        .unwrap_or_else(|e| ui::error(&format!("create App: {e}")));

    scaffold::write_file(&dir.join("crepus.toml"), &crepus_toml(&app_target))
        .unwrap_or_else(|e| ui::error(&format!("write crepus.toml: {e}")));
    scaffold::write_file(&dir.join(".gitignore"), &ios_gitignore())
        .unwrap_or_else(|e| ui::error(&format!("write .gitignore: {e}")));
    scaffold::write_file(&dir.join("README.md"), &readme_md(name, &app_target))
        .unwrap_or_else(|e| ui::error(&format!("write README: {e}")));

    scaffold::write_file(&native.join("Package.swift"), &native_package_swift())
        .unwrap_or_else(|e| ui::error(&format!("write Package.swift: {e}")));
    scaffold::write_file(
        &sources.join("ViewIrModels.swift"),
        include_str!("../assets/ios/ViewIrModels.swift"),
    )
    .unwrap_or_else(|e| ui::error(&format!("write ViewIrModels: {e}")));
    scaffold::write_file(
        &sources.join("ViewIrTreeView.swift"),
        include_str!("../assets/ios/ViewIrTreeView.swift"),
    )
    .unwrap_or_else(|e| ui::error(&format!("write ViewIrTreeView: {e}")));
    scaffold::write_file(
        &sources.join("fixture.json"),
        include_str!("../assets/ios/fixture.json"),
    )
    .unwrap_or_else(|e| ui::error(&format!("write fixture.json: {e}")));

    scaffold::write_file(&dir.join("App").join("App.swift"), &app_swift(&pascal))
        .unwrap_or_else(|e| ui::error(&format!("write App.swift: {e}")));
    scaffold::write_file(
        &dir.join("App").join("ContentView.swift"),
        &content_view_swift(),
    )
    .unwrap_or_else(|e| ui::error(&format!("write ContentView: {e}")));

    eprintln!(
        "\n{} {}",
        ui::ok(),
        style(format!("ios scaffold `{name}`")).cyan().bold()
    );
    eprintln!();
    eprintln!("{}", style("Next:").dim());
    eprintln!("  cd {name}");
    eprintln!("  crepus ios generate");
    eprintln!("  open {app_target}.xcodeproj");
    eprintln!();
    eprintln!(
        "{}",
        style("Or from this directory: crepus ios build").dim()
    );
    ui::done_in(t0.elapsed());
}

fn app_target_name(pascal: &str) -> String {
    if pascal == "NativeShell" {
        "NativeShellHostApp".to_string()
    } else {
        format!("{pascal}App")
    }
}

fn crepus_toml(app_target: &str) -> String {
    format!(
        r#"# Crepuscularity — `crepus ios generate` / `crepus ios build` read this (walks up from cwd).
[ios]
scheme = "{app_target}"
destination = "platform=iOS Simulator,name=iPhone 16,OS=latest"
"#
    )
}

fn native_package_swift() -> String {
    r#"// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "NativeShell",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "NativeShell", targets: ["NativeShell"]),
    ],
    targets: [
        .target(
            name: "NativeShell",
            path: "Sources/NativeShell",
            resources: [.copy("fixture.json")]
        ),
    ]
)
"#
    .to_string()
}

fn app_swift(pascal: &str) -> String {
    format!(
        r#"import SwiftUI

@main
struct {pascal}App: App {{
    var body: some Scene {{
        WindowGroup {{
            ContentView()
        }}
    }}
}}
"#
    )
}

fn content_view_swift() -> String {
    r#"import SwiftUI
import NativeShell

struct ContentView: View {
    var body: some View {
        FixtureRootView()
    }
}
"#
    .to_string()
}

fn ios_gitignore() -> String {
    r#"DerivedData/
.build/
xcuserdata/
*.xcuserstate
*.xcodeproj/
*.xcworkspace/
.DS_Store
"#
    .to_string()
}

fn readme_md(name: &str, app_target: &str) -> String {
    format!(
        r#"# {name} (Crepuscularity iOS shell)

Generated with `crepus ios new {name}`.

- **Crepus** produces `{app_target}.xcodeproj` from `crepus.toml`.
- **NativeShell** is a local Swift package that decodes View IR JSON (`fixture.json`).

## Commands

`crepus.toml` stores the Xcode scheme and simulator destination; run commands from this directory (or any subfolder).

```bash
crepus ios generate
open {app_target}.xcodeproj
```

Build from the CLI:

```bash
crepus ios build
```

Replace `fixture.json` under `NativeShell/Sources/NativeShell/` when you change templates; rebuild the app.

"#
    )
}

fn generate_project(dir: &Path, cfg: &IosSection) {
    let bundle_id = cfg.bundle_id.as_deref().unwrap_or("dev.crepuscularity.app");
    crate::apple_project::generate(
        dir,
        crate::apple_project::Config {
            name: &cfg.scheme,
            bundle_id,
            platform: crate::apple_project::Platform::Ios,
            deployment_target: "17.0",
            package_path: "NativeShell",
            package_product: "NativeShell",
        },
    )
    .unwrap_or_else(|e| ui::error(&e));
    eprintln!("{}", style("project: ok").green());
}

fn run_xcodebuild(
    dir: &Path,
    scheme: &str,
    destination: &str,
    cfg: &IosSection,
    options: BuildOptions,
) {
    let proj = find_xcodeproj(dir).unwrap_or_else(|| {
        ui::error(&format!(
            "no .xcodeproj in {} — run `crepus ios generate` first",
            dir.display()
        ));
    });

    let proj_name = proj
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| ui::error("invalid .xcodeproj filename"));

    let mut cmd = Command::new("xcodebuild");
    cmd.current_dir(dir).args([
        "-project",
        proj_name,
        "-scheme",
        scheme,
        "-destination",
        destination,
        "-configuration",
        if options.release() {
            "Release"
        } else {
            "Debug"
        },
    ]);
    if cfg.allow_provisioning_updates {
        cmd.arg("-allowProvisioningUpdates");
    }
    if let Some(bundle_id) = &cfg.bundle_id {
        cmd.arg(format!("PRODUCT_BUNDLE_IDENTIFIER={bundle_id}"));
    }
    if let Some(development_team) = &cfg.development_team {
        cmd.arg(format!("DEVELOPMENT_TEAM={development_team}"));
    }
    if let Some(code_sign_style) = &cfg.code_sign_style {
        cmd.arg(format!("CODE_SIGN_STYLE={code_sign_style}"));
    }
    let status = cmd
        .arg("build")
        .status()
        .unwrap_or_else(|e| ui::error(&format!("xcodebuild: {e}")));

    if !status.success() {
        ui::error("xcodebuild failed");
    }
    eprintln!("{}", style("xcodebuild: ok").green());
}

fn find_xcodeproj(dir: &Path) -> Option<PathBuf> {
    let rd = fs::read_dir(dir).ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().is_some_and(|e| e == "xcodeproj") {
            return Some(p);
        }
    }
    None
}
