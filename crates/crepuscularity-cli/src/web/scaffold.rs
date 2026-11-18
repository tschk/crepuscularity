use std::path::PathBuf;
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
crepuscularity-web = "0.4.3"
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

#[wasm_bindgen]
pub fn crepus_render(bundle_json: &str) -> Result<String, JsValue> {
    crepuscularity_web::render_bundle(bundle_json).map_err(|e| JsValue::from_str(&e.to_string()))
}
"#;

pub(crate) fn scaffold_site(name: &str) {
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

    let rt_name = slug.replace('-', "_");
    scaffold::ensure_dir(&base.join("runtime/src"))
        .unwrap_or_else(|e| ui::error(&format!("create runtime/src dir: {e}")));
    scaffold::ensure_dir(&base.join("dist"))
        .unwrap_or_else(|e| ui::error(&format!("create dist dir: {e}")));

    scaffold::write_template(
        &base.join("crepus.toml"),
        CREPUS_TOML_TEMPLATE,
        &[(
            "{{name}}",
            &toml::Value::String(name.to_owned()).to_string(),
        )],
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
