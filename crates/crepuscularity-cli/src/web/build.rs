use console::style;
use crepuscularity_core::preprocess::{
    extract_head_block, google_font_css_family_name, google_fonts_head_markup,
    merge_unique_font_families, strip_indent_decorators,
};
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::build_options::BuildOptions;
use crate::docs_generator;
use crate::error::CrepusCliError;
use crate::ui;
use crate::wasm_bundle::{
    cargo_build_wasm32, find_wasm_file, run_wasm_bindgen, run_wasm_opt, wasm_profile_dir,
    WasmOptStatus,
};
use crepuscularity_web::{escape_html, render_bundle_with_ssr};

// ── WASM build ───────────────────────────────────────────────────────────────

struct WasmBuildArgs {
    site_dir: PathBuf,
    out_dir: PathBuf,
    entry: String,
    meta: Option<crate::crepus_toml::WebTargetMeta>,
    options: BuildOptions,
}

pub(crate) struct WebBuildArgs {
    pub(crate) site_dir: Option<PathBuf>,
    pub(crate) out_dir: Option<PathBuf>,
    pub(crate) entry: Option<String>,
    pub(crate) target_id: Option<String>,
    pub(crate) manifest: Option<PathBuf>,
    pub(crate) meta: Option<crate::crepus_toml::WebTargetMeta>,
    pub(crate) options: BuildOptions,
}

