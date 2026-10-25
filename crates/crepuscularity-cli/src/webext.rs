//! Browser extension commands for crepus CLI.

use console::style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::build_options::BuildOptions;
use crate::cli::WebextCommands;
use crate::dispatch::browser_target;
use crate::scaffold;
use crate::ui;
use crate::wasm_bundle::{
    cargo_build_wasm32, find_wasm_file, run_wasm_bindgen, run_wasm_opt, wasm_profile_dirs,
    WasmOptStatus,
};
use crepuscularity_core::watch::COOLDOWN_MS;
use crepuscularity_core::{DriverCache, Fingerprint};
use crepuscularity_webext::BrowserTarget;

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn execute(cmd: WebextCommands) {
    match cmd {
        WebextCommands::New { name } => scaffold_extension(&name),
        WebextCommands::Build {
            build,
            app,
            browser,
        } => {
            let app_path = app_path_or_cwd(app);
            let options = build.into_options_or_exit();
            build_extension(&app_path, false, browser_target(browser), options);
        }
        WebextCommands::Dev {
            build,
            app,
            browser,
        } => {
            let app_path = app_path_or_cwd(app);
            let options = build.into_options_or_exit();
            let loaded = load_extension_manifest(&app_path, None);
            let browsers = selected_browsers(&loaded.config, browser_target(browser));
            if browsers.len() != 1 {
                ui::error("crepus webext dev needs one browser; pass --browser");
            }
            let browser = browsers[0];
            build_extension_with_manifest(
                &app_path,
                loaded.manifest,
                &loaded.config,
                true,
                browser,
                options,
            );
            watch_and_reload(&app_path, browser, options);
        }
        WebextCommands::Manifest { app, browser } => {
            print_manifest(&app_path_or_cwd(app), browser_target(browser));
        }
    }
}

fn app_path_or_cwd(app: Option<PathBuf>) -> PathBuf {
    app.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|e| {
            ui::error(&format!("cannot determine current directory: {e}"));
        })
    })
}

// ── scaffold ─────────────────────────────────────────────────────────────────

