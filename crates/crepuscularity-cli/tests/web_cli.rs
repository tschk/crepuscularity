//! Integration tests for `crepus web`.

use std::process::Command;
use std::sync::Mutex;

static DOCS_HOOK_TEST_LOCK: Mutex<()> = Mutex::new(());

fn crepus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
}

#[test]
fn web_based_scaffolds_generate_valid_cargo_names() {
    for command in [
        ["webext", "new"],
        ["init", "webext"],
        ["moonshine", "new"],
        ["web", "new"],
    ] {
        for (name, slug, package) in [
            ("app²", "app²", "app_"),
            ("123 site", "123-site", "_123-site"),
            ("Éclair", "éclair", "éclair"),
            ("ordinary-site", "ordinary-site", "ordinary-site"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let generated = crepus()
                .current_dir(root.path())
                .args(command)
                .arg(name)
                .output()
                .unwrap();
            assert!(
                generated.status.success(),
                "{}",
                String::from_utf8_lossy(&generated.stderr)
            );
            let project = root.path().join(slug);
            let manifest = if command[0] == "moonshine" {
                project.join("Cargo.toml")
            } else {
                project.join("runtime/Cargo.toml")
            };
            let checked = Command::new("cargo")
                .arg("read-manifest")
                .arg("--manifest-path")
                .arg(manifest)
                .output()
                .unwrap();
            assert!(
                checked.status.success(),
                "{command:?} {name}: {}",
                String::from_utf8_lossy(&checked.stderr)
            );
            let manifest: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
            let expected = if command[0] == "moonshine" {
                package.to_owned()
            } else if command[0] == "web" {
                format!("{}_runtime", package.replace('-', "_"))
            } else {
                format!("{package}_runtime")
            };
            assert_eq!(manifest["name"].as_str(), Some(expected.as_str()));
        }
    }
}

#[cfg(unix)]
#[test]
fn web_build_honors_cargo_home_and_explicit_cargo() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let site = root.path().join("site");
    std::fs::create_dir_all(site.join("runtime/src")).unwrap();
    std::fs::write(site.join("index.crepus"), "p \"Cargo selection\"").unwrap();
    std::fs::write(site.join("runtime/Cargo.toml"), "[package]\nname='runtime'\nversion='0.1.0'\nedition='2021'\n[lib]\ncrate-type=['cdylib']\n").unwrap();
    std::fs::write(site.join("runtime/src/lib.rs"), "").unwrap();
    for (directory, marker) in [
        ("custom cargo/bin", "custom-cargo-home"),
        ("path", "path-cargo"),
        ("explicit", "explicit-cargo"),
    ] {
        let bin = root.path().join(directory);
        std::fs::create_dir_all(&bin).unwrap();
        let cargo = bin.join("cargo");
        std::fs::write(&cargo, format!("#!/bin/sh\necho '{marker}' >&2\nprintf 'cargo-home=%s\\n' \"$CARGO_HOME\" >&2\nexit 1\n")).unwrap();
        std::fs::set_permissions(cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for (explicit, relative) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut command = crepus();
        command
            .current_dir(root.path())
            .args(["web", "build", "--site"])
            .arg(&site)
            .arg("--out-dir")
            .arg(root.path().join("out"))
            .env_remove("CARGO")
            .env(
                "CARGO_HOME",
                if relative {
                    std::path::PathBuf::from("custom cargo")
                } else {
                    root.path().join("custom cargo")
                },
            )
            .env("HOME", root.path().join("empty-home"))
            .env("PATH", root.path().join("path"));
        if explicit {
            command.env("CARGO", root.path().join("explicit/cargo"));
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = if explicit {
            "explicit-cargo"
        } else {
            "custom-cargo-home"
        };
        assert!(stderr.contains(expected), "{stderr}");
        assert!(!stderr.contains("path-cargo"), "{stderr}");
        let home = stderr
            .lines()
            .find_map(|line| line.trim().strip_prefix("cargo-home="))
            .expect("child Cargo home");
        assert!(std::path::Path::new(home).is_absolute(), "{stderr}");
        assert_eq!(
            std::fs::canonicalize(home).unwrap(),
            std::fs::canonicalize(root.path().join("custom cargo")).unwrap()
        );
    }
}

#[test]
fn web_scaffolds_use_the_current_backend_release_families() {
    let root = tempfile::tempdir().unwrap();
    for (kind, crate_name) in [
        ("web", "crepuscularity-web"),
        ("webext", "crepuscularity-webext"),
    ] {
        let name = format!("{kind}-versions");
        let output = crepus()
            .current_dir(root.path())
            .args([kind, "new", &name])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let generated: toml::Value = toml::from_str(
            &std::fs::read_to_string(root.path().join(&name).join("runtime/Cargo.toml")).unwrap(),
        )
        .unwrap();
        let dependency = &generated["dependencies"][crate_name];
        let version = dependency
            .as_str()
            .or_else(|| dependency.get("version").and_then(toml::Value::as_str))
            .unwrap();
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(crate_name)
            .join("Cargo.toml");
        let backend: toml::Value =
            toml::from_str(&std::fs::read_to_string(manifest).unwrap()).unwrap();
        assert_eq!(
            version,
            backend["package"]["version"].as_str().unwrap(),
            "{crate_name}"
        );
        assert!(dependency.get("path").is_none());
    }
}

#[test]
fn webext_build_stops_when_runtime_compilation_fails() {
    let root = tempfile::tempdir().unwrap();
    let scaffold = crepus()
        .current_dir(root.path())
        .args(["webext", "new", "Broken Runtime"])
        .output()
        .unwrap();
    assert!(scaffold.status.success());
    let app = root.path().join("broken-runtime");
    std::fs::write(app.join("runtime/Cargo.toml"), "invalid = [\n").unwrap();
    let output = crepus()
        .args(["webext", "build", "--app"])
        .arg(&app)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        log.contains("extension runtime compilation failed"),
        "{log}"
    );
    assert!(!log.contains("built to"), "{log}");
}

fn wasm_build_prereqs_ok() -> bool {
    let has_target = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
        .unwrap_or(false);
    let has_bindgen = std::process::Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    has_target && has_bindgen
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_new_scaffolds_crepus_and_runtime() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let status = crepus()
        .current_dir(tmp.path())
        .args(["web", "new", "Acme Labs"])
        .status()
        .expect("spawn crepus web new");
    assert!(status.success());

    let site_dir = tmp.path().join("acme-labs");
    assert!(site_dir.join("index.crepus").is_file());
    assert!(site_dir.join("runtime/Cargo.toml").is_file());
    assert!(site_dir.join("runtime/src/lib.rs").is_file());
    let idx = std::fs::read_to_string(site_dir.join("index.crepus")).expect("read index");
    assert!(idx.contains(".crepus"));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_build_example_site_emits_wasm() {
    if !wasm_build_prereqs_ok() {
        eprintln!("skipping: wasm32 target or wasm-bindgen not installed");
        return;
    }
    let out = tempfile::tempdir().expect("tempdir");
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/web-site")
        .canonicalize()
        .expect("canonicalize examples/web-site");

    let status = crepus()
        .args([
            "web",
            "build",
            "--site",
            example.to_str().unwrap(),
            "--entry",
            "./index.crepus",
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("spawn crepus web build");
    assert!(
        status.success(),
        "crepus web build should succeed with wasm32 + wasm-bindgen"
    );

    let has_wasm = std::fs::read_dir(out.path().join("pkg"))
        .expect("read pkg")
        .flatten()
        .any(|e| e.path().extension().map(|x| x == "wasm").unwrap_or(false));
    assert!(
        has_wasm,
        "expected pkg/*.wasm (wasm-bindgen output); install wasm-bindgen-cli and rustup target add wasm32-unknown-unknown"
    );
    let bundle = std::fs::read_to_string(out.path().join("crepus-bundle.json")).expect("bundle");
    let bundle_json: serde_json::Value = serde_json::from_str(&bundle).expect("bundle json");
    assert_eq!(bundle_json["entry"], "index.crepus");
    assert!(bundle.contains("entry"));
    let html = std::fs::read_to_string(out.path().join("index.html")).expect("index.html");
    assert!(html.contains("<!DOCTYPE html>") || html.contains("<!doctype html>"));
    assert!(html.contains("crepus-root"));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_build_docs_site_emits_wasm() {
    let _guard = DOCS_HOOK_TEST_LOCK.lock().expect("docs hook test lock");
    if !wasm_build_prereqs_ok() {
        eprintln!("skipping: wasm32 target or wasm-bindgen not installed");
        return;
    }
    let out = tempfile::tempdir().expect("tempdir");
    let docs_site = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs-site")
        .canonicalize()
        .expect("canonicalize docs-site");

    let status = crepus()
        .args([
            "web",
            "build",
            "--site",
            docs_site.to_str().unwrap(),
            "--out-dir",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("spawn");
    assert!(status.success(), "docs-site web build should succeed");

    let has_wasm = std::fs::read_dir(out.path().join("pkg"))
        .expect("pkg dir")
        .flatten()
        .any(|e| e.path().extension().map(|x| x == "wasm").unwrap_or(false));
    assert!(has_wasm, "WASM pkg should be emitted");
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
// ponytail: docs-renderer binary removed; docs-target tests removed
fn root_build_uses_crepus_toml_lvgl_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::write(
        root.join("crepus.toml"),
        r##"
[[targets]]
type = "lvgl"
id = "dash"
template = "ui.crepus"
out = "dist/dash.xml"
name = "Dash"
root = "screen"

[targets.vars]
device = "STM32F411"
"##,
    )
    .expect("write manifest");
    std::fs::write(
        root.join("ui.crepus"),
        r##"div #panel bg-[#101820]
  h1 text-white
    "{device}"
"##,
    )
    .expect("write template");

    let status = crepus()
        .current_dir(root)
        .args(["build", "--release", "--opt-level", "size"])
        .status()
        .expect("spawn crepus build");
    assert!(status.success());

    let xml = std::fs::read_to_string(root.join("dist/dash.xml")).expect("read xml");
    assert!(xml.contains(r#"<screen name="Dash">"#));
    assert!(xml.contains("STM32F411"));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn root_build_type_selects_only_matching_targets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::write(
        root.join("crepus.toml"),
        r##"
[[targets]]
type = "lvgl"
id = "panel"
template = "panel.crepus"
out = "dist/panel.xml"
name = "Panel"
root = "screen"

[targets.vars]
label = "Panel"

[[targets]]
type = "native"
id = "shell"
template = "shell.crepus"
out = "dist/shell.json"

[targets.vars]
label = "Shell"

[[targets]]
type = "lvgl"
id = "tile"
template = "tile.crepus"
out = "dist/tile.xml"
name = "Tile"
root = "component"

[targets.vars]
label = "Tile"
"##,
    )
    .expect("write manifest");
    std::fs::write(
        root.join("panel.crepus"),
        r##"div #panel
  h1
    "{label}"
"##,
    )
    .expect("write panel template");
    std::fs::write(
        root.join("shell.crepus"),
        r##"div #shell
  h1
    "{label}"
"##,
    )
    .expect("write shell template");
    std::fs::write(
        root.join("tile.crepus"),
        r##"div #tile
  span
    "{label}"
"##,
    )
    .expect("write tile template");

    let out = crepus()
        .current_dir(root)
        .args(["build", "lvgl"])
        .output()
        .expect("spawn crepus build lvgl");
    assert!(
        out.status.success(),
        "crepus build lvgl failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let panel = std::fs::read_to_string(root.join("dist/panel.xml")).expect("read panel xml");
    let tile = std::fs::read_to_string(root.join("dist/tile.xml")).expect("read tile xml");
    assert!(panel.contains(r#"<screen name="Panel">"#));
    assert!(panel.contains("Panel"));
    assert!(tile.contains(r#"<component name="Tile">"#));
    assert!(tile.contains("Tile"));
    assert!(
        !root.join("dist/shell.json").exists(),
        "crepus build lvgl should not build native targets"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn init_webext_scaffolds_same_shape_as_webext_new() {
    fn collect_files(dir: &std::path::Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(path) = stack.pop() {
            for entry in std::fs::read_dir(&path).expect("read dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    let rel = path
                        .strip_prefix(dir)
                        .expect("strip prefix")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let body = std::fs::read_to_string(&path).expect("read scaffold file");
                    files.push((rel, body));
                }
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    let direct = tempfile::tempdir().expect("direct tempdir");
    let init = tempfile::tempdir().expect("init tempdir");

    let direct_status = crepus()
        .current_dir(direct.path())
        .args(["webext", "new", "Acme Extension"])
        .status()
        .expect("spawn crepus webext new");
    assert!(direct_status.success());

    let init_status = crepus()
        .current_dir(init.path())
        .args(["init", "webext", "Acme Extension"])
        .status()
        .expect("spawn crepus init webext");
    assert!(init_status.success());

    let direct_files = collect_files(&direct.path().join("acme-extension"));
    let init_files = collect_files(&init.path().join("acme-extension"));
    assert_eq!(direct_files, init_files);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn webext_manifest_reads_safari_target_from_crepus_toml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("crepus.toml"),
        r#"
[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["safari"]

[targets.extension]
name = "Example"
version = "1.0.0"
description = "Example Safari extension"

[targets.safari]
bundle_identifier = "com.example.extension"
"#,
    )
    .expect("write manifest");

    let output = crepus()
        .current_dir(tmp.path())
        .args(["webext", "manifest", "--browser", "safari"])
        .output()
        .expect("spawn manifest");
    assert!(output.status.success());
    let manifest = String::from_utf8_lossy(&output.stdout);
    assert!(manifest.contains("\"manifest_version\": 3"));
    assert!(!manifest.contains("\"browser_specific_settings\""));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn root_build_uses_configured_webext_browsers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("crepus.toml"),
        r#"
[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["chromium", "firefox"]

[targets.extension]
name = "Example"
version = "1.0.0"
"#,
    )
    .expect("write manifest");

    let status = crepus()
        .current_dir(tmp.path())
        .arg("build")
        .status()
        .expect("spawn build");
    assert!(status.success());
    assert!(tmp.path().join("dist/chromium/manifest.json").is_file());
    assert!(tmp.path().join("dist/firefox/manifest.json").is_file());
}

#[test]
#[cfg(target_os = "macos")]
fn webext_build_packages_safari_project_from_crepus_toml() {
    let tmp = tempfile::Builder::new()
        .prefix("safari-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("tempdir");
    std::fs::write(
        tmp.path().join("crepus.toml"),
        r#"
[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["safari"]

[targets.extension]
name = "Example"
version = "1.0.0"
description = "Example Safari extension"

[targets.safari]
bundle_identifier = "com.example.extension"
project_location = "safari-app"
platforms = ["macos"]
"#,
    )
    .expect("write manifest");

    let status = crepus()
        .current_dir(tmp.path())
        .args(["webext", "build", "--browser", "safari"])
        .status()
        .expect("spawn Safari build");
    let manifest = std::fs::read_to_string(tmp.path().join("dist/safari/manifest.json"))
        .expect("read Safari manifest");
    assert!(status.success(), "manifest: {manifest}");
    assert!(tmp.path().join("dist/safari/manifest.json").is_file());
    assert!(tmp.path().join("safari-app").is_dir());
    let project = tmp
        .path()
        .join("safari-app/Example/Example.xcodeproj/project.pbxproj");
    let converted = Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(project)
        .output()
        .expect("read generated Safari project");
    assert!(
        converted.status.success(),
        "{}",
        String::from_utf8_lossy(&converted.stderr)
    );
    let project: serde_json::Value = serde_json::from_slice(&converted.stdout).unwrap();
    let objects = project["objects"].as_object().unwrap();
    let mut applications = 0;
    let mut extensions = 0;
    for target in objects
        .values()
        .filter(|target| target["isa"] == "PBXNativeTarget")
    {
        let expected = match target["productType"].as_str().unwrap() {
            "com.apple.product-type.application" => {
                applications += 1;
                "com.example.extension"
            }
            "com.apple.product-type.app-extension" => {
                extensions += 1;
                "com.example.extension.Extension"
            }
            other => panic!("unexpected Safari product type {other}"),
        };
        let list = target["buildConfigurationList"].as_str().unwrap();
        let configurations = objects[list]["buildConfigurations"].as_array().unwrap();
        assert!(!configurations.is_empty());
        for configuration in configurations {
            let settings = &objects[configuration.as_str().unwrap()]["buildSettings"];
            assert_eq!(
                settings["PRODUCT_BUNDLE_IDENTIFIER"].as_str(),
                Some(expected),
                "target {}",
                target["name"]
            );
        }
    }
    assert_eq!(applications, 1);
    assert_eq!(extensions, 1);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_build_multi_target_manifest_requires_target_flag() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let tom = r#"
[[targets]]
type = "web"
id = "a"
site = "a"
out = "a/dist"

[[targets]]
type = "web"
id = "b"
site = "b"
out = "b/dist"
"#;
    std::fs::write(root.join("crepus.toml"), tom).expect("write manifest");
    std::fs::create_dir_all(root.join("a")).expect("mkdir a");
    std::fs::create_dir_all(root.join("b")).expect("mkdir b");

    let man = root.join("crepus.toml");
    let out = crepus()
        .current_dir(root)
        .args(["web", "build", "--manifest", man.to_str().unwrap()])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = out.wait_with_output().expect("wait");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--target") || stderr.contains("target"),
        "stderr: {stderr}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_new_fails_when_dir_exists() {
    for target in ["web", "webext"] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("dup-site");
        std::fs::create_dir(&project).unwrap();
        let manifest = project.join("crepus.toml");
        std::fs::write(&manifest, "original project").unwrap();
        let status = crepus()
            .current_dir(tmp.path())
            .args([target, "new", "dup-site"])
            .status()
            .expect("spawn");
        assert!(
            !status.success(),
            "{target} must reject an existing project"
        );
        assert_eq!(
            std::fs::read_to_string(&manifest).unwrap(),
            "original project"
        );
        assert!(!project.join("runtime").exists());
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn root_help_lists_web_commands() {
    let out = crepus().args(["--help"]).output().expect("help");
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "crepus --help failed: {help}");
    assert!(
        help.contains("web") && help.contains("new") && help.contains("build"),
        "usage should list web commands:\n{help}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn lvgl_manifest_build_uses_the_root_template_frontend() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("view.vue"),
        "<script>const ignored = 1 < 2;</script><template><label>{{ title }}</label></template>",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("crepus.toml"),
        r#"[[targets]]
 type = "lvgl"
 id = "dash"
 template = "view.vue"
 out = "dist/dash.xml"
 root = "screen"
 name = "Dash"
 [targets.vars]
 title = "Vue label"
"#,
    )
    .unwrap();
    let output = crepus()
        .current_dir(directory.path())
        .args(["build", "--target", "dash"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let xml = std::fs::read_to_string(directory.path().join("dist/dash.xml")).unwrap();
    assert!(xml.contains("<screen name=\"Dash\">"));
    assert!(xml.contains("text=\"Vue label\""));
    assert!(!xml.contains("const ignored"));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn named_lvgl_manifest_build_honors_wrapper_options() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("ui.crepus"),
        "--- Card\nspan \"Named content\"",
    )
    .unwrap();
    let source = r#"[[targets]]
type = "lvgl"
id = "dash"
template = "ui.crepus"
component = "Card"
out = "dist/dash.xml"
root = "screen"
name = "CustomScreen"
"#;
    std::fs::write(directory.path().join("crepus.toml"), source).unwrap();
    let output = crepus()
        .current_dir(directory.path())
        .args(["build", "--target", "dash"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let xml = std::fs::read_to_string(directory.path().join("dist/dash.xml")).unwrap();
    assert!(xml.contains("<screen name=\"CustomScreen\">"), "{xml}");
    assert!(xml.contains("Named content"));
    std::fs::write(
        directory.path().join("crepus.toml"),
        source.replace("root = \"screen\"", "root = \"invalid\""),
    )
    .unwrap();
    let output = crepus()
        .current_dir(directory.path())
        .args(["build", "--target", "dash"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("dist/dash.xml")).unwrap(),
        xml
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn native_manifest_build_uses_the_root_template_frontend() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("view.vue"), "<script>const ignored = 1 < 2;</script><template><div><span>{{ title }}</span></div></template>").unwrap();
    for target_type in ["native", "ir"] {
        let manifest = format!(
            r#"[[targets]]
type = "{target_type}"
id = "native-ui"
template = "view.vue"
out = "dist/native.json"
[targets.vars]
title = "Vue native title"
"#
        );
        std::fs::write(directory.path().join("crepus.toml"), manifest).unwrap();
        let output = crepus()
            .current_dir(directory.path())
            .args(["build", "--target", "native-ui"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{target_type}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json = std::fs::read_to_string(directory.path().join("dist/native.json")).unwrap();
        let ir: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            ir.pointer("/root/0/children/0/content")
                .and_then(|value| value.as_str()),
            Some("Vue native title"),
            "{json}"
        );
        assert!(!json.contains("const ignored"));
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn embedded_manifest_build_keeps_frontend_and_relative_include_paths() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("views")).unwrap();
    std::fs::write(directory.path().join("views/view.vue"), "<script>const ignored = 1 < 2;</script><template><div class=\"bg-[#ff0000] w-full h-full\"/></template>").unwrap();
    std::fs::write(
        directory.path().join("views/view.crepus"),
        "include child.crepus",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("views/child.crepus"),
        "div bg-[#ff0000] w-full h-full",
    )
    .unwrap();
    for template in ["views/view.vue", "views/view.crepus"] {
        let manifest = format!(
            r#"[[targets]]
type = "embedded"
id = "panel"
template = "{template}"
out = "dist/panel.ppm"
width = 2
height = 2
"#
        );
        std::fs::write(directory.path().join("crepus.toml"), manifest).unwrap();
        let output = crepus()
            .current_dir(directory.path())
            .args(["build", "--target", "panel"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{template}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let ppm = std::fs::read(directory.path().join("dist/panel.ppm")).unwrap();
        let mut expected = b"P6\n2 2\n255\n".to_vec();
        expected.extend([255, 0, 0].repeat(4));
        assert_eq!(ppm, expected, "{template}");
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn wasm_build_rejects_missing_component_before_writing_output() {
    let site = tempfile::tempdir().unwrap();
    std::fs::create_dir(site.path().join("runtime")).unwrap();
    std::fs::write(site.path().join("runtime/Cargo.toml"), "invalid = [").unwrap();
    std::fs::write(
        site.path().join("cards.crepus"),
        "--- Card\nspan \"Existing card\"",
    )
    .unwrap();
    let out = site.path().join("dist");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("index.html"), "Previous output").unwrap();
    for entry in ["cards.crepus#Missing", "cards.crepus#"] {
        let output = crepus()
            .args(["web", "build", "--site"])
            .arg(site.path())
            .args(["--entry", entry])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(log.contains("component not found"), "{log}");
        assert_eq!(
            std::fs::read_to_string(out.join("index.html")).unwrap(),
            "Previous output"
        );
        assert!(!out.join("crepus-bundle.json").exists());
    }
}

#[test]
fn react_emit_writes_an_importable_component_without_a_moonshine_runtime() {
    let site = tempfile::tempdir().unwrap();
    std::fs::write(
        site.path().join("index.crepus"),
        "text \"{title}\"\nbutton @click=choose \"Choose\"",
    )
    .unwrap();
    let result = crepus()
        .args(["web", "build", "--emit", "react", "--site"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let component = std::fs::read_to_string(site.path().join("dist/CrepusEmit.react.tsx")).unwrap();
    assert!(component.contains("export function CrepusEmit("));
    assert!(component.contains("export default CrepusEmit;"));
    assert!(component.contains("__crepusText(scope.title)"));
    assert!(!component.contains("@tschk/moonshine"));
    assert!(!component.contains("createApp"));
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(json["root"][0]["kind"], "text");
}

#[test]
fn imba_emit_writes_native_component_and_companion_bindings() {
    let site = tempfile::tempdir().unwrap();
    std::fs::write(
        site.path().join("index.crepus"),
        "text \"{title}\"\nfor row in rows\n  button @click=choose \"Open {row.label}\"\nbadge label=\"Ready\" tone=success",
    )
    .unwrap();
    let result = crepus()
        .args(["web", "build", "--emit", "imba", "--site"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = std::fs::read_to_string(site.path().join("dist/CrepusEmit.imba")).unwrap();
    let bindings =
        std::fs::read_to_string(site.path().join("dist/CrepusEmit.bindings.ts")).unwrap();
    assert!(source.contains("export tag CrepusEmit"));
    assert!(source.contains("data-tone=(\"success\")> \"Ready\""));
    assert!(source.contains("./CrepusEmit.bindings.ts"));
    assert!(source.contains("for _crepusItem0 in bindings."));
    assert!(bindings.contains("__crepusText(_crepusItem0?.label)"));
    assert!(site.path().join("dist/crepus-view-ir.json").is_file());
}

#[test]
fn imba_emit_rejects_unsupported_control_state_without_silent_output() {
    let site = tempfile::tempdir().unwrap();
    std::fs::write(site.path().join("index.crepus"), "input disabled").unwrap();
    let result = crepus()
        .args(["web", "build", "--emit", "imba", "--site"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported View IR control state"));
    assert!(!site.path().join("dist/CrepusEmit.imba").exists());
}

#[test]
fn imba_picker_emit_keeps_bound_selection_and_change_callbacks() {
    let site = tempfile::tempdir().unwrap();
    std::fs::write(site.path().join("index.crepus"), "picker bind=prefs.choice @change=choose\n  option value=\"one\" \"One\"\n  option value=\"two\" \"Two\"").unwrap();
    let result = crepus()
        .args(["web", "build", "--emit", "imba", "--site"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let source = std::fs::read_to_string(site.path().join("dist/CrepusEmit.imba")).unwrap();
    let bindings =
        std::fs::read_to_string(site.path().join("dist/CrepusEmit.bindings.ts")).unwrap();
    assert!(source.contains("<select"));
    assert_eq!(source.matches("__crepusPickerValue=(bindings.").count(), 1);
    assert!(source.contains("bindings.syncPickers(self)\n\t\tself"));
    assert!(!source.contains(" selected="));
    assert!(source.contains("bindings.dispatch(handlers,\"choose\",e)"));
    assert!(bindings.contains("scope?.prefs?.choice"));
}

#[test]
fn moonshine_dom_and_ssr_emit_native_typescript_without_jsx_frameworks() {
    for (target, runtime) in [
        ("moonshine-dom", "@tschk/moonshine-dom/dom"),
        ("moonshine-ssr", "@tschk/moonshine-dom/ssr"),
    ] {
        let site = tempfile::tempdir().unwrap();
        std::fs::write(
            site.path().join("index.crepus"),
            "text \"Hello {title}\"\nfor row in rows\n  button @click=choose \"Open {row.label}\"",
        )
        .unwrap();
        let result = crepus()
            .args(["web", "build", "--emit", target, "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output =
            std::fs::read_to_string(site.path().join(format!("dist/CrepusEmit.{target}.ts")))
                .unwrap();
        assert!(output.contains(runtime), "{target}: {output}");
        assert!(output.contains("CrepusEmit"), "{target}: {output}");
        for forbidden in [
            "from \"react\"",
            "from \"solid-js\"",
            "@tschk/moonshine/react",
            "<span",
            "<button",
            "<Fragment",
        ] {
            assert!(
                !output.contains(forbidden),
                "{target}: unexpected {forbidden}: {output}"
            );
        }
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(json["root"][0]["kind"], "text");
        assert_eq!(json["root"][1]["kind"], "forEach");
    }
}

#[test]
fn moonshine_dom_and_ssr_reject_unsupported_nodes_without_component_output() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        std::fs::write(site.path().join("index.crepus"), "input disabled").unwrap();
        let result = crepus()
            .args(["web", "build", "--emit", target, "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(!result.status.success(), "{target}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("unsupported View IR control state"),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!site
            .path()
            .join(format!("dist/CrepusEmit.{target}.ts"))
            .exists());
    }
}

#[test]
fn framework_emit_collects_every_supported_frontend_entry_extension() {
    for (extension, source) in [
        ("html", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("jsx", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("TSX", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("csx", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("svelte", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("vue", "<template><div><span>Text {{ title }}</span><button>Open {{ title }}</button></div></template>"),
        ("astro", "<div><span>Text {title}</span><button>Open {title}</button></div>"),
        ("ng", "<div><span>Text {{ title }}</span><button>Open {{ title }}</button></div>"),
        ("ng.html", "<div><span>Text {{ title }}</span><button>Open {{ title }}</button></div>"),
        ("component.html", "<div><span>Text {{ title }}</span><button>Open {{ title }}</button></div>"),
    ] {
        for (target, output_name) in [
            ("react", "CrepusEmit.react.tsx"),
            ("moonshine-dom", "CrepusEmit.moonshine-dom.ts"),
        ] {
            let site = tempfile::tempdir().unwrap();
            std::fs::create_dir(site.path().join("nested")).unwrap();
            let entry = format!("nested/view.{extension}");
            let authored_svelte = target == "moonshine-dom" && extension == "svelte";
            let source = if authored_svelte { format!("<script>let title = 'Title';</script>{source}") } else { source.to_owned() };
            std::fs::write(site.path().join(&entry), source).unwrap();
            let result = crepus()
                .args(["web", "build", "--emit", target, "--entry", &entry, "--site"])
                .arg(site.path())
                .output()
                .unwrap();
            assert!(result.status.success(), "{target}/{extension}: {}", String::from_utf8_lossy(&result.stderr));
            assert!(site.path().join("dist").join(output_name).is_file());
            if authored_svelte {
                assert!(!site.path().join("dist/crepus-view-ir.json").exists());
                let module = std::fs::read_to_string(site.path().join("dist/modules/moonshine-dom/nested/view.svelte.js")).unwrap();
                assert!(module.contains("title = \"Title\""), "{module}");
                assert!(module.contains("() => title"), "{module}");
                continue;
            }
            let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap()).unwrap();
            let text = &json["root"][0]["children"][0];
            let button = &json["root"][0]["children"][1];
            assert_eq!(text["kind"], "text", "{extension}: {json}");
            assert!(text["bind"].as_str().is_some_and(|bind| bind.contains("title")), "{extension}: {json}");
            assert_eq!(button["kind"], "button", "{extension}: {json}");
            assert!(button["labelParts"].as_array().unwrap().iter().any(|part| part["kind"] == "expr" && part["value"] == "title"), "{extension}: {json}");
        }
    }
}

#[test]
fn framework_emit_bundles_nested_frontend_includes_without_changing_literals() {
    let site = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(site.path().join("parts/nested")).unwrap();
    std::fs::write(
        site.path().join("index.crepus"),
        "div\n  include parts/card.svelte",
    )
    .unwrap();
    std::fs::write(
        site.path().join("parts/card.svelte"),
        "<div><span>Svelte literal ｛secret｝</span><include src=\"nested/card.vue\" /></div>",
    )
    .unwrap();
    std::fs::write(
        site.path().join("parts/nested/card.vue"),
        "<template><span>Vue literal ｛secret｝</span></template>",
    )
    .unwrap();
    let result = crepus()
        .args(["web", "build", "--emit", "moonshine-dom", "--site"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap(),
    )
    .unwrap();
    let card = &json["root"][0]["children"][0];
    assert_eq!(
        card["children"][0]["content"], "Svelte literal ｛secret｝",
        "{json}"
    );
    assert_eq!(
        card["children"][1]["content"], "Vue literal ｛secret｝",
        "{json}"
    );
    assert!(site
        .path()
        .join("dist/CrepusEmit.moonshine-dom.ts")
        .is_file());
}

#[test]
fn source_emit_rejects_malformed_control_expressions_before_artifacts() {
    for (target, filename) in [
        ("react", "CrepusEmit.react.tsx"),
        ("moonshine-dom", "CrepusEmit.moonshine-dom.ts"),
        ("moonshine-ssr", "CrepusEmit.moonshine-ssr.ts"),
    ] {
        for source in [
            "if count +\n  text \"Invalid condition\"",
            "for row in rows +\n  text \"{row.name}\"",
        ] {
            let site = tempfile::tempdir().unwrap();
            std::fs::write(site.path().join("index.crepus"), source).unwrap();
            let result = crepus()
                .args(["web", "build", "--emit", target, "--site"])
                .arg(site.path())
                .output()
                .unwrap();
            assert!(
                !result.status.success(),
                "{target} accepted {source:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(!site.path().join("dist").join(filename).exists());
            assert!(!site.path().join("dist/crepus-view-ir.json").exists());
        }
    }
}

#[test]
fn source_emit_validates_control_syntax_without_evaluating_missing_context() {
    for (target, filename) in [
        ("react", "CrepusEmit.react.tsx"),
        ("moonshine-dom", "CrepusEmit.moonshine-dom.ts"),
        ("moonshine-ssr", "CrepusEmit.moonshine-ssr.ts"),
    ] {
        let site = tempfile::tempdir().unwrap();
        std::fs::write(
            site.path().join("index.crepus"),
            "if 10 / divisor > 1\n  input bind=prefs.name @change=prefs.edit\n  button @click=prefs.choose \"Choose\"",
        )
        .unwrap();
        let result = crepus()
            .args(["web", "build", "--emit", target, "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(site.path().join("dist").join(filename).is_file());
        let ir: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(site.path().join("dist/crepus-view-ir.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(ir["root"][0]["condition"], "10 / divisor > 1");
        assert_eq!(ir["root"][0]["thenChildren"][0]["bind"], "prefs.name");
        assert_eq!(ir["root"][0]["thenChildren"][0]["onChange"], "prefs.edit");
        assert_eq!(ir["root"][0]["thenChildren"][1]["onClick"], "prefs.choose");
    }
}

#[test]
fn wasm_output_rejects_static_source_before_runtime_setup() {
    let site = tempfile::tempdir().unwrap();
    let assets = site.path().join("static");
    std::fs::create_dir(&assets).unwrap();
    std::fs::write(assets.join("app.js"), "authored asset").unwrap();
    let outputs = vec![assets.clone(), assets.join("nested-output")];
    #[cfg(unix)]
    let outputs = {
        let mut outputs = outputs;
        let alias = site.path().join("aliased-output");
        std::os::unix::fs::symlink(&assets, &alias).unwrap();
        outputs.push(alias);
        outputs
    };
    for out in outputs {
        let output = crepus()
            .args(["web", "build", "--site"])
            .arg(site.path())
            .arg("--out-dir")
            .arg(&out)
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            log.contains("output must not be inside static source"),
            "{log}"
        );
        assert_eq!(
            std::fs::read_to_string(assets.join("app.js")).unwrap(),
            "authored asset"
        );
        assert!(!assets.join("nested-output").exists());
        assert!(!assets.join("crepus-bundle.json").exists());
    }
}
