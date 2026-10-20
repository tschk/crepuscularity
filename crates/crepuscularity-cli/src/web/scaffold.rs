use std::path::Path;
use std::time::Instant;

use crate::scaffold;
use crate::ui;

// ── scaffold ─────────────────────────────────────────────────────────────────

const CREPUS_TOML_TEMPLATE: &str = r#"[[targets]]
type = "web"
id = "site"
site = "."
out = "dist"
entry = "index.crepus"
name = {{name}}
description = "Crepus static site (.crepus + WASM)"

[targets.seo]
title = {{name}}
description = "Crepus static site (.crepus + WASM)"
robots = "index,follow"
twitter_card = "summary"
"#;

const CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "{{rt_name}}_runtime"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
crepuscularity-web = "0.6.0"
wasm-bindgen = "0.2"

[profile.release]
lto = true
codegen-units = 1
opt-level = "z"

[workspace]
"#;

const INDEX_CREPUS: &str = r#"div w-full min-h-screen bg-zinc-950 text-zinc-50 p-8 flex flex-col gap-4
 div text-3xl font-bold
  "Hello from .crepus"
 div text-zinc-400 max-w-xl
  "This page is rendered in the browser by the same pipeline as crepus web dev — wasm32 + crepus-bundle.json."
"#;

const LIB_RS: &str = r#"use wasm_bindgen::prelude::*;

// This runtime has no DOM imports and can own its state in a web worker.
#[wasm_bindgen]
pub fn crepus_worker_runtime() {}

#[wasm_bindgen]
pub fn crepus_render(bundle_json: &str) -> Result<String, JsValue> {
    crepuscularity_web::render_bundle(bundle_json).map_err(|e| JsValue::from_str(&e.to_string()))
}
"#;

pub(crate) fn scaffold_site(name: &str) {
    scaffold_site_in(name, Path::new(""));
}

fn scaffold_site_in(name: &str, parent: &Path) {
    let t0 = Instant::now();

    let slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();

    let base = parent.join(&slug);
    if base.exists() {
        ui::error(&format!("directory already exists: {slug}"));
    }

    scaffold::create_project_dir(&base).unwrap_or_else(|e| ui::error(&e));

    let rt_name = scaffold::cargo_package_name(&slug.replace('-', "_"));
    scaffold::ensure_dir(&base.join("runtime/src"))
        .unwrap_or_else(|e| ui::error(&format!("create runtime/src dir: {e}")));
    scaffold::ensure_dir(&base.join("dist"))
        .unwrap_or_else(|e| ui::error(&format!("create dist dir: {e}")));

    let display_name = toml::Value::String(name.to_owned()).to_string();
    scaffold::write_template(
        &base.join("crepus.toml"),
        CREPUS_TOML_TEMPLATE,
        &[("{{name}}", &display_name)],
    )
    .unwrap_or_else(|e| ui::error(&format!("write crepus.toml: {e}")));

    scaffold::write_file(&base.join("index.crepus"), INDEX_CREPUS)
        .unwrap_or_else(|e| ui::error(&format!("write index.crepus: {e}")));

    scaffold::write_template(
        &base.join("runtime/Cargo.toml"),
        CARGO_TOML_TEMPLATE,
        &[("{{rt_name}}", &rt_name)],
    )
    .unwrap_or_else(|e| ui::error(&format!("write runtime/Cargo.toml: {e}")));

    scaffold::write_file(&base.join("runtime/src/lib.rs"), LIB_RS)
        .unwrap_or_else(|e| ui::error(&format!("write runtime/src/lib.rs: {e}")));

    scaffold::scaffold_success(
        &slug,
        &base,
        &[
            &format!("cd {slug}"),
            "crepus web dev --site .",
            "crepus build",
        ],
    );
    ui::done_in(t0.elapsed());
}

#[cfg(test)]
mod display_name_tests {
    use super::*;

    #[test]
    fn generated_runtime_names_are_valid_cargo_packages() {
        for (name, slug) in [
            ("app²", "app²"),
            ("123 site", "123-site"),
            ("Éclair", "éclair"),
            ("東京", "東京"),
        ] {
            let parent = tempfile::tempdir().unwrap();
            scaffold_site_in(name, parent.path());
            let manifest = parent.path().join(slug).join("runtime/Cargo.toml");
            // Validate Cargo names without resolving scaffold dependencies.
            let checked = std::process::Command::new("cargo")
                .arg("read-manifest")
                .arg("--manifest-path")
                .arg(&manifest)
                .output()
                .unwrap();
            assert!(
                checked.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }

    #[test]
    fn generated_scaffold_manifests_preserve_display_names() {
        for (name, slug) in [
            ("ordinary-site", "ordinary-site"),
            ("Acme \"Docs\"", "acme--docs-"),
            ("Acme\\Docs", "acme-docs"),
            ("Acme\nDocs", "acme-docs"),
        ] {
            let parent = tempfile::tempdir().unwrap();
            scaffold_site_in(name, parent.path());
            let project = parent.path().join(slug);
            let manifest: toml::Value =
                toml::from_str(&std::fs::read_to_string(project.join("crepus.toml")).unwrap())
                    .unwrap();
            assert_eq!(manifest["targets"][0]["name"].as_str(), Some(name));
            assert_eq!(manifest["targets"][0]["seo"]["title"].as_str(), Some(name));
            let runtime: toml::Value = toml::from_str(
                &std::fs::read_to_string(project.join("runtime/Cargo.toml")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                runtime["package"]["name"].as_str(),
                Some(format!("{}_runtime", slug.replace('-', "_")).as_str())
            );
            assert!(project.join("index.crepus").is_file());
            assert!(std::fs::read_to_string(project.join("runtime/src/lib.rs"))
                .unwrap()
                .contains("pub fn crepus_worker_runtime()"));
        }
    }
}