const WEBEXT_CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "{{slug}}_runtime"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
crepuscularity-webext = { version = "0.2.7" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde-wasm-bindgen = "0.6"
wasm-bindgen = "0.2"
"#;

const WEBEXT_LIB_RS: &str = r##"use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn runtime_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn popup_main() {}

#[wasm_bindgen]
pub fn options_main() {}

#[wasm_bindgen]
pub fn content_main() {}

#[wasm_bindgen]
pub fn background_main() {}

#[wasm_bindgen]
pub fn settings_seed() -> Result<(), JsValue> {
    Ok(())
}

#[wasm_bindgen]
pub fn handle_background_message(message: JsValue) -> Result<JsValue, JsValue> {
    let message_json: serde_json::Value =
        serde_wasm_bindgen::from_value(message).unwrap_or(serde_json::Value::Null);
    let result = serde_json::json!({
        "ok": true,
        "echo": message_json,
    });
    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
"##;

const WEBEXT_UI_CREPUS: &str = r#"+++
[Popup.defaults]
title = "Extension"
description = ""
+++

--- Popup
div flex flex-col gap-4 p-4
  div text-xl font-bold
    "{title}"
  div text-sm text-zinc-500
    "{description}"
"#;

fn scaffold_extension(name: &str) {
    let t0 = Instant::now();

    let slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();

    let base = PathBuf::from(&slug);
    if base.exists() {
        ui::error(&format!("directory already exists: {slug}"));
    }

    scaffold::ensure_dir(&base.join("runtime/src"))
        .unwrap_or_else(|e| ui::error(&format!("create runtime/src dir: {e}")));
    scaffold::ensure_dir(&base.join("views"))
        .unwrap_or_else(|e| ui::error(&format!("create views dir: {e}")));

    scaffold::write_file(&base.join("crepus.toml"), &scaffold_crepus_toml(name))
        .unwrap_or_else(|e| ui::error(&format!("write crepus.toml: {e}")));

    scaffold::write_template(
        &base.join("runtime/Cargo.toml"),
        WEBEXT_CARGO_TOML_TEMPLATE,
        &[("{{slug}}", &slug)],
    )
    .unwrap_or_else(|e| ui::error(&format!("write runtime/Cargo.toml: {e}")));

    scaffold::write_file(&base.join("runtime/src/lib.rs"), WEBEXT_LIB_RS)
        .unwrap_or_else(|e| ui::error(&format!("write runtime/src/lib.rs: {e}")));

    scaffold::write_file(&base.join("views/ui.crepus"), WEBEXT_UI_CREPUS)
        .unwrap_or_else(|e| ui::error(&format!("write views/ui.crepus: {e}")));

    let steps = [
        format!("cd {slug}"),
        "crepus build".to_string(),
        format!(
            "{}",
            style("# Load dist/unpacked/ in chrome://extensions").dim()
        ),
    ];
    scaffold::scaffold_success(
        &slug,
        &base,
        &steps.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    ui::done_in(t0.elapsed());
}

fn scaffold_crepus_toml(name: &str) -> String {
    format!(
        r#"[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["chromium"]

[targets.extension]
name = "{name}"
version = "0.1.0"
description = "A browser extension built with crepuscularity"

[targets.capabilities]
storage = true
background-script = true
content-script = true
host-permissions = ["https://example.com/*"]
"#
    )
}

// ── auto-detect ──────────────────────────────────────────────────────────────

fn auto_detect_pages(app_path: &Path, manifest: &mut crepuscularity_webext::ExtensionManifest) {
    let pages_dir = app_path.join("pages");
    let opts = &mut manifest.options;

    // action_popup: look for popup.crepus or action.crepus
    if opts.action_popup.is_none() {
        for stem in ["popup", "action"] {
            let p = pages_dir.join(format!("{stem}.crepus"));
            if p.exists() {
                opts.action_popup = Some(format!("pages/{stem}.crepus"));
                break;
            }
        }
    }

    // options_ui: look for options.crepus
    if opts.options_ui.is_none() {
        let p = pages_dir.join("options.crepus");
        if p.exists() {
            opts.options_ui = Some(crepuscularity_webext::OptionsUiSpec {
                page: "pages/options.crepus".to_string(),
                browser_style: Some(false),
                open_in_tab: Some(true),
            });
        }
    }

    // chrome_url_overrides: look for new-tab.crepus
    if manifest.chrome_url_overrides.is_empty() {
        let p = pages_dir.join("new-tab.crepus");
        if p.exists() {
            manifest
                .chrome_url_overrides
                .insert("newtab".to_string(), "pages/new-tab.crepus".to_string());
        }
    }
}

fn auto_detect_icons(app_path: &Path, manifest: &mut crepuscularity_webext::ExtensionManifest) {
    let icons_dir = app_path.join("icons");
    if !icons_dir.is_dir() {
        return;
    }

    let needs_icons = manifest.options.icons.is_empty();
    let needs_action_icons = manifest.options.action_icons.is_empty();

    if !needs_icons && !needs_action_icons {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            // Match icon{size}.png (not action_* icons)
            if needs_icons && name.starts_with("icon") && !name.contains("action") {
                if let Some(size) = detect_icon_size(&name) {
                    manifest.options.icons.insert(size, format!("icons/{name}"));
                }
            }

            // Match action_disabled_* files
            if needs_action_icons && name.contains("action_disabled") {
                if let Some(size) = detect_icon_size(&name) {
                    manifest
                        .options
                        .action_icons
                        .insert(size, format!("icons/{name}"));
                }
            }
        }
    }
}

fn detect_icon_size(filename: &str) -> Option<String> {
    // Extract digits from filename: icon128.png → "128", action_disabled_16.png → "16"
    let digits: String = filename.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

// ── build ─────────────────────────────────────────────────────────────────────

fn build_extension(
    app_path: &Path,
    dev: bool,
    browser: Option<BrowserTarget>,
    options: BuildOptions,
) {
    let loaded = load_extension_manifest(app_path, None);
    for browser in selected_browsers(&loaded.config, browser) {
        build_extension_with_manifest(
            app_path,
            loaded.manifest.clone(),
            &loaded.config,
            dev,
            browser,
            options,
        );
    }
}

fn build_extension_with_manifest(
    app_path: &Path,
    mut manifest: crepuscularity_webext::ExtensionManifest,
    config: &crate::crepus_toml::WebextTargetConfig,
    dev: bool,
    browser: Option<BrowserTarget>,
    options: BuildOptions,
) {
    let t0 = Instant::now();
    build_extension_inner(app_path, &mut manifest, config, dev, t0, browser, options);
}

fn build_extension_inner(
    app_path: &Path,
    manifest: &mut crepuscularity_webext::ExtensionManifest,
    config: &crate::crepus_toml::WebextTargetConfig,
    dev: bool,
    t0: Instant,
    browser: Option<BrowserTarget>,
    options: BuildOptions,
) {
    // Auto-detect options from project structure
    auto_detect_pages(app_path, manifest);
    auto_detect_icons(app_path, manifest);

    let ext_name = style(manifest.extension.name.as_str()).cyan().bold();
    let label = if options.release() {
        "crepus webext release"
    } else {
        "crepus webext debug"
    };
    eprintln!("{} building {ext_name}", style(label).dim());
    eprintln!();

    let dist = match browser {
        Some(target) => app_path.join("dist").join(target.dist_dir()),
        None => app_path.join("dist/unpacked"),
    };
    let src_dir = dist.join("src");
    let vendor_dir = dist.join("vendor");
    std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
        ui::error(&format!("create src/ dir: {e}"));
    });
    std::fs::create_dir_all(&vendor_dir).unwrap_or_else(|e| {
        ui::error(&format!("create vendor/ dir: {e}"));
    });

    // ── Step 1: manifest.json ────────────────────────────────────────────────
    let manifest_json = generate_manifest_json(manifest, browser, dev);
    write_manifest_json(&dist, &manifest_json);

    // ── Step 2: runtime assets ───────────────────────────────────────────────
    write_runtime_assets(app_path, &dist, &src_dir, &vendor_dir, dev, manifest);

    // ── Step 3: WASM runtime ─────────────────────────────────────────────────
    build_wasm_runtime_if_exists(app_path, &vendor_dir, options);

    // ── Step 4: pre-render popup HTML ────────────────────────────────────────
    prerender_and_write_popup_html(app_path, &src_dir, manifest);

    // ── Capability check ────────────────────────────────────────────────────
    report_missing_capabilities(app_path, manifest);

    if browser == Some(BrowserTarget::Safari) && !dev {
        package_safari(app_path, &dist, manifest, config.safari.as_ref());
    }

    eprintln!(
        "\n{} built to {}",
        ui::ok(),
        style(dist.display().to_string()).cyan()
    );
    match browser {
        Some(BrowserTarget::Firefox) => eprintln!(
            "  {} load {} in {}",
            ui::dim("→"),
            style(dist.join("manifest.json").display().to_string()).underlined(),
            style("about:debugging#/runtime/this-firefox").cyan()
        ),
        Some(BrowserTarget::Safari) => eprintln!(
            "  {} packaged {}",
            ui::dim("→"),
            style(
                config
                    .safari
                    .as_ref()
                    .and_then(|safari| safari.project_location.as_deref())
                    .unwrap_or("dist/safari-app")
            )
            .underlined(),
        ),
        _ => eprintln!(
            "  {} load {} in {}",
            ui::dim("→"),
            style(dist.display().to_string()).underlined(),
            style("chrome://extensions").cyan()
        ),
    }
    ui::done_in(t0.elapsed());
}

fn generate_manifest_json(
    manifest: &crepuscularity_webext::ExtensionManifest,
    browser: Option<BrowserTarget>,
    dev: bool,
) -> String {
    let mut json = match browser {
        Some(target) => manifest.to_manifest_v3_json_for_browser(target),
        None => manifest.to_manifest_v3_json(),
    };
    if dev {
        json = inject_dev_content_script(&json);
    }
    json
}

fn write_manifest_json(dist: &Path, json: &str) {
    let sp = ui::spinner("generating manifest.json");
    std::fs::write(dist.join("manifest.json"), json).unwrap_or_else(|e| {
        ui::error(&format!("write manifest.json: {e}"));
    });
    ui::spinner_ok(&sp, "manifest.json");
}

fn build_wasm_runtime_if_exists(app_path: &Path, vendor_dir: &Path, options: BuildOptions) {
    let runtime_dir = app_path.join("runtime");
    if runtime_dir.exists() {
        build_wasm_runtime(app_path, &runtime_dir, vendor_dir, options);
    } else {
        ui::warning("no runtime/ directory — skipping WASM compile");
        ui::warning("run `crepus webext new` to scaffold a full project");
    }
}

fn prerender_and_write_popup_html(
    app_path: &Path,
    src_dir: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
) {
    let popup_template = app_path.join("views/popup.crepus");
    if popup_template.exists() {
        let sp = ui::spinner("pre-rendering popup.html");
        match prerender_popup_html(app_path, &popup_template, manifest) {
            Ok(html) => {
                std::fs::write(src_dir.join("popup.html"), &html).unwrap_or_else(|e| {
                    ui::error(&format!("write popup.html: {e}"));
                });
                ui::spinner_ok(&sp, "popup.html (pre-rendered from popup.crepus)");
            }
            Err(e) => {
                sp.finish_and_clear();
                ui::warning(&format!("popup pre-render failed: {e}"));
            }
        }
    }
}

fn report_missing_capabilities(
    app_path: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
) {
    match crepuscularity_webext::check_project_capabilities_with_manifest(app_path, manifest) {
        Ok(missing) if !missing.is_empty() => {
            eprintln!();
            ui::warning("missing capabilities detected — add these to crepus.toml:");
            for cap in &missing {
                eprintln!(
                    "  {} {}",
                    ui::dim("→"),
                    style(cap.to_permission_string()).yellow()
                );
            }
        }
        Err(e) => {
            ui::warning(&format!("capability scan failed: {e}"));
        }
        _ => {}
    }
}

fn selected_browsers(
    config: &crate::crepus_toml::WebextTargetConfig,
    browser: Option<BrowserTarget>,
) -> Vec<Option<BrowserTarget>> {
    if let Some(browser) = browser {
        return vec![Some(browser)];
    }
    if config.browsers.is_empty() {
        return vec![None];
    }
    config
        .browsers
        .iter()
        .map(|browser| {
            BrowserTarget::parse(browser).unwrap_or_else(|| {
                ui::error(&format!(
                    "unsupported webext browser {browser:?}; expected chromium, firefox, or safari"
                ))
            })
        })
        .map(Some)
        .collect()
}

fn package_safari(
    app_path: &Path,
    dist: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
    safari: Option<&crate::crepus_toml::SafariTomlSection>,
) {
    if !cfg!(target_os = "macos") {
        ui::error("Safari packaging requires macOS with Xcode");
    }
    let safari = safari
        .unwrap_or_else(|| ui::error("Safari builds need [targets.safari] with bundle_identifier"));
    let project = safari
        .project_location
        .as_deref()
        .map(|path| app_path.join(path))
        .unwrap_or_else(|| app_path.join("dist/safari-app"));
    if project.exists() {
        ui::error(&format!(
            "Safari project already exists at {}; remove it or set targets.safari.project_location",
            project.display()
        ));
    }
    let app_name = safari
        .app_name
        .as_deref()
        .unwrap_or(manifest.extension.name.as_str());
    let args = safari_packager_args(
        dist,
        &project,
        app_name,
        &safari.bundle_identifier,
        &safari.platforms,
    );
    let output = Command::new("xcrun")
        .args(args)
        .output()
        .unwrap_or_else(|_| ui::error("Safari packaging requires macOS with Xcode"));
    if !output.status.success() {
        ui::error(&format!(
            "Safari packaging failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
}

fn safari_packager_args(
    dist: &Path,
    project: &Path,
    app_name: &str,
    bundle_identifier: &str,
    platforms: &[String],
) -> Vec<String> {
    let mut args = vec![
        "safari-web-extension-packager".to_string(),
        "--project-location".to_string(),
        project.display().to_string(),
        "--app-name".to_string(),
        app_name.to_string(),
        "--bundle-identifier".to_string(),
        bundle_identifier.to_string(),
        "--swift".to_string(),
        "--copy-resources".to_string(),
        "--no-open".to_string(),
        "--no-prompt".to_string(),
    ];
    match platforms {
        [] => {}
        [macos, ios] if macos == "macos" && ios == "ios" => {}
        [ios] if ios == "ios" => args.push("--ios-only".to_string()),
        [macos] if macos == "macos" => args.push("--macos-only".to_string()),
        _ => ui::error(
            "targets.safari.platforms must be [\"macos\"], [\"ios\"], or [\"macos\", \"ios\"]",
        ),
    }
    args.push(dist.display().to_string());
    args
}

fn write_runtime_assets(
    app_path: &Path,
    dist: &Path,
    src_dir: &Path,
    vendor_dir: &Path,
    dev: bool,
    manifest: &crepuscularity_webext::ExtensionManifest,
) {
    let sp = ui::spinner("writing runtime assets");
    use crepuscularity_webext::extension_assets as a;

    macro_rules! w {
        ($name:expr, $path:expr, $content:expr) => {
            std::fs::write($path, $content).unwrap_or_else(|e| {
                ui::error(&format!("write {}: {e}", $name));
            });
        };
    }

    w!("popup.html", src_dir.join("popup.html"), a::POPUP_HTML);
    w!("popup.css", src_dir.join("popup.css"), a::POPUP_CSS);
    w!("popup.js", src_dir.join("popup.js"), a::POPUP_JS);
    w!("options.js", src_dir.join("options.js"), a::OPTIONS_JS);
    w!(
        "background.js",
        src_dir.join("background.js"),
        a::BACKGROUND_JS
    );
    let custom_bg = app_path.join("src/background.js");
    if custom_bg.exists() {
        std::fs::copy(&custom_bg, src_dir.join("background.js")).unwrap_or_else(|e| {
            ui::error(&format!("copy custom background.js: {e}"));
        });
    }
    w!("content.js", src_dir.join("content.js"), a::CONTENT_JS);
    w!("content.css", src_dir.join("content.css"), a::CONTENT_CSS);
    w!(
        "browser-shim.js",
        src_dir.join("browser-shim.js"),
        a::BROWSER_SHIM
    );
    w!(
        "runtime-as-adapter.js",
        src_dir.join("runtime-as-adapter.js"),
        a::RUNTIME_ADAPTER
    );
    if dev {
        w!("dev.js", src_dir.join("dev.js"), a::DEV_JS);
    }
    w!("unocss.js", vendor_dir.join("unocss.js"), a::UNOCSS_JS);
    copy_app_assets(app_path, dist).unwrap_or_else(|e| {
        ui::error(&format!("copy app assets: {e}"));
    });
    render_crepus_css_assets(app_path, dist).unwrap_or_else(|e| {
        ui::error(&format!("render extension .css.crepus assets: {e}"));
    });
    render_crepus_pages(app_path, dist, manifest).unwrap_or_else(|e| {
        ui::error(&format!("render extension .crepus pages: {e}"));
    });
    ui::spinner_ok(&sp, "runtime assets");
}

struct LoadedExtensionManifest {
    manifest: crepuscularity_webext::ExtensionManifest,
    config: crate::crepus_toml::WebextTargetConfig,
}

fn load_extension_manifest(app_path: &Path, target_id: Option<&str>) -> LoadedExtensionManifest {
    let crepus_toml = app_path.join("crepus.toml");
    if crepus_toml.is_file() {
        let raw = std::fs::read_to_string(&crepus_toml)
            .unwrap_or_else(|e| ui::error(&format!("read {}: {e}", crepus_toml.display())));
        let manifest = crate::crepus_toml::CrepusManifest::parse(&raw)
            .unwrap_or_else(|e| ui::error(&format!("parse {}: {e}", crepus_toml.display())));
        let targets = manifest.resolved_targets(app_path);
        let webext: Vec<_> = targets
            .into_iter()
            .filter(|target| target.target_type == "webext")
            .filter(|target| target_id.map(|id| target.id == id).unwrap_or(true))
            .collect();
        match webext.as_slice() {
            [target] => {
                return LoadedExtensionManifest {
                    manifest: target.webext.clone().unwrap_or_else(|| {
                        ui::error(&format!(
                            "webext target {:?} in {} needs [targets.extension]",
                            target.id,
                            crepus_toml.display()
                        ))
                    }),
                    config: target.webext_config.clone(),
                };
            }
            [] if target_id.is_some() => ui::error(&format!(
                "no webext target {:?} in {}",
                target_id.unwrap(),
                crepus_toml.display()
            )),
            [] => {}
            many => ui::error(&format!(
                "{} defines {} webext targets; pass --target ID",
                crepus_toml.display(),
                many.len()
            )),
        }
    }

    ui::error(&format!(
        "no crepus.toml webext target found in {}",
        app_path.display()
    ));
}

pub(crate) fn build_app_path(app_path: &Path, options: BuildOptions) {
    build_extension(app_path, false, None, options);
}

pub(crate) fn build_app_target(
    app_path: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
    config: &crate::crepus_toml::WebextTargetConfig,
    options: BuildOptions,
) {
    for browser in selected_browsers(config, None) {
        build_extension_with_manifest(app_path, manifest.clone(), config, false, browser, options);
    }
}

fn build_wasm_runtime(
    app_path: &Path,
    runtime_dir: &Path,
    vendor_dir: &Path,
    options: BuildOptions,
) {
    if !compile_wasm_runtime_step(runtime_dir, options) {
        return;
    }

    let Some(wasm_path) = find_built_wasm_step(app_path, runtime_dir, options) else {
        return;
    };

    run_wasm_bindgen_step(&wasm_path, vendor_dir);
    optimize_wasm_step(vendor_dir, options);
}

fn compile_wasm_runtime_step(runtime_dir: &Path, options: BuildOptions) -> bool {
    let sp = ui::spinner("compiling WASM runtime");
    match cargo_build_wasm32(runtime_dir, options) {
        Ok(()) => {
            ui::spinner_ok(&sp, "WASM compiled");
            true
        }
        Err(stderr) => {
            sp.finish_and_clear();
            eprintln!("  {} WASM compile failed", ui::err());
            for line in stderr.lines().take(15) {
                eprintln!("    {}", style(line).dim());
            }
            if stderr.lines().count() > 15 {
                eprintln!(
                    "    {}",
                    style("... (run cargo build manually for full output)").dim()
                );
            }
            false
        }
    }
}

fn find_built_wasm_step(
    app_path: &Path,
    runtime_dir: &Path,
    options: BuildOptions,
) -> Option<PathBuf> {
    let (workspace_target, local_target) = wasm_profile_dirs(app_path, runtime_dir, options);
    let wasm_file = find_wasm_file(&workspace_target).or_else(|| find_wasm_file(&local_target));

    if wasm_file.is_none() {
        ui::warning(&format!(
            "built .wasm not found in target/wasm32-unknown-unknown/{}/",
            options.cargo_profile()
        ));
    }

    wasm_file
}

fn run_wasm_bindgen_step(wasm_path: &Path, vendor_dir: &Path) {
    let sp = ui::spinner("running wasm-bindgen");
    match run_wasm_bindgen(wasm_path, vendor_dir, "runtime") {
        Ok(()) => {
            ui::spinner_ok(&sp, "wasm-bindgen — vendor/runtime.js + runtime_bg.wasm");
        }
        Err(err) => {
            sp.finish_and_clear();
            if err.starts_with("wasm-bindgen:") {
                let _ = std::fs::copy(wasm_path, vendor_dir.join("runtime_bg.wasm"));
                ui::warning("wasm-bindgen not found — copied raw .wasm");
                ui::warning("install: cargo install wasm-bindgen-cli");
            } else {
                eprintln!("  {} wasm-bindgen failed", ui::err());
                for line in err.lines().take(10) {
                    eprintln!("    {}", style(line).dim());
                }
                ui::warning("install wasm-bindgen-cli: cargo install wasm-bindgen-cli");
            }
        }
    }
}

fn optimize_wasm_step(vendor_dir: &Path, options: BuildOptions) {
    if options.optimize_artifacts() {
        let wasm = vendor_dir.join("runtime_bg.wasm");
        if wasm.is_file() {
            let sp = ui::spinner("optimizing WASM");
            match run_wasm_opt(&wasm, options.optimization) {
                Ok(WasmOptStatus::Optimized) => ui::spinner_ok(&sp, "WASM optimized"),
                Ok(WasmOptStatus::NotInstalled) => {
                    sp.finish_and_clear();
                    ui::warning("wasm-opt not found — install Binaryen to optimize WASM");
                }
                Err(err) => {
                    sp.finish_and_clear();
                    ui::warning(&format!("wasm-opt failed: {err}"));
                }
            }
        }
    }
}

/// System prompt text passed as `{system_prompt}` context variable into the
/// popup template. Defined here so it can be kept in sync with the pre-render
/// step without duplicating it in the app's Rust source.
pub const SYSTEM_PROMPT: &str = "\
When creating charts, forms, interactive UI, or visualisations,
wrap output in <ai-anywhere> tags for the anywhere browser extension.

── Format (crepus + data only — no JavaScript) ─────────────
<ai-anywhere type=\"widget\" title=\"Widget Title\">
  <anywhere-ui lang=\"crepus\">
    <!-- crepus template here (see syntax below) -->
  </anywhere-ui>
  <anywhere-data>{\"key\": \"value\"}</anywhere-data>
</ai-anywhere>

ui langs: crepus (preferred) | html
types:    widget | chart | form | visualization | tool
Do NOT use <anywhere-script>. Put all UI in crepus. Put all data in anywhere-data JSON.
Crepus expressions support variables, if/for/match, and simple math — not arrow functions or .filter().
Precompute counts/aggregates in anywhere-data when needed.

── Crepus template syntax ───────────────────────────────────
Indent-based. Children are indented under their parent.

Elements (put ALL HTML attributes on the SAME line as the tag):
  button.primary type=\"button\" data-action=\"toggle\" data-id={id}
    \"Toggle\"
  div card
    \"child text with {variable} interpolation\"

Conditionals:
  if {condition}
    span
      \"yes\"
  else
    span
      \"no\"

Loops:
  for item in {items}
    div row
      \"{item.name}: {item.value}\"

Match:
  match {status}
    \"ok\" =>
      span green
        \"OK\"
    _ =>
      span
        \"unknown\"

Variables:
  $: let total = {count * price}
  $: default title = {\"Untitled\"}

Actions (pass data-* as action payload):
  button primary type=\"button\" data-action=\"submit\" data-id={item.id}
    \"Submit\"

── Crepus example ───────────────────────────────────────────
<ai-anywhere type=\"chart\" title=\"Status\">
  <anywhere-ui lang=\"crepus\">
div dashboard
  h2 title
    \"{title}\"
  for item in {items}
    div row
      span label
        \"{item.label}\"
      span value
        \"{item.value}\"
  </anywhere-ui>
  <anywhere-data>{\"title\":\"Stats\",\"items\":[{\"label\":\"Users\",\"value\":\"42\"},{\"label\":\"Active\",\"value\":\"7\"}]}</anywhere-data>
</ai-anywhere>";

/// Render `views/popup.crepus` with three states (main / help / crepus-ref)
/// and embed all three as sibling `<div>` elements in the returned HTML.
/// The popup opens instantly from the pre-rendered HTML; popup.js still loads
/// runtime WASM and requires `popup_main` for behavior wiring.
fn load_system_prompt(app_path: &Path) -> String {
    for path in [
        app_path.join("resources/system_prompt.txt"),
        app_path.join("system_prompt.txt"),
    ] {
        if path.is_file() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                return text;
            }
        }
    }
    SYSTEM_PROMPT.to_string()
}

fn prerender_popup_html(
    app_path: &Path,
    template_path: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
) -> Result<String, String> {
    use crepuscularity_core::context::TemplateContext;
    use crepuscularity_web::render_from_files;

    let source = std::fs::read_to_string(template_path).map_err(|e| e.to_string())?;
    let mut files = HashMap::new();
    files.insert("popup.crepus".to_string(), source);

    let render = |show_help: bool, show_crepus: bool| -> Result<String, String> {
        let mut ctx = TemplateContext::new();
        ctx.set("enabled", true);
        ctx.set("auto_render", false);
        ctx.set("show_help", show_help);
        ctx.set("show_crepus", show_crepus);
        ctx.set("system_prompt", load_system_prompt(app_path));
        render_from_files(&files, "popup.crepus", &ctx).map_err(|e| e.to_string())
    };

    let main_html = render(false, false)?;
    let help_html = render(true, false)?;
    let crepus_html = render(false, true)?;

    let title = &manifest.extension.name;
    // Inline the CSS so the popup is a single self-contained file with zero
    // external fetches before it renders. UnoCSS is not needed here — the popup
    // uses BEM class names from popup.css, not utility classes.
    let css = crepuscularity_webext::extension_assets::POPUP_CSS;
    Ok(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200">
  <style>{css}</style>
</head>
<body>
  <div id="view-main">{main_html}</div>
  <div id="view-help" hidden>{help_html}</div>
  <div id="view-crepus" hidden>{crepus_html}</div>
  <script type="module" src="./popup.js"></script>
</body>
</html>"#
    ))
}

// ── manifest ──────────────────────────────────────────────────────────────────

fn print_manifest(app_path: &Path, browser: Option<BrowserTarget>) {
    let manifest = load_extension_manifest(app_path, None).manifest;
    match browser {
        Some(target) => println!("{}", manifest.to_manifest_v3_json_for_browser(target)),
        None => println!("{}", manifest.to_manifest_v3_json()),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn copy_app_assets(app_path: &Path, dist: &Path) -> std::io::Result<()> {
    for dir in ["src", "pages", "icons", "resources"] {
        let source = app_path.join(dir);
        if source.is_dir() {
            copy_dir_contents(&source, &dist.join(dir))?;
        }
    }
    Ok(())
}

fn copy_dir_contents(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_contents(&entry.path(), &target)?;
        } else if file_type.is_file() {
            if entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "crepus")
            {
                continue;
            }
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn render_crepus_css_assets(app_path: &Path, dist: &Path) -> Result<(), String> {
    let src_dir = app_path.join("src");
    if !src_dir.is_dir() {
        return Ok(());
    }
    render_crepus_css_assets_in(&src_dir, &src_dir, &dist.join("src"))
}

fn render_crepus_css_assets_in(root: &Path, dir: &Path, out_root: &Path) -> Result<(), String> {
    use crepuscularity_core::preprocess::strip_indent_decorators;

    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            render_crepus_css_assets_in(root, &path, out_root)?;
            continue;
        }
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".css.crepus"))
        {
            continue;
        }
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let output_name = rel
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "invalid css asset name".to_string())?
            .trim_end_matches(".crepus")
            .to_string();
        let output = out_root.join(rel).with_file_name(output_name);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rendered = strip_indent_decorators(&source).inline_css;
        let css = if output.exists() {
            let existing = std::fs::read_to_string(&output).map_err(|e| e.to_string())?;
            if existing.trim().is_empty() {
                rendered
            } else {
                format!("{existing}\n\n{rendered}")
            }
        } else {
            rendered
        };
        std::fs::write(output, css).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn render_crepus_pages(
    app_path: &Path,
    dist: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
) -> Result<(), String> {
    let pages_dir = app_path.join("pages");
    if !pages_dir.is_dir() {
        return Ok(());
    }

    let mut files = HashMap::new();
    let mut entries = Vec::new();
    collect_crepus_pages(&pages_dir, &mut files, &mut entries)?;
    if entries.is_empty() {
        return Ok(());
    }

    let out_dir = dist.join("pages");
    let src_dir = dist.join("src");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;

    let cache = DriverCache::open(app_path);

    for entry in &entries {
        render_crepus_page(entry, &files, &src_dir, &out_dir, manifest, &cache)?;
    }
    Ok(())
}

fn render_crepus_page(
    entry: &str,
    files: &HashMap<String, String>,
    src_dir: &Path,
    out_dir: &Path,
    manifest: &crepuscularity_webext::ExtensionManifest,
    cache: &DriverCache,
) -> Result<(), String> {
    let source = files
        .get(entry)
        .ok_or_else(|| format!("missing page source: {entry}"))?;
    let stem = Path::new(entry)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");
    // WASM exports use underscore, not hyphen
    let fn_name = stem.replace('-', "_");

    let js_vendor_prefix = page_script_prefix(entry);
    let js = format!(
        r#"import init, * as runtime from "{prefix}vendor/runtime.js";
try {{
  const wasmBytes = await fetch("{prefix}vendor/runtime_bg.wasm").then(r => r.arrayBuffer());
  await init({{ module_or_path: wasmBytes }});
  const main = runtime.{fn}_main;
  if (typeof main === "function") {{
    await main();
  }} else {{
    const render = runtime.render_{fn};
    if (typeof render === "function") {{
      const output = await render({{}});
      const root = document.getElementById("root");
      if (root) {{
        const rawHtml = typeof output === "string" ? output : (output?.html ?? output?.get?.("html"));
        const cssText = typeof output === "string" ? "" : (output?.css ?? output?.get?.("css"));
        if (typeof rawHtml === "string") {{
          const css = cssText ? `<style>${{cssText}}</style>` : "";
          const combined = typeof output === "string" ? output : `${{css}}${{rawHtml}}`;
          const parser = new DOMParser();
          const doc = parser.parseFromString(combined, "text/html");

          const scripts = doc.querySelectorAll("script");
          for (let i = 0; i < scripts.length; i++) {{
            scripts[i].remove();
          }}

          const allElements = doc.querySelectorAll("*");
          for (let i = 0; i < allElements.length; i++) {{
            const el = allElements[i];

            if (el.tagName === "IFRAME" || el.tagName === "OBJECT" || el.tagName === "EMBED" || el.tagName === "APPLET" || el.tagName === "MATH" || el.tagName === "SVG" || el.tagName === "META" || el.tagName === "BASE") {{
              el.remove();
              continue;
            }}

            const attrs = el.attributes;
            for (let j = attrs.length - 1; j >= 0; j--) {{
              const attrName = attrs[j].name.toLowerCase();
              const attrValue = attrs[j].value.toLowerCase().trim();
              if (attrName.startsWith("on")) {{
                el.removeAttribute(attrs[j].name);
              }} else if ((attrName === "src" || attrName === "href" || attrName === "data") && (attrValue.startsWith("javascript:") || attrValue.startsWith("data:text/html") || attrValue.startsWith("vbscript:"))) {{
                el.removeAttribute(attrs[j].name);
              }}
            }}
          }}

          const nodes = [];
          if (doc.head) {{
            const children = Array.from(doc.head.childNodes);
            for (let i = 0; i < children.length; i++) {{
              nodes.push(children[i]);
            }}
          }}
          if (doc.body) {{
            const children = Array.from(doc.body.childNodes);
            for (let i = 0; i < children.length; i++) {{
              nodes.push(children[i]);
            }}
          }}
          root.replaceChildren(...nodes);
        }} else {{
          root.textContent = JSON.stringify(output ?? null);
        }}
      }}
    }}
  }}
}} catch (error) {{
  const root = document.getElementById("root");
  if (root) {{
    root.textContent = String(error?.stack ?? error);
  }} else {{
    throw error;
  }}
}}"#,
        prefix = js_vendor_prefix,
        fn = fn_name
    );
    let js_output = src_dir.join(page_script_relative_path(entry));
    if let Some(parent) = js_output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&js_output, &js).map_err(|e| format!("write {}: {e}", js_output.display()))?;

    let html = render_crepus_page_html(source, files, entry, manifest)?;
    let output = out_dir
        .join(entry.trim_end_matches(".crepus"))
        .with_extension("html");

    // Cache skip: avoid re-rendering unchanged page templates.
    let fp = Fingerprint::new(source, Some(entry), "webext-page");
    if output.is_file() && cache.is_up_to_date(&fp, &html) {
        return Ok(());
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(output, &html).map_err(|e| e.to_string())?;
    cache.record(&fp, &html);
    Ok(())
}

fn collect_crepus_pages(
    root: &Path,
    files: &mut HashMap<String, String>,
    entries: &mut Vec<String>,
) -> Result<(), String> {
    use rayon::prelude::*;

    let crepus_files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .collect::<Result<Vec<_>, walkdir::Error>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|e| {
            let path = e.path();
            let is_file_or_link = e.file_type().is_file() || e.file_type().is_symlink();
            is_file_or_link && path.extension().and_then(|ext| ext.to_str()) == Some("crepus")
        })
        .map(|e| e.path().to_owned())
        .collect();

    let results: Result<Vec<(String, String)>, String> = crepus_files
        .into_par_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            Ok((rel, source))
        })
        .collect();

    for (rel, source) in results? {
        entries.push(rel.clone());
        files.insert(rel, source);
    }
    entries.sort();

    Ok(())
}