fn resolve_wasm_build_args(args: &WebBuildArgs) -> WasmBuildArgs {
    let targets = if let Some(manifest) = &args.manifest {
        crate::crepus_toml::load_web_targets(Some(manifest.clone()))
    } else if let Some(site) = &args.site_dir {
        // Manifest-target builds already supply resolved metadata; do not select again.
        if args.meta.is_some() {
            None
        } else {
            crate::crepus_toml::try_load_web_targets(Some(site.join("crepus.toml")))
                .unwrap_or_else(|error| ui::error(&error))
        }
    } else {
        crate::crepus_toml::load_web_targets(None)
    };
    let picked = targets.as_ref().map(|targets| {
        crate::crepus_toml::resolve_pick(targets, args.target_id.as_deref())
            .unwrap_or_else(|error| ui::error(&error))
    });
    if picked.is_none() && (args.manifest.is_some() || args.target_id.is_some()) {
        let source = args
            .manifest
            .as_ref()
            .or(args.site_dir.as_ref())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "the current directory".into());
        ui::error(&format!("cannot load web targets from {source}"));
    }
    let site_dir = args
        .site_dir
        .clone()
        .or_else(|| picked.as_ref().map(|target| target.site_dir.clone()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let out_dir = args
        .out_dir
        .clone()
        .or_else(|| picked.as_ref().map(|target| target.out_dir.clone()))
        .unwrap_or_else(|| site_dir.join("dist"));
    let entry = args
        .entry
        .clone()
        .or_else(|| picked.as_ref().map(|target| target.entry.clone()))
        .unwrap_or_else(|| "index.crepus".into());
    let meta = args
        .meta
        .clone()
        .or_else(|| picked.as_ref().map(|target| target.meta.clone()));
    WasmBuildArgs {
        site_dir,
        out_dir,
        entry: super::normalize_bundle_entry(&entry),
        meta,
        options: args.options,
    }
}

pub(crate) fn build_site_wasm(cli: &WebBuildArgs) {
    let t0 = Instant::now();
    let b = resolve_wasm_build_args(cli);
    validate_static_output(&b.site_dir, &b.out_dir).unwrap_or_else(|error| ui::error(&error));
    let runtime_dir = b.site_dir.join("runtime");
    if !runtime_dir.join("Cargo.toml").is_file() {
        ui::error(&format!(
            "no runtime/Cargo.toml under {} — run `crepus web new <name>` or copy examples/web-site",
            b.site_dir.display()
        ));
    }

    let label = if b.options.release() {
        "crepus web release"
    } else {
        "crepus web debug"
    };
    eprintln!(
        "{} building WASM site → {}",
        style(label).dim(),
        style(b.out_dir.display().to_string()).cyan()
    );
    eprintln!();

    let mut files = load_and_validate_files(&b);
    let template_head_html = extract_and_render_head(&b.entry, &mut files);
    let llms_site_text = entry_readable_text(&b.entry, &files);
    let bundle_str = write_bundle(&b, &files);

    let head = load_site_head(&b.site_dir);
    let head = merge_site_head_meta(head, b.meta.as_ref());
    let head = merge_llms_alternates(head, b.meta.as_ref());

    generate_index_and_assets(&b, &files, &bundle_str, &head, &template_head_html);
    process_docs_and_static(&b, &files, &head, llms_site_text.as_deref());
    compile_and_optimize_wasm(&b, &runtime_dir);

    eprintln!(
        "\n{} wrote {}",
        ui::ok(),
        style(b.out_dir.display().to_string()).cyan()
    );
    eprintln!(
        "  {} open {}/index.html via a static server (fetch + WASM modules need HTTP)",
        ui::dim("→"),
        b.out_dir.display()
    );
    ui::done_in(t0.elapsed());
}

fn load_and_validate_files(b: &WasmBuildArgs) -> HashMap<String, String> {
    let mut files = HashMap::new();
    load_all_crepus(&b.site_dir, &b.site_dir, &mut files);
    if files.is_empty() {
        ui::error(&format!("no .crepus files under {}", b.site_dir.display()));
    }
    let entry_path = b
        .entry
        .split_once('#')
        .map_or(b.entry.as_str(), |(path, _)| path);
    if !files.contains_key(entry_path) {
        ui::error(&format!(
            "entry {:?} not found in virtual file map (keys: {:?})",
            b.entry,
            files.keys().take(5).collect::<Vec<_>>()
        ));
    }
    if let Some((path, name)) = b.entry.split_once('#') {
        let components = crepuscularity_core::parse_component_file(&files[path])
            .unwrap_or_else(|e| ui::error(&format!("parse entry {}: {e}", b.entry)));
        if !components.components.contains_key(name) {
            ui::error(&format!("component not found: {name} in {path}"));
        }
    }
    files
}

fn extract_and_render_head(entry: &str, files: &mut HashMap<String, String>) -> String {
    let entry = entry.split_once('#').map_or(entry, |(path, _)| path);
    let mut template_head_html = String::new();
    if let Some(entry_content) = files.get(entry).cloned() {
        let (head_raw, body_raw) = extract_head_block(&entry_content);
        if let Some(head_src) = head_raw {
            template_head_html = render_head_raw(&head_src);
            files.insert(entry.to_string(), body_raw);
        }
    }
    template_head_html
}

fn write_bundle(b: &WasmBuildArgs, files: &HashMap<String, String>) -> String {
    let bundle = json!({
        "entry": b.entry,
        "files": files,
    });
    let bundle_str = serde_json::to_string(&bundle).unwrap_or_else(|e| {
        ui::error(&format!("serialize bundle: {e}"));
    });

    std::fs::create_dir_all(&b.out_dir).unwrap_or_else(|e| {
        ui::error(&format!("mkdir {}: {e}", b.out_dir.display()));
    });
    let bundle_path = b.out_dir.join("crepus-bundle.json");
    let skip_bundle_write = std::fs::read(&bundle_path)
        .is_ok_and(|existing| existing.as_slice() == bundle_str.as_bytes());
    if !skip_bundle_write {
        std::fs::write(&bundle_path, &bundle_str).unwrap_or_else(|e| {
            ui::error(&format!("write {}: {e}", bundle_path.display()));
        });
    }
    bundle_str
}

fn generate_index_and_assets(
    b: &WasmBuildArgs,
    files: &HashMap<String, String>,
    bundle_str: &str,
    head: &SiteHead,
    template_head_html: &str,
) {
    let google_fonts = merged_site_google_fonts(&b.site_dir, files, b.meta.as_ref());
    let inline_css = merged_site_inline_css(files);
    let vendor_dir = b.out_dir.join("vendor");

    std::fs::create_dir_all(&vendor_dir)
        .unwrap_or_else(|e| ui::error(&format!("mkdir vendor: {e}")));
    copy_unocss(&vendor_dir);

    let mut index_html = render_index_html(head, &google_fonts, &inline_css, template_head_html);
    if let Ok(ssr_html) = render_bundle_with_ssr(bundle_str, true) {
        let needle = r#"<div id="crepus-root"></div>"#;
        if let Some(pos) = index_html.find(needle) {
            let bundle_escaped = ssr_escape_json(bundle_str);
            let replacement = format!(
                r#"<div id="crepus-root">{}</div><script id="__crepus_bundle__" type="application/json">{}</script>"#,
                ssr_html, bundle_escaped
            );
            index_html.replace_range(pos..pos + needle.len(), &replacement);
        }
    }
    std::fs::write(b.out_dir.join(".nojekyll"), b"")
        .unwrap_or_else(|e| ui::error(&format!("write .nojekyll: {e}")));
    std::fs::write(b.out_dir.join("index.html"), index_html)
        .unwrap_or_else(|e| ui::error(&format!("write index.html: {e}")));
    for (name, source) in super::WEB_RUNTIME_ASSETS {
        std::fs::write(b.out_dir.join(name), source)
            .unwrap_or_else(|e| ui::error(&format!("write {name}: {e}")));
    }
}

fn process_docs_and_static(
    b: &WasmBuildArgs,
    files: &HashMap<String, String>,
    head: &SiteHead,
    llms_site_text: Option<&str>,
) {
    if let Some(docs) = b.meta.as_ref().and_then(|meta| meta.docs.as_ref()) {
        let theme = crate::web_docs_hook::DocsHookTheme {
            accent: head.theme.accent.clone(),
            accent_soft: head.theme.accent_soft.clone(),
            surface: head.theme.surface.clone(),
            text: head.theme.text.clone(),
            muted: head.theme.muted.clone(),
            border: head.theme.border.clone(),
        };
        if let Err(e) = crate::web_docs_hook::run_docs_hook(
            &b.site_dir,
            &b.out_dir.join("docs"),
            docs,
            &head.page_title,
            &theme,
        ) {
            ui::error(&format!("run docs hook: {e}"));
        }
    }

    let static_src = b.site_dir.join("static");
    if static_src.is_dir() {
        copy_dir_recursive(&static_src, &b.out_dir.join("static"))
            .unwrap_or_else(|e| ui::error(&format!("copy static/: {e}")));
    }

    if let Some(src) = docs_src_candidate(&b.site_dir) {
        let has_md = std::fs::read_dir(&src).is_ok_and(|mut d| {
            d.any(|e| {
                e.ok()
                    .is_some_and(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            })
        });
        if has_md && b.meta.as_ref().and_then(|m| m.docs.as_ref()).is_none() {
            if let Err(e) = docs_generator::generate_docs(
                &src,
                &b.out_dir.join("docs"),
                &head.theme,
                &head.page_title,
            ) {
                ui::warning(&format!("docs generation: {e}"));
            }
        }
    }

    if let Err(e) = crate::web_islands::build_web_islands(&b.site_dir, &b.out_dir, files, &b.entry)
    {
        ui::error(&e);
    }

    write_seo_files(&b.out_dir, head);
    write_llms_files(
        &b.site_dir,
        &b.out_dir,
        head,
        b.meta.as_ref(),
        llms_site_text,
    );
}

fn compile_and_optimize_wasm(b: &WasmBuildArgs, runtime_dir: &Path) {
    let pkg_dir = b.out_dir.join("pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap_or_else(|e| ui::error(&format!("mkdir pkg: {e}")));

    let sp = ui::spinner("compiling site WASM (wasm32-unknown-unknown)");
    match cargo_build_wasm32(runtime_dir, b.options) {
        Ok(()) => ui::spinner_ok(&sp, "WASM compiled"),
        Err(stderr) => {
            sp.finish_and_clear();
            eprintln!("  {} WASM compile failed", ui::err());
            for line in stderr.lines().take(20) {
                eprintln!("    {}", style(line).dim());
            }
            ui::error("fix runtime compile errors (see above)");
        }
    }

    let target = wasm_profile_dir(runtime_dir, b.options).unwrap_or_else(|error| ui::error(&error));
    let wasm_path = find_wasm_file(&target, runtime_dir).unwrap_or_else(|| {
        ui::error(&format!(
            "runtime library .wasm not found in {} (check runtime Cargo.toml [lib] crate-type)",
            target.display()
        ))
    });

    let sp = ui::spinner("wasm-bindgen");
    match run_wasm_bindgen(&wasm_path, &pkg_dir, "runtime") {
        Ok(()) => ui::spinner_ok(&sp, "pkg/runtime.js + runtime_bg.wasm"),
        Err(err) => {
            sp.finish_and_clear();
            if err.starts_with("wasm-bindgen:") {
                ui::error("wasm-bindgen not found — install: cargo install wasm-bindgen-cli");
            }
            ui::error(&format!("wasm-bindgen: {err}"));
        }
    }

    if b.options.optimize_artifacts() {
        let wasm = pkg_dir.join("runtime_bg.wasm");
        if wasm.is_file() {
            let sp = ui::spinner("optimizing WASM");
            match run_wasm_opt(&wasm, b.options.optimization) {
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

fn load_all_crepus(root: &Path, dir: &Path, map: &mut HashMap<String, String>) {
    // HTML bundles resolve includes exclusively from this supplied source map.
    load_all_crepus_inner(root, dir, map, &mut HashSet::new(), true);
}

fn load_all_crepus_inner(
    root: &Path,
    dir: &Path,
    map: &mut HashMap<String, String>,
    ancestors: &mut HashSet<PathBuf>,
    framework_templates: bool,
) {
    let Ok(canonical) = std::fs::canonicalize(dir) else {
        return;
    };
    // Sibling aliases keep their lexical bundle paths; only active ancestors are cycles.
    if !ancestors.insert(canonical.clone()) {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip output and cargo targets
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if matches!(
                    name,
                    "dist" | "target" | ".git" | "node_modules" | ".crepus-dev"
                ) {
                    continue;
                }
                load_all_crepus_inner(root, &path, map, ancestors, framework_templates);
            } else if path.extension().is_some_and(|e| e == "crepus")
                || (framework_templates && is_framework_template(&path))
            {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let key = relative_key(root, &path);
                    let content = if path.extension().is_some_and(|e| e == "crepus") {
                        super::normalize_fullwidth_braces(&content)
                    } else {
                        content
                    };
                    map.insert(key, content);
                }
            }
        }
    }
    ancestors.remove(&canonical);
}

pub(crate) fn is_framework_template(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "crepus" | "jsx" | "tsx" | "csx" | "svelte" | "vue" | "astro" | "ng" | "html"
    )
}

/// Collect supported template frontends for framework source emission.
pub(crate) fn load_framework_templates(root: &Path, dir: &Path, map: &mut HashMap<String, String>) {
    load_all_crepus_inner(root, dir, map, &mut HashSet::new(), true);
}

pub(crate) struct EmitPaths {
    pub(crate) site_dir: PathBuf,
    pub(crate) out_dir: PathBuf,
    pub(crate) entry: String,
}

/// Resolve site/out/entry for non-html `--emit` builds (no runtime/ WASM required).
pub(crate) fn resolve_emit_paths(
    site: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    entry: Option<String>,
    target_id: Option<String>,
    manifest: Option<PathBuf>,
) -> EmitPaths {
    let resolved = resolve_wasm_build_args(&WebBuildArgs {
        site_dir: site,
        out_dir,
        entry,
        target_id,
        manifest,
        meta: None,
        options: BuildOptions::debug(),
    });
    EmitPaths {
        site_dir: resolved.site_dir,
        out_dir: resolved.out_dir,
        entry: resolved.entry,
    }
}

fn relative_key(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug, Clone)]
pub(crate) struct SiteHead {
    pub(crate) page_title: String,
    pub(crate) description: String,
    pub(crate) og_image: Option<String>,
    pub(crate) extra_head_html: String,
    pub(crate) theme: ThemeCss,
    pub(crate) seo: crate::crepus_toml::SeoConfig,
}

impl Default for SiteHead {
    fn default() -> Self {
        Self {
            page_title: "Crepus site".into(),
            description: "Built with Crepuscularity".into(),
            og_image: None,
            extra_head_html: String::new(),
            theme: ThemeCss::default(),
            seo: crate::crepus_toml::SeoConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeCss {
    pub(crate) accent: String,
    pub(crate) accent_soft: String,
    pub(crate) surface: String,
    pub(crate) text: String,
    pub(crate) muted: String,
    pub(crate) border: String,
}

impl Default for ThemeCss {
    fn default() -> Self {
        Self {
            accent: "#3b82f6".into(),
            accent_soft: "#60a5fa".into(),
            surface: "#09090b".into(),
            text: "#fafafa".into(),
            muted: "#a1a1aa".into(),
            border: "#27272a".into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SiteJsonPartial {
    business_name: Option<String>,
    title: Option<String>,
    seo: Option<SeoPartial>,
    theme: Option<ThemePartial>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeoPartial {
    title: Option<String>,
    description: Option<String>,
    og_image: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemePartial {
    accent: Option<String>,
    accent_soft: Option<String>,
    surface: Option<String>,
    text: Option<String>,
    muted: Option<String>,
    border: Option<String>,
}

/// Merge `crepus.toml` target fonts with every `google-font` pragma in bundled `.crepus` files.
pub(crate) fn merged_site_google_fonts(
    _site_dir: &Path,
    files: &HashMap<String, String>,
    meta: Option<&crate::crepus_toml::WebTargetMeta>,
) -> Vec<String> {
    let mut collected = Vec::new();
    if let Some(meta) = meta {
        collected.extend(meta.google_fonts.clone());
    }
    for content in files.values() {
        collected.extend(strip_indent_decorators(content).google_fonts);
    }
    merge_unique_font_families(collected)
}

pub(crate) fn merged_site_inline_css(files: &HashMap<String, String>) -> String {
    let mut blocks: Vec<String> = Vec::new();
    for content in files.values() {
        let css = strip_indent_decorators(content).inline_css;
        if !css.trim().is_empty() {
            blocks.push(css.trim().to_string());
        }
    }
    blocks.join("\n\n")
}

pub(crate) fn load_site_head(site_dir: &Path) -> SiteHead {
    let mut head = SiteHead::default();

    let path = site_dir.join("site.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(partial) = serde_json::from_str::<SiteJsonPartial>(&raw) {
            let seo_title = partial.seo.as_ref().and_then(|s| s.title.clone());
            head.page_title = partial
                .title
                .clone()
                .or(seo_title)
                .or(partial.business_name.clone())
                .unwrap_or_else(|| head.page_title.clone());
            if let Some(seo) = &partial.seo {
                if let Some(d) = &seo.description {
                    head.description = d.clone();
                    head.seo.description = Some(d.clone());
                }
                head.og_image = seo.og_image.clone();
                head.seo.title = seo.title.clone();
                head.seo.image = seo.og_image.clone();
            }
            if let Some(t) = &partial.theme {
                let mut th = ThemeCss::default();
                if let Some(x) = &t.accent {
                    th.accent = x.clone();
                }
                if let Some(x) = &t.accent_soft {
                    th.accent_soft = x.clone();
                }
                if let Some(x) = &t.surface {
                    th.surface = x.clone();
                }
                if let Some(x) = &t.text {
                    th.text = x.clone();
                }
                if let Some(x) = &t.muted {
                    th.muted = x.clone();
                }
                if let Some(x) = &t.border {
                    th.border = x.clone();
                }
                head.theme = th;
            }
        }
    }

    head
}

pub(crate) fn merge_site_head_meta(
    mut head: SiteHead,
    meta: Option<&crate::crepus_toml::WebTargetMeta>,
) -> SiteHead {
    let Some(meta) = meta else {
        return head;
    };
    if let Some(name) = &meta.name {
        head.page_title = name.clone();
    }
    if let Some(description) = &meta.description {
        head.description = description.clone();
    }
    if let Some(head_html) = &meta.head_html {
        head.extra_head_html = head_html.clone();
    }
    if let Some(seo) = &meta.seo {
        head.seo = seo.clone();
        if let Some(title) = &seo.title {
            head.page_title = title.clone();
        }
        if let Some(description) = &seo.description {
            head.description = description.clone();
        }
        if let Some(image) = &seo.image {
            head.og_image = Some(image.clone());
        }
    }
    head
}

pub(crate) fn render_index_html(
    head: &SiteHead,
    google_fonts: &[String],
    inline_css: &str,
    template_head: &str,
) -> String {
    let seo = render_seo_head(head);
    let font_markup = google_fonts_head_markup(google_fonts);
    let body_font_css = google_fonts
        .first()
        .map(|n| {
            let q = google_font_css_family_name(n)
                .replace('\\', r"\\")
                .replace('"', r#"\""#);
            format!(r#""{q}", system-ui, -apple-system, sans-serif"#)
        })
        .unwrap_or_else(|| "system-ui, -apple-system, sans-serif".to_string());
    let t = &head.theme;
    let inline_css_tag = if inline_css.trim().is_empty() {
        String::new()
    } else {
        format!("<style>\n{}\n</style>", inline_css)
    };
    let extra_head = {
        let mut parts: Vec<&str> = Vec::new();
        if !template_head.trim().is_empty() {
            parts.push(template_head.trim());
        }
        if !head.extra_head_html.trim().is_empty() {
            parts.push(head.extra_head_html.trim());
        }
        if !inline_css_tag.trim().is_empty() {
            parts.push(inline_css_tag.trim());
        }
        parts.join("\n")
    };

    let html_title = head.seo.title.as_deref().unwrap_or(&head.page_title);
    let html_description = head.seo.description.as_deref().unwrap_or(&head.description);

    super::WEB_INDEX_HTML
        .replace("__CREPUS_TITLE__", &escape_html(html_title))
        .replace("__CREPUS_DESC__", &escape_html(html_description))
        .replace("__CREPUS_OG__", &seo)
        .replace("__CREPUS_GOOGLE_FONTS__", &font_markup)
        .replace("__CREPUS_EXTRA_HEAD__", &extra_head)
        .replace("__CREPUS_NOSCRIPT__", &render_llms_noscript(head))
        .replace("__CREPUS_BODY_FONT__", &body_font_css)
        .replace("__THEME_ACCENT__", &escape_html(&t.accent))
        .replace("__THEME_ACCENT_SOFT__", &escape_html(&t.accent_soft))
        .replace("__THEME_SURFACE__", &escape_html(&t.surface))
        .replace("__THEME_TEXT__", &escape_html(&t.text))
        .replace("__THEME_MUTED__", &escape_html(&t.muted))
        .replace("__THEME_BORDER__", &escape_html(&t.border))
}

/// Escape `<` so JSON cannot close its HTML script element or start an HTML comment.
pub(crate) fn ssr_escape_json(json: &str) -> String {
    json.replace('<', "\\u003c")
}

#[test]
fn embedded_json_cannot_close_script_with_mixed_case_tag() {
    let value = serde_json::json!({"text": "</sCrIpT><script>alert(1)</script><!--"});
    let escaped = ssr_escape_json(&value.to_string());
    assert!(!escaped.contains('<'));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&escaped).unwrap(),
        value
    );
}

struct ResolvedSeoTags<'a> {
    title: &'a str,
    description: &'a str,
    og_type: &'a str,
    image: Option<&'a String>,
    twitter_card: &'a str,
}

fn render_seo_head(head: &SiteHead) -> String {
    let seo = &head.seo;
    let image = seo.image.as_ref().or(head.og_image.as_ref());
    let resolved = ResolvedSeoTags {
        title: seo.title.as_deref().unwrap_or(&head.page_title),
        description: seo.description.as_deref().unwrap_or(&head.description),
        og_type: seo.og_type.as_deref().unwrap_or("website"),
        image,
        twitter_card: seo.twitter_card.as_deref().unwrap_or_else(|| {
            if image.is_some() {
                "summary_large_image"
            } else {
                "summary"
            }
        }),
    };
    format_seo_tags(head, &resolved)
}

fn format_seo_tags(head: &SiteHead, resolved: &ResolvedSeoTags<'_>) -> String {
    let seo = &head.seo;
    let mut lines = Vec::new();

    if let Some(canonical) = &seo.canonical {
        lines.push(format!(
            r#"  <link rel="canonical" href="{}">"#,
            escape_html(canonical)
        ));
    }
    if !seo.keywords.is_empty() {
        lines.push(format!(
            r#"  <meta name="keywords" content="{}">"#,
            escape_html(&seo.keywords.join(", "))
        ));
    }
    if let Some(author) = &seo.author {
        lines.push(format!(
            r#"  <meta name="author" content="{}">"#,
            escape_html(author)
        ));
    }
    if let Some(robots) = &seo.robots {
        lines.push(format!(
            r#"  <meta name="robots" content="{}">"#,
            escape_html(robots)
        ));
    }
    if let Some(theme_color) = &seo.theme_color {
        lines.push(format!(
            r#"  <meta name="theme-color" content="{}">"#,
            escape_html(theme_color)
        ));
    }
    if let Some(application_name) = &seo.application_name {
        lines.push(format!(
            r#"  <meta name="application-name" content="{}">"#,
            escape_html(application_name)
        ));
    }
    if let Some(generator) = &seo.generator {
        if !generator.trim().is_empty() {
            lines.push(format!(
                r#"  <meta name="generator" content="{}">"#,
                escape_html(generator)
            ));
        }
    }

    lines.push(format!(
        r#"  <meta property="og:title" content="{}">"#,
        escape_html(resolved.title)
    ));
    lines.push(format!(
        r#"  <meta property="og:description" content="{}">"#,
        escape_html(resolved.description)
    ));
    lines.push(format!(
        r#"  <meta property="og:type" content="{}">"#,
        escape_html(resolved.og_type)
    ));
    if let Some(canonical) = &seo.canonical {
        lines.push(format!(
            r#"  <meta property="og:url" content="{}">"#,
            escape_html(canonical)
        ));
    }
    if let Some(site_name) = &seo.site_name {
        lines.push(format!(
            r#"  <meta property="og:site_name" content="{}">"#,
            escape_html(site_name)
        ));
    }
    if let Some(locale) = &seo.locale {
        lines.push(format!(
            r#"  <meta property="og:locale" content="{}">"#,
            escape_html(locale)
        ));
    }
    if let Some(image) = resolved.image {
        lines.push(format!(
            r#"  <meta property="og:image" content="{}">"#,
            escape_html(image)
        ));
    }
    if let Some(image_alt) = &seo.image_alt {
        lines.push(format!(
            r#"  <meta property="og:image:alt" content="{}">"#,
            escape_html(image_alt)
        ));
    }

    lines.push(format!(
        r#"  <meta name="twitter:card" content="{}">"#,
        escape_html(resolved.twitter_card)
    ));
    lines.push(format!(
        r#"  <meta name="twitter:title" content="{}">"#,
        escape_html(resolved.title)
    ));
    lines.push(format!(
        r#"  <meta name="twitter:description" content="{}">"#,
        escape_html(resolved.description)
    ));
    if let Some(image) = resolved.image {
        lines.push(format!(
            r#"  <meta name="twitter:image" content="{}">"#,
            escape_html(image)
        ));
    }
    if let Some(image_alt) = &seo.image_alt {
        lines.push(format!(
            r#"  <meta name="twitter:image:alt" content="{}">"#,
            escape_html(image_alt)
        ));
    }
    if let Some(site) = &seo.twitter_site {
        lines.push(format!(
            r#"  <meta name="twitter:site" content="{}">"#,
            escape_html(site)
        ));
    }
    if let Some(creator) = &seo.twitter_creator {
        lines.push(format!(
            r#"  <meta name="twitter:creator" content="{}">"#,
            escape_html(creator)
        ));
    }

    for alt in &seo.alternates {
        let mut attrs = vec![
            r#"rel="alternate""#.to_string(),
            format!(r#"href="{}""#, escape_html(&alt.href)),
        ];
        if let Some(hreflang) = &alt.hreflang {
            attrs.push(format!(r#"hreflang="{}""#, escape_html(hreflang)));
        }
        if let Some(media) = &alt.media {
            attrs.push(format!(r#"media="{}""#, escape_html(media)));
        }
        if let Some(title) = &alt.title {
            attrs.push(format!(r#"title="{}""#, escape_html(title)));
        }
        if let Some(mime_type) = &alt.mime_type {
            attrs.push(format!(r#"type="{}""#, escape_html(mime_type)));
        }
        lines.push(format!("  <link {}>", attrs.join(" ")));
    }

    for json in &seo.json_ld {
        if !json.trim().is_empty() {
            lines.push(format!(
                r#"  <script type="application/ld+json">{}</script>"#,
                json.trim()
            ));
        }
    }

    if let Some(favicon) = &seo.favicon {
        let href = escape_html(favicon);
        if favicon.ends_with(".svg") {
            lines.push(format!(
                r#"  <link rel="icon" type="image/svg+xml" href="{}">"#,
                href
            ));
        } else if favicon.ends_with(".ico") {
            lines.push(format!(
                r#"  <link rel="icon" type="image/x-icon" href="{}">"#,
                href
            ));
        } else if favicon.ends_with(".png") {
            lines.push(format!(
                r#"  <link rel="icon" type="image/png" href="{}">"#,
                href
            ));
        } else {
            lines.push(format!(r#"  <link rel="icon" href="{}">"#, href));
        }
    }
    if let Some(touch) = &seo.apple_touch_icon {
        lines.push(format!(
            r#"  <link rel="apple-touch-icon" href="{}">"#,
            escape_html(touch)
        ));
    }

    lines.join("\n")
}

fn merge_llms_alternates(
    mut head: SiteHead,
    meta: Option<&crate::crepus_toml::WebTargetMeta>,
) -> SiteHead {
    let Some(llms) = meta.and_then(|m| m.llms.as_ref()) else {
        return head;
    };
    if !llms.enabled {
        return head;
    }
    let base = llms_base_url(llms, &head);
    let alts = [
        ("llms.txt", "LLM index", "text/plain"),
        ("llms-full.txt", "Full LLM bundle", "text/plain"),
        ("agent.md", "Agent guide", "text/markdown"),
    ];
    for (path, title, mime_type) in alts {
        let href = format!("{base}/{path}");
        if head.seo.alternates.iter().any(|alt| alt.href == href) {
            continue;
        }
        head.seo.alternates.push(crate::crepus_toml::SeoAlternate {
            href,
            hreflang: None,
            media: None,
            title: Some(title.into()),
            mime_type: Some(mime_type.into()),
        });
    }
    head
}

fn write_seo_files(out_dir: &Path, head: &SiteHead) {
    let seo = &head.seo;
    if let Some(sitemap) = &seo.sitemap {
        if sitemap.enabled {
            let discovered_paths = discover_html_sitemap_paths(out_dir);
            if let Some(xml) = render_sitemap_xml(sitemap, &discovered_paths) {
                std::fs::write(out_dir.join("sitemap.xml"), xml)
                    .unwrap_or_else(|e| ui::error(&format!("write sitemap.xml: {e}")));
            }
        }
    }
    if let Some(robots) = &seo.robots_txt {
        if robots.enabled {
            let txt = render_robots_txt(robots, seo);
            std::fs::write(out_dir.join("robots.txt"), txt)
                .unwrap_or_else(|e| ui::error(&format!("write robots.txt: {e}")));
        }
    }
}

fn write_llms_files(
    site_dir: &Path,
    out_dir: &Path,
    head: &SiteHead,
    meta: Option<&crate::crepus_toml::WebTargetMeta>,
    site_text: Option<&str>,
) {
    let Some(llms) = meta.and_then(|m| m.llms.as_ref()) else {
        return;
    };
    if !llms.enabled {
        return;
    }
    let base = llms_base_url(llms, head);
    let title = llms
        .title
        .as_deref()
        .or(head.seo.title.as_deref())
        .unwrap_or(&head.page_title);
    let description = llms
        .description
        .as_deref()
        .or(head.seo.description.as_deref())
        .unwrap_or(&head.description);

    let mut index = format!(
        "# {title}\n\n> {description}\n\nStatic Crepus site. Agents should fetch the Markdown/text files below instead of scraping the rendered app shell.\n\n## Markdown\n\n- [Full bundle]({base}/llms-full.txt): One request with the agent guide and readable site text.\n- [Agent guide]({base}/agent.md): Fetch order and hosting notes.\n"
    );
    for source in &llms.sources {
        let label = source.title.as_deref().unwrap_or(&source.path);
        let href = source
            .href
            .clone()
            .unwrap_or_else(|| format!("{base}/{}", source.path.trim_start_matches('/')));
        index.push_str(&format!("- [{label}]({href}): Source Markdown.\n",));
    }

    let agent = format!(
        "# {title} — agent guide\n\n> Use direct static files. Do not scrape the HTML shell unless no Markdown file answers the task.\n\n## Best first request\n\n`curl -sL {base}/llms-full.txt`\n\n## Files\n\n- {base}/llms.txt — compact index\n- {base}/llms-full.txt — full bundle\n{}\n",
        llms.sources
            .iter()
            .map(|source| {
                source
                    .href
                    .clone()
                    .unwrap_or_else(|| format!("{base}/{}", source.path.trim_start_matches('/')))
            })
            .map(|href| format!("- {href}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let mut full = format!("{index}\n---\n\n{agent}");
    if let Some(site_text) = site_text {
        if !site_text.trim().is_empty() {
            full.push_str(&format!(
                "\n---\n\n# Rendered site text\n\n{}\n",
                site_text.trim()
            ));
        }
    }
    let formatted_bodies: Vec<_> = llms
        .sources
        .par_iter()
        .map(|source| {
            let rel = source.path.trim_start_matches('/');
            let path = site_dir.join(rel);
            let body = std::fs::read_to_string(&path).unwrap_or_else(|_| String::new());
            let label = source.title.as_deref().unwrap_or(rel);
            let formatted = format!("\n---\n\n# {label}\n\n{body}\n");
            if !body.is_empty() && !rel.split('/').any(|part| part == "..") {
                if let Some(parent) = Path::new(rel).parent() {
                    std::fs::create_dir_all(out_dir.join(parent))
                        .unwrap_or_else(|e| ui::error(&format!("mkdir llms source dir: {e}")));
                }
                std::fs::write(out_dir.join(rel), body)
                    .unwrap_or_else(|e| ui::error(&format!("write {rel}: {e}")));
            }
            formatted
        })
        .collect();

    for formatted in formatted_bodies {
        full.push_str(&formatted);
    }

    std::fs::write(out_dir.join("llms.txt"), index)
        .unwrap_or_else(|e| ui::error(&format!("write llms.txt: {e}")));
    std::fs::write(out_dir.join("llms-full.txt"), full)
        .unwrap_or_else(|e| ui::error(&format!("write llms-full.txt: {e}")));
    std::fs::write(out_dir.join("agent.md"), agent)
        .unwrap_or_else(|e| ui::error(&format!("write agent.md: {e}")));
}

fn render_llms_noscript(head: &SiteHead) -> String {
    let Some(href) = head
        .seo
        .alternates
        .iter()
        .find(|alt| alt.href.ends_with("/llms-full.txt"))
        .map(|alt| alt.href.as_str())
    else {
        return String::new();
    };
    format!(
        r#"<noscript><p><a href="{}">Agent-readable text version</a></p></noscript>"#,
        escape_html(href)
    )
}

#[derive(Clone)]
struct ReadableElement {
    tag: String,
    href: Option<String>,
    link_output: Option<usize>,
}

fn entry_readable_text(entry: &str, files: &HashMap<String, String>) -> Option<String> {
    if let Some((path, name)) = entry.split_once('#') {
        let source = files.get(path)?;
        let (_, body, _) = crepuscularity_core::parser::split_frontmatter_parts(source);
        let (_, selected, _) = crepuscularity_core::parser::split_component_body_sections(body)
            .into_iter()
            .find(|(component, _, _)| component == name)?;
        Some(render_crepus_readable_text(&selected))
    } else {
        files
            .get(entry)
            .map(|source| render_crepus_readable_text(source))
    }
}

fn render_crepus_readable_text(source: &str) -> String {
    let nodes = match crepuscularity_core::parse_template(source) {
        Ok(nodes) => nodes,
        Err(error) => {
            ui::warning(&format!("extract readable site text: {error}"));
            return String::new();
        }
    };
    let mut out = Vec::new();
    collect_readable_text(&nodes, &mut Vec::new(), &mut out);
    out.join("\n\n")
}

fn collect_readable_text(
    nodes: &[crepuscularity_core::ast::Node],
    stack: &mut Vec<ReadableElement>,
    out: &mut Vec<String>,
) {
    use crepuscularity_core::ast::{Node, TextPart};

    // This is a source summary; dynamic branches and loop bodies are visited once.
    for node in nodes {
        match node {
            Node::Element(element) => {
                let href = element
                    .bindings
                    .iter()
                    .find(|binding| binding.prop == "href")
                    .and_then(|binding| serde_json::from_str::<String>(&binding.value).ok());
                stack.push(ReadableElement {
                    tag: element.tag.clone(),
                    href,
                    link_output: None,
                });
                collect_readable_text(&element.children, stack, out);
                stack.pop();
            }
            Node::Text(parts) => {
                let mut text = String::new();
                for part in parts {
                    match part {
                        TextPart::Literal(value) => text.push_str(value),
                        TextPart::Expr(expr) => {
                            text.push('{');
                            text.push_str(expr);
                            text.push('}');
                        }
                    }
                }
                push_readable_text(out, stack, text);
            }
            Node::RawText(text) => push_readable_text(out, stack, text.clone()),
            Node::If(block) => {
                collect_readable_text(&block.then_children, stack, out);
                if let Some(children) = &block.else_children {
                    collect_readable_text(children, stack, out);
                }
            }
            Node::For(block) => collect_readable_text(&block.body, stack, out),
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_readable_text(&arm.body, stack, out);
                }
            }
            Node::Include(include) => collect_readable_text(&include.slot, stack, out),
            Node::LetDecl(_) | Node::Embed(_) | Node::RawHtml(_) => {}
        }
    }
}

fn push_readable_text(out: &mut Vec<String>, stack: &mut [ReadableElement], text: String) {
    if text.trim().is_empty() {
        return;
    }
    if let Some(tag) = stack
        .iter()
        .rev()
        .find_map(|element| match element.tag.as_str() {
            "h1" => Some("#"),
            "h2" => Some("##"),
            "h3" => Some("###"),
            _ => None,
        })
    {
        out.push(format!("{tag} {text}"));
        return;
    }
    if let Some((href, link_output)) = stack.iter_mut().rev().find_map(|element| {
        element
            .href
            .as_deref()
            .map(|href| (href, &mut element.link_output))
    }) {
        let text = text
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        let suffix = format!("]({href})");
        let may_merge = link_output.is_some() && *link_output == out.len().checked_sub(1);
        if let Some(last) = out
            .last_mut()
            .filter(|last| may_merge && last.ends_with(&suffix))
        {
            last.insert_str(last.len() - suffix.len(), &text);
            return;
        }
        *link_output = Some(out.len());
        out.push(format!("[{text}]({href})"));
        return;
    }
    out.push(text);
}

fn llms_base_url(llms: &crate::crepus_toml::LlmsConfig, head: &SiteHead) -> String {
    llms.base_url
        .as_deref()
        .or(head.seo.canonical.as_deref())
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string()
}

fn render_robots_txt(
    robots: &crate::crepus_toml::RobotsTxtConfig,
    seo: &crate::crepus_toml::SeoConfig,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "User-agent: {}",
        robots.user_agent.as_deref().unwrap_or("*")
    ));
    for allow in &robots.allow {
        lines.push(format!("Allow: {allow}"));
    }
    for disallow in &robots.disallow {
        lines.push(format!("Disallow: {disallow}"));
    }
    if let Some(sitemap) = &robots.sitemap {
        lines.push(format!("Sitemap: {sitemap}"));
    } else if let Some(sitemap) = &seo.sitemap {
        if sitemap.enabled {
            if let Some(base_url) = &sitemap.base_url {
                lines.push(format!(
                    "Sitemap: {}/sitemap.xml",
                    base_url.trim_end_matches('/')
                ));
            }
        }
    }
    for alt in &seo.alternates {
        if alt.href.ends_with("/llms.txt") || alt.href.ends_with("/llms-full.txt") {
            lines.push(format!("# {}", alt.href));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn render_sitemap_xml(
    sitemap: &crate::crepus_toml::SitemapConfig,
    discovered_paths: &[String],
) -> Option<String> {
    let base = sitemap.base_url.as_ref()?.trim_end_matches('/').to_string();
    let mut paths = if sitemap.paths.is_empty() {
        vec!["/".to_string()]
    } else {
        sitemap.paths.clone()
    };
    for path in discovered_paths {
        if !paths.iter().any(|known| known == path) {
            paths.push(path.clone());
        }
    }
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );
    for path in paths {
        let loc = if path.starts_with("http://") || path.starts_with("https://") {
            path
        } else {
            format!("{base}/{}", path.trim_start_matches('/'))
        };
        out.push_str("  <url>\n");
        out.push_str(&format!("    <loc>{}</loc>\n", escape_xml_text(&loc)));
        if let Some(changefreq) = &sitemap.changefreq {
            out.push_str(&format!(
                "    <changefreq>{}</changefreq>\n",
                escape_xml_text(changefreq)
            ));
        }
        if let Some(priority) = sitemap.priority {
            out.push_str(&format!("    <priority>{priority:.1}</priority>\n"));
        }
        out.push_str("  </url>\n");
    }
    out.push_str("</urlset>\n");
    Some(out)
}

fn discover_html_sitemap_paths(out_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    collect_html_sitemap_paths(out_dir, out_dir, &mut paths);
    paths.sort();
    paths
}

fn collect_html_sitemap_paths(base: &Path, dir: &Path, paths: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_html_sitemap_paths(base, &path, paths);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let route = if rel == "index.html" {
            "/".to_string()
        } else if let Some(prefix) = rel.strip_suffix("/index.html") {
            format!("/{prefix}/")
        } else {
            format!("/{rel}")
        };
        if !paths.iter().any(|known| known == &route) {
            paths.push(route);
        }
    }
}

fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Find a docs/ or public/ directory relative to the site dir.
fn docs_src_candidate(site_dir: &Path) -> Option<PathBuf> {
    let dirs = [
        site_dir.join("../docs"),
        site_dir.join("docs"),
        site_dir.join("public"),
    ];
    for d in &dirs {
        if d.is_dir() {
            return Some(d.clone());
        }
    }
    if let Some(parent) = site_dir.parent() {
        let p = parent.join("docs");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

fn render_head_raw(head_raw: &str) -> String {
    let lines: Vec<&str> = head_raw.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if let Some(tail) = trimmed.strip_prefix("title ") {
            let text = tail.trim().trim_matches('"');
            out.push_str(&format!("<title>{}</title>\n", text));
            i += 1;
        } else if let Some(tail) = trimmed.strip_prefix("meta ") {
            out.push_str(&format!("<meta {}>\n", tail.trim()));
            i += 1;
        } else if let Some(tail) = trimmed.strip_prefix("link ") {
            out.push_str(&format!("<link {}>\n", tail.trim()));
            i += 1;
        } else if let Some(tail) = trimmed.strip_prefix("script ") {
            out.push_str(&format!("<script {}></script>\n", tail.trim()));
            i += 1;
        } else if trimmed == "style" {
            let mut css = String::new();
            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                if sub.trim().is_empty() {
                    css.push('\n');
                    i += 1;
                    continue;
                }
                let sub_indent = sub.len() - sub.trim_start().len();
                if sub_indent > indent {
                    let dedented = sub
                        .get((indent + 2).min(sub_indent)..)
                        .unwrap_or_else(|| sub.trim_start());
                    css.push_str(dedented);
                    css.push('\n');
                    i += 1;
                } else {
                    break;
                }
            }
            out.push_str(&format!("<style>\n{}</style>\n", css.trim()));
        } else if trimmed.starts_with("google-fonts:") {
            i += 1;
        } else {
            // unknown — treat title-like: if it has a quoted string child
            i += 1;
        }
    }
    out.trim().to_string()
}

fn copy_unocss(vendor_dir: &Path) {
    let dst = vendor_dir.join("unocss.js");
    std::fs::write(&dst, crepuscularity_web::UNOCSS_JS).unwrap_or_else(|e| {
        ui::error(&format!("write {}: {e}", dst.display()));
    });
}

/// Build site `runtime/` to WASM once and populate `.crepus-dev/` with the same assets as a `crepus web build` dist folder.
pub(crate) fn ensure_web_dev_artifacts(site_dir: &Path, entry: &str) -> Result<(), CrepusCliError> {
    let options = BuildOptions::debug();
    let runtime_dir = site_dir.join("runtime");
    if !runtime_dir.join("Cargo.toml").is_file() {
        return Err(CrepusCliError::context(format!(
            "no runtime/Cargo.toml under {} — run `crepus web new <name>` or copy examples/web-site",
            site_dir.display()
        )));
    }

    let dev = site_dir.join(super::WEB_DEV_ARTIFACT_DIR);
    let vendor_dir = dev.join("vendor");
    let pkg_dir = dev.join("pkg");
    std::fs::create_dir_all(&vendor_dir).map_err(|e| CrepusCliError::io(e, vendor_dir.clone()))?;
    std::fs::create_dir_all(&pkg_dir).map_err(|e| CrepusCliError::io(e, pkg_dir.clone()))?;

    copy_unocss(&vendor_dir);
    for (name, source) in super::WEB_RUNTIME_ASSETS {
        std::fs::write(dev.join(name), source)
            .map_err(|e| CrepusCliError::io(e, dev.join(name)))?;
    }

    let mut files: HashMap<String, String> = HashMap::new();
    load_all_crepus(site_dir, site_dir, &mut files);
    crate::web_islands::build_web_islands(site_dir, &dev, &files, entry)
        .map_err(CrepusCliError::context)?;

    cargo_build_wasm32(&runtime_dir, options).map_err(CrepusCliError::context)?;
    let target = wasm_profile_dir(&runtime_dir, options).map_err(CrepusCliError::context)?;
    let wasm_path = find_wasm_file(&target, &runtime_dir).ok_or_else(|| {
        CrepusCliError::context(format!(
            "runtime library .wasm not found in {} (check runtime Cargo.toml [lib] crate-type)",
            target.display()
        ))
    })?;
    run_wasm_bindgen(&wasm_path, &pkg_dir, "runtime").map_err(CrepusCliError::context)?;
    Ok(())
}

fn validate_static_output(site: &Path, output: &Path) -> Result<(), String> {
    let source = super::module_emit::absolute_resolved(&site.join("static"))?;
    let output = super::module_emit::absolute_resolved(output)?;
    if output.starts_with(&source) {
        return Err(format!(
            "web output must not be inside static source {}",
            source.display()
        ));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    use walkdir::WalkDir;
    let source = src.canonicalize()?;
    let destination = super::module_emit::absolute_resolved(dst).map_err(std::io::Error::other)?;
    if source == destination {
        return Ok(());
    }
    if source.starts_with(&destination) || destination.starts_with(&source) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "static copy trees must not overlap",
        ));
    }
    let entries = WalkDir::new(src)
        .follow_links(true)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(std::io::Error::other)?;
    for entry in &entries {
        if entry.path().canonicalize()?.starts_with(&destination) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "static source must not resolve inside the destination tree",
            ));
        }
    }
    std::fs::create_dir_all(dst)?;
    let root = dst.canonicalize()?;
    let mut directories = Vec::new();
    let mut files_to_copy = Vec::new();
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(src).map_err(std::io::Error::other)?;
        crate::scaffold::validate_copy_destination(&root, relative)?;
        let output = dst.join(relative);
        if entry.file_type().is_dir() {
            directories.push(output);
        } else {
            files_to_copy.push((path.to_path_buf(), output));
        }
    }
    // Validate the complete plan before creating descendants or overwriting files.
    for directory in directories {
        std::fs::create_dir_all(directory)?;
    }
    files_to_copy
        .into_par_iter()
        .try_for_each(|(source, destination)| -> std::io::Result<()> {
            std::fs::copy(source, destination)?;
            Ok(())
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(unix)]
    fn static_copy_rejects_redirected_destinations_before_any_file_write() {
        use std::os::unix::fs::symlink;
        for parent_link in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            let outside = root.path().join("outside");
            std::fs::create_dir_all(source.join("images")).unwrap();
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(source.join("safe.txt"), "new safe").unwrap();
            std::fs::write(destination.join("safe.txt"), "keep safe").unwrap();
            std::fs::write(source.join("images/logo.txt"), "new logo").unwrap();
            std::fs::write(outside.join("logo.txt"), "keep outside").unwrap();
            if parent_link {
                symlink(&outside, destination.join("images")).unwrap();
            } else {
                std::fs::create_dir(destination.join("images")).unwrap();
                symlink(
                    outside.join("logo.txt"),
                    destination.join("images/logo.txt"),
                )
                .unwrap();
            }
            assert!(super::copy_dir_recursive(&source, &destination).is_err());
            assert_eq!(
                std::fs::read_to_string(outside.join("logo.txt")).unwrap(),
                "keep outside"
            );
            assert_eq!(
                std::fs::read_to_string(destination.join("safe.txt")).unwrap(),
                "keep safe"
            );
        }
    }

    #[test]
    fn static_output_preflight_keeps_normal_and_in_place_outputs() {
        let site = tempfile::tempdir().unwrap();
        assert!(super::validate_static_output(site.path(), &site.path().join("dist")).is_ok());
        assert!(super::validate_static_output(site.path(), site.path()).is_ok());
        assert!(
            super::validate_static_output(site.path(), &site.path().join("static/new")).is_err()
        );
        assert!(!site.path().join("static").exists());
        assert!(!site.path().join("dist").exists());
    }

    #[test]
    fn static_copy_rejects_overlapping_trees_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("asset.txt"), "source").unwrap();
        std::fs::write(root.path().join("asset.txt"), "keep destination").unwrap();
        assert!(super::copy_dir_recursive(&source, &source.join("nested")).is_err());
        assert!(!source.join("nested").exists());
        assert!(super::copy_dir_recursive(&source, root.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("asset.txt")).unwrap(),
            "keep destination"
        );
    }

    #[test]
    fn static_copy_onto_itself_preserves_assets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("asset.txt"), "keep source").unwrap();
        super::copy_dir_recursive(root.path(), root.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("asset.txt")).unwrap(),
            "keep source"
        );
    }

    #[test]
    #[cfg(unix)]
    fn static_copy_rejects_source_aliases_into_output_before_writes() {
        for directory in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let output = root.path().join("output");
            std::fs::create_dir(&source).unwrap();
            std::fs::create_dir(&output).unwrap();
            std::fs::write(source.join("safe.txt"), "new").unwrap();
            std::fs::write(output.join("safe.txt"), "keep safe").unwrap();
            std::fs::write(output.join("asset.txt"), "keep asset").unwrap();
            if directory {
                std::os::unix::fs::symlink(&output, source.join("nested")).unwrap();
            } else {
                std::os::unix::fs::symlink(output.join("asset.txt"), source.join("asset.txt"))
                    .unwrap();
            }
            assert!(super::copy_dir_recursive(&source, &output).is_err());
            assert_eq!(
                std::fs::read_to_string(output.join("asset.txt")).unwrap(),
                "keep asset"
            );
            assert_eq!(
                std::fs::read_to_string(output.join("safe.txt")).unwrap(),
                "keep safe"
            );
            assert!(!output.join("nested").exists());
        }
    }

    #[test]
    #[cfg(unix)]
    fn static_copy_preserves_external_source_aliases() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let external = root.path().join("external");
        let output = root.path().join("output");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&external).unwrap();
        std::fs::write(external.join("asset.txt"), "external asset").unwrap();
        std::os::unix::fs::symlink(&external, source.join("linked")).unwrap();
        super::copy_dir_recursive(&source, &output).unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join("linked/asset.txt")).unwrap(),
            "external asset"
        );
        assert_eq!(
            std::fs::read_to_string(external.join("asset.txt")).unwrap(),
            "external asset"
        );
    }

    #[test]
    fn static_copy_preserves_normal_nested_files() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        std::fs::create_dir_all(source.join("nested/deep")).unwrap();
        std::fs::write(source.join("nested/deep/asset.txt"), "asset").unwrap();
        super::copy_dir_recursive(&source, &destination).unwrap();
        assert_eq!(
            std::fs::read_to_string(destination.join("nested/deep/asset.txt")).unwrap(),
            "asset"
        );
    }

    #[test]
    fn html_bundle_collects_included_template_frontends() {
        for (filename, source) in [
            ("child.vue", "<template><p>VISIBLE</p></template>"),
            ("child.svelte", "<p>VISIBLE</p>"),
            ("child.jsx", "<p>VISIBLE</p>"),
            ("child.astro", "<p>VISIBLE</p>"),
            ("child.ng", "<p>VISIBLE</p>"),
        ] {
            let site = tempfile::tempdir().unwrap();
            std::fs::write(
                site.path().join("index.crepus"),
                format!("include {filename}"),
            )
            .unwrap();
            std::fs::write(site.path().join(filename), source).unwrap();
            let mut files = std::collections::HashMap::new();
            super::load_all_crepus(site.path(), site.path(), &mut files);
            let bundle = serde_json::json!({"entry": "index.crepus", "files": files}).to_string();
            let html = crepuscularity_web::render_bundle(&bundle)
                .unwrap_or_else(|error| panic!("{filename}: {error}"));
            assert!(html.contains("VISIBLE"), "{filename}: {html}");
        }
    }

    #[test]
    fn html_bundle_collects_frontends_in_named_components_and_forwarded_slots() {
        let site = tempfile::tempdir().unwrap();
        for (name, source) in [
            (
                "index.crepus",
                "include cards.crepus#Card\n  include child.vue",
            ),
            ("cards.crepus", "--- Card\ninclude wrapper.crepus\n  slot"),
            ("wrapper.crepus", "div\n  slot"),
            ("child.vue", "<template><p>SLOT VISIBLE</p></template>"),
        ] {
            std::fs::write(site.path().join(name), source).unwrap();
        }
        let mut files = std::collections::HashMap::new();
        super::load_all_crepus(site.path(), site.path(), &mut files);
        let bundle = serde_json::json!({"entry": "index.crepus", "files": files}).to_string();
        assert!(crepuscularity_web::render_bundle(&bundle)
            .unwrap()
            .contains("SLOT VISIBLE"));
    }

    #[test]
    fn framework_collection_preserves_sources_and_skips_output_and_dependencies() {
        let site = tempfile::tempdir().unwrap();
        for name in [
            "App.vue",
            "App.SVELTE",
            "App.tsx",
            "App.jsx",
            "App.csx",
            "App.astro",
            "App.ng",
            "App.ng.html",
            "App.component.html",
        ] {
            std::fs::write(site.path().join(name), "<span>Literal ｛braces｝</span>").unwrap();
        }
        std::fs::write(site.path().join("index.crepus"), "text value=｛name｝").unwrap();
        std::fs::write(site.path().join("index.html"), "not a template entry").unwrap();
        for name in ["dist", "target", "node_modules", ".git", ".crepus-dev"] {
            let path = site.path().join(name);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("Ignored.vue"), "ignored").unwrap();
        }
        let mut files = std::collections::HashMap::new();
        super::load_framework_templates(site.path(), site.path(), &mut files);
        assert_eq!(files.len(), 11, "{files:?}");
        assert_eq!(files["App.vue"], "<span>Literal ｛braces｝</span>");
        assert_eq!(files["App.SVELTE"], "<span>Literal ｛braces｝</span>");
        assert_eq!(files["index.crepus"], "text value={name}");

        files.clear();
        super::load_all_crepus(site.path(), site.path(), &mut files);
        assert_eq!(files.len(), 11);
        assert_eq!(files["App.vue"], "<span>Literal ｛braces｝</span>");
        assert_eq!(files["index.crepus"], "text value={name}");
    }

    #[test]
    #[cfg(unix)]
    fn template_collection_does_not_revisit_ancestor_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("index.crepus"), "span \"Root\"").unwrap();
        std::os::unix::fs::symlink(directory.path(), directory.path().join("cycle")).unwrap();
        let mut files = std::collections::HashMap::new();
        super::load_all_crepus(directory.path(), directory.path(), &mut files);
        assert_eq!(files.len(), 1, "{:?}", files.keys().collect::<Vec<_>>());
        assert_eq!(files["index.crepus"], "span \"Root\"");
    }

    #[test]
    #[cfg(unix)]
    fn independent_directory_aliases_keep_their_template_paths() {
        let directory = tempfile::tempdir().unwrap();
        let site = directory.path().join("site");
        let shared = directory.path().join("shared");
        std::fs::create_dir(&site).unwrap();
        std::fs::create_dir(&shared).unwrap();
        std::fs::write(shared.join("card.crepus"), "span \"Shared\"").unwrap();
        for alias in ["first", "second"] {
            std::os::unix::fs::symlink(&shared, site.join(alias)).unwrap();
        }
        let mut files = std::collections::HashMap::new();
        super::load_all_crepus(&site, &site, &mut files);
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("first/card.crepus"));
        assert!(files.contains_key("second/card.crepus"));
    }

    use super::*;

    fn seo_head() -> SiteHead {
        let mut head = SiteHead {
            page_title: "Fallback title".into(),
            description: "Fallback description".into(),
            ..SiteHead::default()
        };
        head.seo = crate::crepus_toml::SeoConfig {
            title: Some("SEO title".into()),
            description: Some("SEO description".into()),
            canonical: Some("https://example.com/".into()),
            image: Some("https://example.com/og.png".into()),
            image_alt: Some("Preview".into()),
            site_name: Some("Example".into()),
            locale: Some("en_US".into()),
            og_type: Some("website".into()),
            twitter_card: Some("summary_large_image".into()),
            twitter_site: Some("@example".into()),
            twitter_creator: Some("@author".into()),
            keywords: vec!["rust".into(), "ui".into()],
            author: Some("AJ".into()),
            robots: Some("index,follow".into()),
            theme_color: Some("#101820".into()),
            application_name: Some("Example App".into()),
            generator: Some("crepuscularity".into()),
            alternates: vec![crate::crepus_toml::SeoAlternate {
                href: "https://example.com/fr/".into(),
                hreflang: Some("fr".into()),
                media: None,
                title: None,
                mime_type: None,
            }],
            json_ld: vec![r#"{"@context":"https://schema.org","@type":"WebSite"}"#.into()],
            robots_txt: None,
            sitemap: None,
            favicon: None,
            apple_touch_icon: None,
        };
        head
    }

    #[test]
    fn readable_text_keeps_separate_anchors_with_the_same_destination() {
        let source = r#"a href="https://example.com" "First"
a href="https://example.com" "Second"
"#;
        assert_eq!(
            super::render_crepus_readable_text(source),
            "[First](https://example.com)\n\n[Second](https://example.com)"
        );
    }

    #[test]
    fn readable_link_fragments_append_after_delimiters_inside_the_label() {
        let source = r#"a href="https://example.com"
  span "A](B"
  span " tail"
"#;
        assert_eq!(
            super::render_crepus_readable_text(source),
            "[A\\](B tail](https://example.com)"
        );
    }

    #[test]
    fn readable_text_includes_inline_headings_links_and_expression_placeholders() {
        let source = r#"h1 "Title"
a href="https://example.com" "Inline link"
a href="https://example.org"
  span "Nested"
  span " link"
p title="Not body text" "Hello {name}"
"#;
        assert_eq!(super::render_crepus_readable_text(source),
            "# Title\n\n[Inline link](https://example.com)\n\n[Nested link](https://example.org)\n\nHello {name}");
    }

    #[test]
    fn readable_text_visits_control_bodies_and_preserves_escaped_literals() {
        let source = r#"if enabled
  span "Yes"
else
  span "No"
for item in items
  span "Item {item}"
include card.crepus
  span "Slot"
p "Say \"hi\""
"#;
        assert_eq!(
            super::render_crepus_readable_text(source),
            "Yes\n\nNo\n\nItem {item}\n\nSlot\n\nSay \"hi\""
        );
    }

    #[test]
    fn named_wasm_entry_keeps_head_bundle_and_selected_readable_text() {
        let site = tempfile::tempdir().unwrap();
        std::fs::create_dir(site.path().join("pages")).unwrap();
        std::fs::write(site.path().join("pages/cards.crepus"),
            "head\n  title \"Component page\"\n--- Card\nh1\n  \"Selected card\"\ninclude child.crepus\n--- Other\nspan\n  \"Sibling content\"").unwrap();
        std::fs::write(
            site.path().join("pages/child.crepus"),
            "span \"Relative child\"",
        )
        .unwrap();
        let build = super::WasmBuildArgs {
            site_dir: site.path().to_path_buf(),
            out_dir: site.path().join("dist"),
            entry: "pages/cards.crepus#Card".into(),
            meta: None,
            options: crate::build_options::BuildOptions::debug(),
        };
        let mut files = super::load_and_validate_files(&build);
        let head = super::extract_and_render_head(&build.entry, &mut files);
        assert_eq!(head, "<title>Component page</title>");
        let text = super::entry_readable_text(&build.entry, &files).unwrap();
        assert!(text.contains("Selected card"), "{text}");
        assert!(!text.contains("Sibling content"), "{text}");
        let bundle = super::write_bundle(&build, &files);
        let parsed: serde_json::Value = serde_json::from_str(&bundle).unwrap();
        assert_eq!(parsed["entry"], build.entry);
        assert!(parsed["files"]["pages/cards.crepus"]
            .as_str()
            .unwrap()
            .contains("--- Other"));
        let rendered = crepuscularity_web::render_bundle(&bundle).unwrap();
        assert!(rendered.contains("Selected card"), "{rendered}");
        assert!(rendered.contains("Relative child"), "{rendered}");
        assert!(!rendered.contains("Sibling content"), "{rendered}");
        super::generate_index_and_assets(&build, &files, &bundle, &SiteHead::default(), &head);
        for (name, expected) in crate::web::WEB_RUNTIME_ASSETS {
            assert_eq!(
                std::fs::read_to_string(build.out_dir.join(name)).unwrap(),
                expected,
                "{name}"
            );
        }
        let html = std::fs::read_to_string(build.out_dir.join("index.html")).unwrap();
        assert!(html.contains("<title>Component page</title>"));
        assert!(html.contains("<div id=\"crepus-root\"><h1"), "{html}");
    }

    #[test]
    fn head_css_preserves_content_with_short_indentation() {
        for indent in [" ", "\t", "\u{2003}", "  "] {
            let source = format!("style\n{indent}éclair {{ color: red; }}\ntitle \"After\"");
            assert_eq!(
                super::render_head_raw(&source),
                "<style>\néclair { color: red; }</style>\n<title>After</title>"
            );
        }
    }

    #[test]
    fn render_index_html_emits_full_seo_head() {
        let html = render_index_html(&seo_head(), &[], "", "");
        assert!(html.contains(r#"<title>SEO title</title>"#));
        assert!(html.contains(r#"<meta name="description" content="SEO description">"#));
        assert!(html.contains(r#"<link rel="canonical" href="https://example.com/">"#));
        assert!(html.contains(r#"<meta name="keywords" content="rust, ui">"#));
        assert!(html.contains(r#"<meta name="author" content="AJ">"#));
        assert!(html.contains(r#"<meta name="robots" content="index,follow">"#));
        assert!(html.contains(r##"<meta name="theme-color" content="#101820">"##));
        assert!(html.contains(r#"<meta property="og:title" content="SEO title">"#));
        assert!(html.contains(r#"<meta property="og:image" content="https://example.com/og.png">"#));
        assert!(html.contains(r#"<meta name="twitter:creator" content="@author">"#));
        assert!(
            html.contains(r#"<link rel="alternate" href="https://example.com/fr/" hreflang="fr">"#)
        );
        assert!(html.contains(r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite"}</script>"#));
    }

    #[test]
    fn render_seo_auxiliary_files_from_config() {
        let mut head = seo_head();
        head.seo.robots_txt = Some(crate::crepus_toml::RobotsTxtConfig {
            enabled: true,
            user_agent: None,
            allow: vec!["/".into()],
            disallow: vec!["/private".into()],
            sitemap: None,
        });
        head.seo.sitemap = Some(crate::crepus_toml::SitemapConfig {
            enabled: true,
            base_url: Some("https://example.com".into()),
            paths: vec!["/".into(), "/docs/".into()],
            changefreq: Some("weekly".into()),
            priority: Some(0.8),
        });
        let tmp = tempfile::tempdir().expect("tempdir");
        write_seo_files(tmp.path(), &head);
        let robots = std::fs::read_to_string(tmp.path().join("robots.txt")).expect("robots");
        assert!(robots.contains("User-agent: *"));
        assert!(robots.contains("Allow: /"));
        assert!(robots.contains("Disallow: /private"));
        assert!(robots.contains("Sitemap: https://example.com/sitemap.xml"));
        let sitemap = std::fs::read_to_string(tmp.path().join("sitemap.xml")).expect("sitemap");
        assert!(sitemap.contains("<loc>https://example.com/</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/docs/</loc>"));
        assert!(sitemap.contains("<changefreq>weekly</changefreq>"));
        assert!(sitemap.contains("<priority>0.8</priority>"));
    }

    #[test]
    fn llms_mode_emits_alternates_and_markdown_files() {
        let site = tempfile::tempdir().expect("site");
        let out = tempfile::tempdir().expect("out");
        std::fs::write(site.path().join("README.md"), "# Example\n\nReadable docs.")
            .expect("readme");
        let meta = crate::crepus_toml::WebTargetMeta {
            llms: Some(crate::crepus_toml::LlmsConfig {
                enabled: true,
                base_url: Some("https://example.com".into()),
                title: Some("Example".into()),
                description: Some("Agent-readable example.".into()),
                sources: vec![crate::crepus_toml::LlmsSource {
                    path: "README.md".into(),
                    href: None,
                    title: Some("README".into()),
                }],
            }),
            ..crate::crepus_toml::WebTargetMeta::default()
        };
        let head = merge_llms_alternates(seo_head(), Some(&meta));
        let html = render_index_html(&head, &[], "", "");
        assert!(html.contains(r#"<link rel="alternate" href="https://example.com/llms-full.txt" title="Full LLM bundle" type="text/plain">"#));
        assert!(html.contains(r#"<noscript><p><a href="https://example.com/llms-full.txt">Agent-readable text version</a></p></noscript>"#));
        let page_text = render_crepus_readable_text(
            r#"
h1
  "Main title"
section
  h2
    "Projects"
  a href="https://example.com/project"
    span
      "Project"
    span
      " — readable link text"
"#,
        );
        write_llms_files(
            site.path(),
            out.path(),
            &head,
            Some(&meta),
            Some(&page_text),
        );
        let llms = std::fs::read_to_string(out.path().join("llms.txt")).expect("llms");
        let full = std::fs::read_to_string(out.path().join("llms-full.txt")).expect("full");
        assert!(llms.contains("https://example.com/README.md"));
        assert!(full.contains("# Main title"));
        assert!(full.contains("## Projects"));
        assert!(full.contains("[Project — readable link text](https://example.com/project)"));
        assert!(full.contains("# README"));
        assert!(full.contains("Readable docs."));
        assert!(out.path().join("README.md").is_file());
    }

    #[test]
    fn sitemap_includes_discovered_html_when_enabled() {
        let mut head = seo_head();
        head.seo.sitemap = Some(crate::crepus_toml::SitemapConfig {
            enabled: true,
            base_url: Some("https://example.com".into()),
            paths: vec!["/".into()],
            changefreq: None,
            priority: None,
        });
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("docs")).expect("docs dir");
        std::fs::write(tmp.path().join("docs/cli.html"), "").expect("cli");
        std::fs::write(tmp.path().join("docs/runtime.html"), "").expect("runtime");
        write_seo_files(tmp.path(), &head);
        let sitemap = std::fs::read_to_string(tmp.path().join("sitemap.xml")).expect("sitemap");
        assert!(sitemap.contains("<loc>https://example.com/</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/docs/cli.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/docs/runtime.html</loc>"));
    }
}

#[cfg(test)]
mod bundle_output_tests {
    use super::*;

    fn args(site: &Path, output: &str) -> WasmBuildArgs {
        WasmBuildArgs {
            site_dir: site.to_path_buf(),
            out_dir: site.join(output),
            entry: "index.crepus".into(),
            meta: None,
            options: BuildOptions::debug(),
        }
    }

    fn files() -> HashMap<String, String> {
        HashMap::from([("index.crepus".into(), "text \"Current source\"".into())])
    }

    #[test]
    fn changed_destination_is_repaired_after_an_identical_previous_build() {
        let site = tempfile::tempdir().unwrap();
        let build = args(site.path(), "dist");
        let desired = write_bundle(&build, &files());
        let destination = build.out_dir.join("crepus-bundle.json");
        std::fs::write(&destination, "stale or damaged artifact").unwrap();
        assert_eq!(write_bundle(&build, &files()), desired);
        assert_eq!(std::fs::read_to_string(destination).unwrap(), desired);
    }

    #[test]
    fn alternate_output_is_checked_independently_of_a_previous_destination() {
        let site = tempfile::tempdir().unwrap();
        let first = args(site.path(), "first");
        let second = args(site.path(), "second");
        let desired = write_bundle(&first, &files());
        let stale = HashMap::from([("index.crepus".into(), "text \"Old source\"".into())]);
        write_bundle(&second, &stale);
        assert_eq!(write_bundle(&second, &files()), desired);
        assert_eq!(
            std::fs::read_to_string(second.out_dir.join("crepus-bundle.json")).unwrap(),
            desired
        );
        assert_eq!(
            std::fs::read_to_string(first.out_dir.join("crepus-bundle.json")).unwrap(),
            desired
        );
    }

    #[test]
    fn identical_destination_keeps_its_existing_modified_time() {
        let site = tempfile::tempdir().unwrap();
        let build = args(site.path(), "dist");
        let desired = write_bundle(&build, &files());
        let destination = build.out_dir.join("crepus-bundle.json");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&destination)
            .unwrap();
        file.set_times(
            std::fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_234_567_890),
            ),
        )
        .unwrap();
        let before = std::fs::metadata(&destination).unwrap().modified().unwrap();
        drop(file);
        assert_eq!(write_bundle(&build, &files()), desired);
        assert_eq!(
            std::fs::metadata(destination).unwrap().modified().unwrap(),
            before
        );
    }
}

#[cfg(test)]
mod build_path_tests {
    use super::*;

    #[test]
    fn resolved_manifest_build_metadata_does_not_trigger_a_second_target_selection() {
        let site = tempfile::tempdir().unwrap();
        std::fs::write(site.path().join("crepus.toml"),
            "[[targets]]\ntype='web'\nid='first'\nsite='.'\n[[targets]]\ntype='web'\nid='second'\nsite='.'\n").unwrap();
        let resolved = resolve_wasm_build_args(&WebBuildArgs {
            site_dir: Some(site.path().to_path_buf()),
            out_dir: Some(site.path().join("chosen-output")),
            entry: Some("chosen.crepus".into()),
            target_id: None,
            manifest: None,
            meta: Some(crate::crepus_toml::WebTargetMeta::default()),
            options: BuildOptions::debug(),
        });
        assert_eq!(resolved.site_dir, site.path());
        assert_eq!(resolved.out_dir, site.path().join("chosen-output"));
        assert_eq!(resolved.entry, "chosen.crepus");
    }
}
