//! `crepus web` — `.crepus`-first static sites (WASM runtime) + dev server.
//!
//! Commands: new, build, dev (alias: serve), build-full.
//!
//! Production builds mirror `crepus webext`: compile the site `runtime/` crate to
//! `wasm32-unknown-unknown`, run `wasm-bindgen`, ship `crepus-bundle.json` + a thin HTML shell.
//! The default WASM entrypoint calls `crepuscularity_web::render_bundle`; sites that need Rust
//! context can replace that with `render_from_files` + a hand-built `TemplateContext`.

use std::path::PathBuf;

use crate::cli::{WebCommands, WebEmitTarget};
use crate::ui;

const WEB_INDEX_HTML: &str = include_str!("../assets/web/index.html");
const WEB_APP_JS: &str = include_str!("../assets/web/app.js");
const WEB_RUNTIME_WORKER_JS: &str = include_str!("../assets/web/runtime-worker.js");
const WEB_RUNTIME_CLIENT_JS: &str = include_str!("../assets/web/runtime-client.js");
const WEB_RUNTIME_ASSETS: [(&str, &str); 3] = [
    ("app.js", WEB_APP_JS),
    ("runtime-worker.js", WEB_RUNTIME_WORKER_JS),
    ("runtime-client.js", WEB_RUNTIME_CLIENT_JS),
];

pub mod build;
pub mod emit;
pub mod full;
mod module_emit;
pub(crate) use module_emit::absolute_resolved;
pub mod scaffold;
mod source_validate;

pub(crate) use build::{
    build_site_wasm, ensure_web_dev_artifacts, is_framework_template, load_site_head,
    merge_site_head_meta, merged_site_google_fonts, merged_site_inline_css, render_index_html,
    ThemeCss, WebBuildArgs,
};

/// Directory under a site root where `crepus web dev` caches UnoCSS, `app.js`, and wasm-bindgen output.
pub(crate) const WEB_DEV_ARTIFACT_DIR: &str = ".crepus-dev";

pub(crate) fn normalize_bundle_entry(entry: &str) -> String {
    use std::path::{Component, Path};

    let (path, suffix) = entry
        .split_once('#')
        .map_or((entry, None), |(path, name)| (path, Some(name)));
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) =>
            {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    let mut entry = normalized.to_string_lossy().replace('\\', "/");
    if let Some(name) = suffix {
        entry.push('#');
        entry.push_str(name);
    }
    entry
}

fn normalize_fullwidth_braces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut quote = None;
    let mut escaped = false;
    for c in s.chars() {
        if let Some(delimiter) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == delimiter {
                quote = None;
            }
        } else {
            if matches!(c, '\'' | '"') {
                quote = Some(c);
            }
            out.push(match c {
                '\u{FF5B}' => '{',
                '\u{FF5D}' => '}',
                _ => c,
            });
        }
    }
    out
}

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn execute(cmd: WebCommands) {
    match cmd {
        WebCommands::New { name } => scaffold::scaffold_site(&name),
        WebCommands::Build {
            build,
            site,
            out_dir,
            output,
            entry,
            target_id,
            manifest,
            emit,
        } => {
            if emit != WebEmitTarget::Html {
                emit::build_emit_stub(emit, site, out_dir.or(output), entry, target_id, manifest);
                return;
            }
            let b = build::WebBuildArgs {
                site_dir: site,
                out_dir: out_dir.or(output),
                entry,
                target_id,
                manifest,
                meta: None,
                options: build.into_options_or_exit(),
            };
            build::build_site_wasm(&b);
        }
        WebCommands::Dev {
            site,
            port,
            entry,
            target_id,
            manifest,
            emit,
        } => {
            if emit != WebEmitTarget::Html {
                ui::warning(&format!(
                    "`crepus web dev --emit {:?}` is ignored; the dev server always serves the HTML/WASM site",
                    emit
                ));
            }
            let opts = resolve_dev_options(site, port, entry, target_id, manifest);
            crate::web_serve::run(opts);
        }
        WebCommands::BuildFull {
            site,
            wasm,
            server,
            emit,
        } => {
            if emit != WebEmitTarget::Html {
                ui::warning(&format!(
                    "`crepus web build-full --emit {:?}` is ignored; build-full always emits HTML/WASM",
                    emit
                ));
            }
            let site_dir = site.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|e| {
                    ui::error(&format!("cannot determine current directory: {e}"));
                })
            });
            full::run_build_full(&full::BuildFullArgs {
                site_dir,
                wasm,
                server,
            });
        }
    }
}

fn resolve_dev_options(
    site: Option<PathBuf>,
    port: u16,
    entry: String,
    target_id: Option<String>,
    manifest: Option<PathBuf>,
) -> crate::web_serve::ServeOptions {
    let explicit_site = site.is_some();
    let mut site_dir =
        site.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let entry_from_args = entry != "index.crepus";
    let mut entry = entry;
    let mut meta = None;

    if explicit_site {
        if let Some(targets) =
            crate::crepus_toml::load_web_targets(Some(site_dir.join("crepus.toml")))
        {
            let picked = crate::crepus_toml::resolve_pick(&targets, target_id.as_deref())
                .unwrap_or_else(|m| ui::error(&m));
            if !entry_from_args {
                entry = picked.entry;
            }
            meta = Some(picked.meta);
        }
    } else if let Some(targets) = crate::crepus_toml::load_web_targets(manifest) {
        let picked = crate::crepus_toml::resolve_pick(&targets, target_id.as_deref())
            .unwrap_or_else(|m| ui::error(&m));
        site_dir = picked.site_dir;
        if !entry_from_args {
            entry = picked.entry;
        }
        meta = Some(picked.meta);
    }

    crate::web_serve::ServeOptions {
        site_dir,
        port,
        entry,
        meta,
    }
}

#[cfg(test)]
mod brace_normalization_tests {
    use super::normalize_fullwidth_braces;

    #[test]
    fn bundle_entries_keep_roots_parent_prefixes_and_component_names() {
        for (input, expected) in [
            ("./index.crepus", "index.crepus"),
            ("pages/../index.crepus", "index.crepus"),
            ("./pages/./main.crepus#Card", "pages/main.crepus#Card"),
            ("../../index.crepus", "../../index.crepus"),
            ("../pages/../index.crepus", "../index.crepus"),
            ("/site/index.crepus", "/site/index.crepus"),
            ("", ""),
            (".", ""),
        ] {
            assert_eq!(super::normalize_bundle_entry(input), expected);
        }
    }

    #[test]
    fn normalization_preserves_quoted_literal_braces() {
        assert_eq!(
            normalize_fullwidth_braces("div value=｛name｝"),
            "div value={name}"
        );
        let source = r#"span "Unicode ｛ ｝ and escaped \"quotes\"""#;
        assert_eq!(normalize_fullwidth_braces(source), source);
        assert_eq!(
            normalize_fullwidth_braces("div title='｛literal｝'"),
            "div title='｛literal｝'"
        );
    }
}