fn render_crepus_page_html(
    source: &str,
    files: &HashMap<String, String>,
    entry: &str,
    manifest: &crepuscularity_webext::ExtensionManifest,
) -> Result<String, String> {
    use crepuscularity_core::context::TemplateContext;
    use crepuscularity_core::preprocess::{google_fonts_head_markup, strip_indent_decorators};
    use crepuscularity_web::render_from_files;

    let decorators = strip_indent_decorators(source);
    let body =
        render_from_files(files, entry, &TemplateContext::new()).map_err(|e| e.to_string())?;
    let stem = Path::new(entry)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");
    let title_suffix = title_case(stem);
    let title = if title_suffix.is_empty() {
        manifest.extension.name.clone()
    } else {
        format!("{} {}", manifest.extension.name, title_suffix)
    };
    let script_src = page_script_src(entry);
    let script = format!(r#"<script type="module" src="{script_src}"></script>"#);
    let fonts = google_fonts_head_markup(&decorators.google_fonts);
    let style = if decorators.inline_css.is_empty() {
        String::new()
    } else {
        format!("<style>{}</style>", decorators.inline_css)
    };
    Ok(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  {fonts}
  {style}
</head>
<body>
{body}
{script}
</body>
</html>"#
    ))
}

fn inject_dev_content_script(manifest_json: &str) -> String {
    if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(manifest_json) {
        if let Some(scripts) = v.get_mut("content_scripts").and_then(|v| v.as_array_mut()) {
            for entry in scripts {
                if let Some(js) = entry.get_mut("js").and_then(|v| v.as_array_mut()) {
                    js.push(serde_json::Value::String("src/dev.js".to_string()));
                }
            }
        }
        return serde_json::to_string_pretty(&v).unwrap_or_else(|_| manifest_json.to_string());
    }
    manifest_json.to_string()
}

fn watch_and_reload(app_path: &Path, browser: Option<BrowserTarget>, options: BuildOptions) {
    use console::style;
    use notify::Watcher;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc::channel;

    eprintln!();
    eprintln!("  {}", style("watching for changes — Ctrl+C to stop").dim());

    let reload_id = AtomicU64::new(1);
    let src_dir = match browser {
        Some(target) => app_path.join("dist").join(target.dist_dir()).join("src"),
        None => app_path.join("dist/unpacked/src"),
    };
    let _ = std::fs::write(src_dir.join(".reload-id"), "1");

    let (tx, rx) = channel();
    let mut watcher =
        match notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || matches!(event.kind, notify::EventKind::Create(_)) {
                    let _ = tx.send(());
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                ui::warning(&format!("file watcher failed: {e}"));
                return;
            }
        };

    let dirs = [
        "runtime/src",
        "runtime/views",
        "src",
        "pages",
        "views",
        "icons",
        "resources",
    ];
    for dir in &dirs {
        let d = app_path.join(dir);
        if d.exists() {
            let _ = watcher.watch(&d, notify::RecursiveMode::Recursive);
        }
    }
    let _ = watcher.watch(
        app_path.join("crepus.toml").as_path(),
        notify::RecursiveMode::NonRecursive,
    );

    // Throttle: rebuild at most every 500ms
    loop {
        let _ = rx.recv();
        // drain pending events
        while rx.try_recv().is_ok() {}
        std::thread::sleep(std::time::Duration::from_millis(COOLDOWN_MS));

        eprintln!();
        let sp = ui::spinner("rebuilding");
        let t0 = std::time::Instant::now();
        build_extension(app_path, true, browser, options);
        sp.finish_and_clear();

        let id = reload_id.fetch_add(1, Ordering::SeqCst);
        let _ = std::fs::write(src_dir.join(".reload-id"), id.to_string());
        ui::done_in(t0.elapsed());
    }
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut out = first.to_uppercase().to_string();
            out.push_str(chars.as_str());
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod safari_tests {
    use super::*;

    #[test]
    fn safari_packager_args_include_configured_ios_target() {
        let args = safari_packager_args(
            Path::new("dist/safari"),
            Path::new("dist/safari-app"),
            "Example",
            "com.example.extension",
            &["ios".to_string()],
        );
        assert_eq!(
            args,
            vec![
                "safari-web-extension-packager",
                "--project-location",
                "dist/safari-app",
                "--app-name",
                "Example",
                "--bundle-identifier",
                "com.example.extension",
                "--swift",
                "--copy-resources",
                "--no-open",
                "--no-prompt",
                "--ios-only",
                "dist/safari",
            ]
        );
    }
}

fn page_script_src(entry: &str) -> String {
    let relative = page_script_relative_path(entry)
        .to_string_lossy()
        .replace('\\', "/");
    format!("{}src/{relative}", page_script_prefix(entry))
}

fn page_script_prefix(entry: &str) -> String {
    let parent_depth = Path::new(entry)
        .parent()
        .map(|parent| parent.components().count())
        .unwrap_or(0);
    "../".repeat(parent_depth + 1)
}

fn page_script_relative_path(entry: &str) -> PathBuf {
    Path::new(entry).with_extension("js")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_crepus_toml_uses_narrow_content_scope() {
        let toml = scaffold_crepus_toml("My Extension");
        assert!(!toml.contains("<all_urls>"));

        let manifest: crate::crepus_toml::CrepusManifest = toml::from_str(&toml).unwrap();
        let target = manifest.targets.first().expect("target");
        let capabilities = target.capabilities.as_ref().expect("capabilities");
        assert!(capabilities.content_script);
        assert_eq!(capabilities.host_permissions, vec!["https://example.com/*"]);
    }

    #[test]
    fn copy_app_assets_overlays_app_owned_directories_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");

        std::fs::create_dir_all(app.join("src")).expect("app src");
        std::fs::create_dir_all(app.join("pages")).expect("app pages");
        std::fs::create_dir_all(app.join("icons")).expect("app icons");
        std::fs::create_dir_all(app.join("resources")).expect("app resources");
        std::fs::create_dir_all(dist.join("src")).expect("dist src");
        std::fs::create_dir_all(dist.join("vendor")).expect("dist vendor");

        std::fs::write(app.join("src/content.js"), "custom content").expect("write app src");
        std::fs::write(app.join("pages/options.html"), "options").expect("write page");
        std::fs::write(app.join("icons/icon16.png"), b"icon").expect("write icon");
        std::fs::write(app.join("resources/tlds.txt"), "com").expect("write resource");
        std::fs::write(dist.join("src/content.js"), "runtime content").expect("write runtime src");
        std::fs::write(dist.join("vendor/runtime_bg.wasm"), b"wasm").expect("write wasm");

        copy_app_assets(&app, &dist).expect("copy app assets");

        assert_eq!(
            std::fs::read_to_string(dist.join("src/content.js")).expect("read content"),
            "custom content"
        );
        assert_eq!(
            std::fs::read_to_string(dist.join("pages/options.html")).expect("read page"),
            "options"
        );
        assert_eq!(
            std::fs::read(dist.join("icons/icon16.png")).expect("read icon"),
            b"icon"
        );
        assert_eq!(
            std::fs::read_to_string(dist.join("resources/tlds.txt")).expect("read resource"),
            "com"
        );
        assert_eq!(
            std::fs::read(dist.join("vendor/runtime_bg.wasm")).expect("read wasm"),
            b"wasm"
        );
    }

    #[test]
    fn renders_extension_pages_from_crepus_sources() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");
        std::fs::create_dir_all(app.join("pages")).expect("pages");

        std::fs::write(
            app.join("pages/options.crepus"),
            "section #root\n  h1\n    \"Options\"\n<style>\n#root{color:#000}\n</style>",
        )
        .expect("write crepus");

        let manifest = crepuscularity_webext::ExtensionManifest {
            extension: crepuscularity_webext::ExtensionInfo {
                name: "Test Extension".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: Default::default(),
            content_scripts: Vec::new(),
            plugins: HashMap::new(),
            options: Default::default(),
            web_accessible_resources: Default::default(),
            commands: Default::default(),
            chrome_url_overrides: Default::default(),
        };

        render_crepus_pages(&app, &dist, &manifest).expect("render pages");
        let html = std::fs::read_to_string(dist.join("pages/options.html")).expect("read html");
        assert!(html.contains("<title>Test Extension Options</title>"));
        assert!(html.contains("<section id=\"root\">"));
        assert!(html.contains("#root{color:#000}"));
        assert!(html.contains("../src/options.js"));
        let js = std::fs::read_to_string(dist.join("src/options.js")).expect("read js");
        assert!(js.contains("runtime.options_main"));
        assert!(js.contains("runtime.render_options"));
        assert!(js.contains(r#"document.getElementById("root")"#));
    }

    #[test]
    fn renders_nested_extension_pages_with_correct_script_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");
        std::fs::create_dir_all(app.join("pages/settings")).expect("pages");

        std::fs::write(
            app.join("pages/settings/options.crepus"),
            "section #root\n  h1\n    \"Options\"",
        )
        .expect("write crepus");

        let manifest = crepuscularity_webext::ExtensionManifest {
            extension: crepuscularity_webext::ExtensionInfo {
                name: "Test Extension".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: Default::default(),
            content_scripts: Vec::new(),
            plugins: HashMap::new(),
            options: Default::default(),
            web_accessible_resources: Default::default(),
            commands: Default::default(),
            chrome_url_overrides: Default::default(),
        };

        render_crepus_pages(&app, &dist, &manifest).expect("render pages");
        let html =
            std::fs::read_to_string(dist.join("pages/settings/options.html")).expect("read html");
        assert!(html.contains("../../src/settings/options.js"));
    }

    #[test]
    fn renders_duplicate_page_stems_without_js_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");
        std::fs::create_dir_all(app.join("pages/admin")).expect("admin pages");
        std::fs::create_dir_all(app.join("pages/settings")).expect("settings pages");

        std::fs::write(
            app.join("pages/admin/options.crepus"),
            "section #admin\n  h1\n    \"Admin\"",
        )
        .expect("write admin crepus");
        std::fs::write(
            app.join("pages/settings/options.crepus"),
            "section #settings\n  h1\n    \"Settings\"",
        )
        .expect("write settings crepus");

        let manifest = crepuscularity_webext::ExtensionManifest {
            extension: crepuscularity_webext::ExtensionInfo {
                name: "Test Extension".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: Default::default(),
            content_scripts: Vec::new(),
            plugins: HashMap::new(),
            options: Default::default(),
            web_accessible_resources: Default::default(),
            commands: Default::default(),
            chrome_url_overrides: Default::default(),
        };

        render_crepus_pages(&app, &dist, &manifest).expect("render pages");
        let admin_html =
            std::fs::read_to_string(dist.join("pages/admin/options.html")).expect("admin html");
        let settings_html = std::fs::read_to_string(dist.join("pages/settings/options.html"))
            .expect("settings html");

        assert!(admin_html.contains("../../src/admin/options.js"));
        assert!(settings_html.contains("../../src/settings/options.js"));
        let admin_js =
            std::fs::read_to_string(dist.join("src/admin/options.js")).expect("admin js");
        let settings_js =
            std::fs::read_to_string(dist.join("src/settings/options.js")).expect("settings js");
        assert!(admin_js.contains(r#"from "../../vendor/runtime.js""#));
        assert!(settings_js.contains(r#"from "../../vendor/runtime.js""#));
    }

    #[test]
    fn renders_extension_css_from_crepus_sources() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");
        std::fs::create_dir_all(app.join("src")).expect("src");

        std::fs::write(
            app.join("src/content.css.crepus"),
            "div\n<style>\n.vc-find{color:#000}\n</style>",
        )
        .expect("write crepus css");

        render_crepus_css_assets(&app, &dist).expect("render css");
        assert_eq!(
            std::fs::read_to_string(dist.join("src/content.css")).expect("read css"),
            ".vc-find{color:#000}"
        );
    }

    #[test]
    fn appends_extension_css_to_existing_content_stylesheet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("app");
        let dist = tmp.path().join("dist/unpacked");
        std::fs::create_dir_all(app.join("src")).expect("src");
        std::fs::create_dir_all(dist.join("src")).expect("dist src");
        std::fs::write(dist.join("src/content.css"), ".framework{}").expect("framework css");
        std::fs::write(
            app.join("src/content.css.crepus"),
            "motion.div\n<style>\n.app-extra{color:#123}\n</style>",
        )
        .expect("write crepus css");

        render_crepus_css_assets(&app, &dist).expect("render css");
        let css = std::fs::read_to_string(dist.join("src/content.css")).expect("read css");
        assert!(css.contains(".framework{}"));
        assert!(css.contains(".app-extra{color:#123}"));
    }
}
