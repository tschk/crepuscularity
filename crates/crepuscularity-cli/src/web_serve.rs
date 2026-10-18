//! `crepus web dev` — hot-reload dev server for supported template sources.
//!
//! # How it works
//!
//! 1. Walks `site_dir` and loads all supported template sources into an in-memory
//!    **virtual file map** (`Arc<RwLock<HashMap<String, String>>>`).
//! 2. A `notify` watcher observes templates, authored island dependencies, and runtime sources; a debounce thread
//!    batches events within a 50 ms window then updates the virtual file map
//!    and bumps a generation counter. Non-`.crepus` template changes request a
//!    full page reload, preserving authored island module reload behavior.
//! 3. A plain `std::net::TcpListener` accept-loop spawns one thread per
//!    connection — fine for a dev server with ≤ a handful of browser tabs.
//! 4. `GET /dev-reload` is a **Server-Sent Events** endpoint: the browser
//!    keeps the connection open; when the generation counter changes the
//!    server sends typed reload/error events and keeps the connection open.
//! 5. On startup the server runs the same **wasm32 + wasm-bindgen** step as `crepus web build`,
//!    writing UnoCSS, `app.js`, and `pkg/` under **`.crepus-dev/`** beside the site. Saving
//!    **`runtime/**/*.rs`** (or `Cargo.toml` / `Cargo.lock` in that crate) repeats that build and
//!    triggers a full page reload — same idea as `cargo-leptos` rebuilding the client WASM, except
//!    the Rust compiler still recompiles the world (there is no incremental “hot swap” of native code).
//! 6. **`GET /crepus-bundle.json`** returns the live virtual `.crepus` map (for the browser WASM renderer).
//! 7. **`GET /**` for the configured entry serves the real HTML shell** (`vendor/unocss.js`, `app.js`, `#crepus-root`)
//!    plus a dev-reload script. Subordinate `.html` routes get a static preview shell (no WASM) with UnoCSS.
//! 8. Static assets **`/static/*`** continue to resolve under the site directory.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecursiveMode, Watcher};

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use serde_json::json;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::{escape_html, render_from_files, render_from_files_with_ssr};

use crate::crepus_toml::WebTargetMeta;
use crate::web::{
    ensure_web_dev_artifacts, load_site_head, merge_site_head_meta, merged_site_google_fonts,
    merged_site_inline_css, render_index_html,
};
use crate::web_docs_hook::{docs_src_path, run_docs_hook, DocsHookTheme};

/// Watcher event: template, authored island dependencies, or site WASM crate.
enum WatchEvent {
    Crepus(PathBuf),
    RuntimeDirty,
    IslandDirty(PathBuf),
}

fn is_runtime_source_path(site_dir: &Path, path: &Path) -> bool {
    let runtime_root = site_dir.join("runtime");
    let root = std::fs::canonicalize(&runtime_root).unwrap_or_else(|_| runtime_root.clone());
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !resolved.starts_with(&root) {
        return false;
    }
    if resolved.components().any(|c| c.as_os_str() == "target") {
        return false;
    }
    let ext = path.extension().and_then(|e| e.to_str());
    matches!(ext, Some("rs" | "toml"))
        || path.file_name().and_then(|n| n.to_str()) == Some("Cargo.lock")
}

// Use lexical paths as well as resolved paths: remove/rename notifications may
// refer to a file that no longer exists, while symlinks must stay inside the site.
fn is_authored_site_path(site_dir: &Path, path: &Path) -> bool {
    fn allowed(root: &Path, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        !relative.as_os_str().is_empty()
            && relative.components().all(|part| {
                let std::path::Component::Normal(name) = part else {
                    return false;
                };
                let name = name.to_string_lossy();
                !matches!(
                    name.as_ref(),
                    ".crepus-dev" | "dist" | "target" | "node_modules" | ".git" | "runtime"
                ) && !name.starts_with(".crepus-react-island-")
            })
    }
    allowed(site_dir, path)
        && path
            .canonicalize()
            .map_or(true, |resolved| allowed(site_dir, &resolved))
}

fn island_rebuild_sse_message(path: &Path) -> String {
    format!(
        "event: crepus-reload\ndata: {}\n\n",
        json!({
            "file": path.file_name().unwrap_or_default().to_string_lossy(),
            "reload": true
        })
    )
}

fn runtime_rebuild_sse_message() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!(
        "event: crepus-reload\ndata: {{\"file\":\"runtime (WASM)\",\"ts\":{}}}\n\n",
        ts
    )
}

fn runtime_build_error_sse(message: &str) -> String {
    format!("event: crepus-error\ndata: {}\n\n", json!(message))
}

// ── Options ──────────────────────────────────────────────────────────────────

/// Configuration for `crepus web dev`.
pub struct ServeOptions {
    /// Root directory containing `.crepus` source files.
    pub site_dir: PathBuf,
    /// TCP port to listen on (default 4000).
    pub port: u16,
    /// Entry-point template relative to `site_dir` (default `"index.crepus"`).
    pub entry: String,
    pub meta: Option<WebTargetMeta>,
}

#[derive(Clone)]
struct DevRequestContext {
    vfm: Arc<RwLock<HashMap<String, String>>>,
    generation: Arc<AtomicU64>,
    last_sse_msg: Arc<RwLock<String>>,
    entry: String,
    site_dir: PathBuf,
    dev_root: PathBuf,
    meta: Option<WebTargetMeta>,
}

// ── Hot-reload script ────────────────────────────────────────────────────────

const RELOAD_SCRIPT: &str = "<script>
(function(){
  function showErrorOverlay(msg){
    var el=document.getElementById('__crepus_err__');
    if(!el){
      el=document.createElement('div');
      el.id='__crepus_err__';
      el.style.cssText='position:fixed;inset:0;z-index:99999;background:rgba(10,0,0,0.93);color:#ff6e6e;font-family:monospace;font-size:14px;padding:40px;white-space:pre-wrap;overflow-y:auto;display:flex;flex-direction:column;gap:12px';
      document.body.appendChild(el);
    }
    while(el.firstChild){el.removeChild(el.firstChild);}
    var h=document.createElement('strong');
    h.style.cssText='font-size:20px;color:#ff9999';
    h.textContent='\u{26a0} Crepus error';
    var b=document.createElement('pre');
    b.style.cssText='margin:0;white-space:pre-wrap;color:#ff6e6e';
    b.textContent=msg;
    var hint=document.createElement('span');
    hint.style.cssText='color:#555;font-size:12px';
    hint.textContent='Fix the template and save to dismiss.';
    el.appendChild(h);el.appendChild(b);el.appendChild(hint);
  }
  function dismissErrorOverlay(){var el=document.getElementById('__crepus_err__');if(el)el.remove();}
  function showToast(text){
    var t=document.createElement('div');
    t.style.cssText='position:fixed;bottom:16px;right:16px;z-index:99998;background:#18181b;color:#a1a1aa;font-family:monospace;font-size:12px;padding:8px 14px;border-radius:6px;border:1px solid #3f3f46;pointer-events:none;transition:opacity 0.3s';
    t.textContent=text;
    document.body.appendChild(t);
    setTimeout(function(){t.style.opacity='0';setTimeout(function(){t.remove();},300);},1800);
  }
  var es=new EventSource('/dev-reload');
  es.addEventListener('crepus-reload',function(e){
    dismissErrorOverlay();
    var info={};try{info=JSON.parse(e.data);}catch(_){}
    if (info.reload === true || (info.file && info.file.includes('runtime'))) {
      showToast('\u{21bb} '+(info.file||'template')+' updated');
      setTimeout(function(){location.reload();},150);
    } else {
      fetch('/crepus-bundle.json', {cache: 'no-store'}).then(r => r.json()).then(function(bundle) {
        var handled = !window.dispatchEvent(new CustomEvent('crepus-update-bundle', {detail: bundle, cancelable: true}));
        if (!handled) { location.reload(); return; }
        showToast('\u{21bb} '+(info.file||'template')+' updated');
      }).catch(function(){
        location.reload();
      });
    }
  });
  es.addEventListener('crepus-error',function(e){
    showErrorOverlay(JSON.parse(e.data));
  });
  es.onmessage=function(e){if(e.data==='reload'){dismissErrorOverlay();location.reload();}};
  es.onerror=function(){};
})();
</script>";

// ── Startup validation ───────────────────────────────────────────────────────

/// Recursively collect `.crepus` file paths under `dir`.
fn walk_crepus_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut paths = Vec::new();
    collect_crepus_files(dir, &mut paths, false, &mut HashSet::new());
    out.extend(paths.into_iter().filter(|path| {
        path.extension()
            .is_some_and(|extension| extension == "crepus")
    }));
}

/// Parse every `.crepus` file and return pass/fail lines sorted by relative path.
///
/// Reading and parsing remain parallel, but collecting from Rayon preserves the
/// indexed input order so startup output is repeatable.
fn template_validation_messages(site_dir: &Path) -> Vec<String> {
    use crepuscularity_core::parser::parse_template_with_path;

    let mut files = Vec::new();
    walk_crepus_files(site_dir, &mut files);
    files.sort_unstable();

    files
        .par_iter()
        .map(|path| {
            let rel = path.strip_prefix(site_dir).unwrap_or(path);
            match std::fs::read_to_string(path).map(|s| parse_template_with_path(&s, Some(path))) {
                Ok(Ok(_)) => format!("  {} {}", console::style("✓").green(), rel.display()),
                Ok(Err(e)) => format!("  {} {} — {}", console::style("✗").red(), rel.display(), e),
                Err(e) => format!(
                    "  {} {} — read error: {}",
                    console::style("✗").red(),
                    rel.display(),
                    e
                ),
            }
        })
        .collect()
}

/// Walk `site_dir` and parse every `.crepus` file, printing pass/fail per file.
fn validate_templates(site_dir: &Path) {
    let messages = template_validation_messages(site_dir);
    if messages.is_empty() {
        eprintln!("  {} no .crepus files found", console::style("⚠").yellow());
    } else {
        for message in messages {
            eprintln!("{message}");
        }
    }
}

// ── Public entry point ───────────────────────────────────────────────────────

/// Run the dev server, blocking until the process is killed.
pub fn run(mut opts: ServeOptions) {
    opts.entry = crate::web::normalize_bundle_entry(&opts.entry);
    let site_dir = std::fs::canonicalize(&opts.site_dir).unwrap_or_else(|_| opts.site_dir.clone());

    // Validate all templates at startup.
    let validate_start = Instant::now();
    eprintln!("\n  {} validating templates…", console::style("→").dim());
    validate_templates(&site_dir);
    let validate_elapsed = validate_start.elapsed();
    eprintln!(
        "  {} templates validated in {:.2}s",
        console::style("✓").green(),
        validate_elapsed.as_secs_f64()
    );

    let wasm_start = Instant::now();
    eprintln!(
        "  {} compiling dev WASM → {}",
        console::style("→").dim(),
        console::style(format!("{}/", crate::web::WEB_DEV_ARTIFACT_DIR)).cyan()
    );
    if let Err(e) = ensure_web_dev_artifacts(&site_dir, &opts.entry) {
        eprintln!("  {} {}", console::style("✗").red().bold(), e);
        std::process::exit(1);
    }
    let wasm_elapsed = wasm_start.elapsed();
    eprintln!(
        "  {} UnoCSS, app.js, and pkg/ ready in {:.2}s",
        console::style("✓").green(),
        wasm_elapsed.as_secs_f64()
    );

    let dev_root = site_dir.join(crate::web::WEB_DEV_ARTIFACT_DIR);

    let head = load_site_head(&site_dir);
    if let Some(docs) = opts.meta.as_ref().and_then(|meta| meta.docs.as_ref()) {
        let docs_theme = DocsHookTheme {
            accent: head.theme.accent.clone(),
            accent_soft: head.theme.accent_soft.clone(),
            surface: head.theme.surface.clone(),
            text: head.theme.text.clone(),
            muted: head.theme.muted.clone(),
            border: head.theme.border.clone(),
        };
        let docs_src = docs_src_path(&site_dir, docs);
        let docs_out = dev_root.join("docs");
        match run_docs_hook(&site_dir, &docs_out, docs, &head.page_title, &docs_theme) {
            Ok(()) => eprintln!(
                "  {} docs → {} (serve at /docs/)",
                console::style("✓").green(),
                docs_out.display()
            ),
            Err(e) => eprintln!(
                "  {} could not mirror docs: {}",
                console::style("⚠").yellow(),
                e
            ),
        }
        start_docs_markdown_watcher(
            site_dir.clone(),
            docs.clone(),
            docs_src,
            docs_out,
            docs_theme.clone(),
            head.page_title.clone(),
        );
    }

    // Build initial virtual file map.
    let vfm: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));
    load_all_crepus(&site_dir, &vfm);

    // Generation counter — bumped on every hot reload.
    let generation: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

    // Last SSE message — updated by the watcher, read by SSE long-poll handlers.
    let last_sse_msg: Arc<RwLock<String>> = Arc::new(RwLock::new(String::new()));

    // Spawn watcher + debounce.
    start_watcher(
        site_dir.clone(),
        Arc::clone(&vfm),
        Arc::clone(&generation),
        Arc::clone(&last_sse_msg),
        opts.entry.clone(),
    );

    // Bind TCP listener.
    let addr = format!("127.0.0.1:{}", opts.port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("crepus web dev: cannot bind {addr}: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "\n  {} crepus web dev\n  {} http://localhost:{}\n  {} edit templates, islands, or runtime/ — templates hot-reload; islands reload; Rust rebuilds WASM\n",
        console::style("▶").green().bold(),
        console::style("→").dim(),
        opts.port,
        console::style("→").dim(),
    );

    let ctx = DevRequestContext {
        vfm: Arc::clone(&vfm),
        generation: Arc::clone(&generation),
        last_sse_msg: Arc::clone(&last_sse_msg),
        entry: opts.entry.clone(),
        site_dir: site_dir.clone(),
        dev_root: dev_root.clone(),
        meta: opts.meta.clone(),
    };

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    handle_connection(s, ctx);
                });
            }
            Err(e) => {
                eprintln!("crepus web dev: accept error: {e}");
            }
        }
    }
}

// ── Virtual file map ─────────────────────────────────────────────────────────

/// Load supported template sources into the dev virtual file map.
fn load_all_crepus(site_dir: &Path, vfm: &Arc<RwLock<HashMap<String, String>>>) {
    let mut paths = vec![];
    load_dir_recursive_collect(site_dir, &mut paths);
    let results: Vec<(String, String)> = paths
        .par_iter()
        .filter_map(|path| {
            std::fs::read_to_string(path).ok().map(|content| {
                let key = relative_key(site_dir, path);
                (key, content)
            })
        })
        .collect();
    let mut map = vfm
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (key, content) in results {
        map.insert(key, content);
    }
}

fn load_dir_recursive_collect(dir: &Path, paths: &mut Vec<PathBuf>) {
    collect_crepus_files(dir, paths, true, &mut HashSet::new());
}

fn collect_crepus_files(
    dir: &Path,
    paths: &mut Vec<PathBuf>,
    skip_generated: bool,
    ancestors: &mut HashSet<PathBuf>,
) {
    let Ok(identity) = dir.canonicalize() else {
        return;
    };
    if !ancestors.insert(identity.clone()) {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if skip_generated
                    && matches!(
                        name,
                        "dist" | "target" | ".git" | "node_modules" | ".crepus-dev"
                    )
                {
                    continue;
                }
                collect_crepus_files(&path, paths, skip_generated, ancestors);
            } else if crate::web::is_framework_template(&path) {
                paths.push(path);
            }
        }
    }
    // Only ancestry cycles are skipped; separate logical aliases remain importable.
    ancestors.remove(&identity);
}

/// Return a forward-slash relative path string from `root` to `abs`.
fn relative_key(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

// ── SSE message builder ──────────────────────────────────────────────────────

/// Try to parse `path` and return the appropriate SSE event string.
fn build_sse_message(path: &Path) -> String {
    use crepuscularity_core::parser::parse_template_with_path;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    match std::fs::read_to_string(path).map(|s| parse_template_with_path(&s, Some(path))) {
        Ok(Ok(_)) => format!(
            "event: crepus-reload\ndata: {}\n\n",
            json!({ "file": filename, "ts": ts }),
        ),
        Ok(Err(e)) => runtime_build_error_sse(&e.to_string()),
        Err(e) => runtime_build_error_sse(&format!("read error: {e}")),
    }
}

// ── File watcher + debounce ──────────────────────────────────────────────────

fn classify_authored_path(path: &Path) -> WatchEvent {
    if !path.is_dir() && crate::web::is_framework_template(path) {
        WatchEvent::Crepus(path.to_path_buf())
    } else {
        WatchEvent::IslandDirty(path.to_path_buf())
    }
}

fn queue_template_update(
    path: PathBuf,
    pending: &mut Vec<PathBuf>,
    island_path: &mut Option<PathBuf>,
) {
    // Frontend files may also be authored island modules. Keep their full-reload
    // behavior while refreshing bundle bytes.
    if !path.is_file()
        || path.extension().and_then(|extension| extension.to_str()) != Some("crepus")
    {
        *island_path = Some(path.clone());
    }
    pending.push(path);
}

fn apply_template_updates(
    site_dir: &Path,
    vfm: &Arc<RwLock<HashMap<String, String>>>,
    pending_crepus: &mut Vec<PathBuf>,
    rt: &tokio::runtime::Runtime,
) -> (usize, Option<PathBuf>) {
    if pending_crepus.is_empty() {
        return (0, None);
    }
    let mut changed = 0;
    let mut last_crepus_path = None;
    let results = rt.block_on(async {
        let mut futures = Vec::new();
        for path in pending_crepus.drain(..) {
            let site_dir = site_dir.to_path_buf();
            futures.push(tokio::task::spawn_blocking(move || {
                let key = relative_key(&site_dir, &path);
                let mut sources = Vec::new();
                if path.is_dir() {
                    load_dir_recursive_collect(&path, &mut sources);
                    sources.retain(|source| is_authored_site_path(&site_dir, source));
                } else if crate::web::is_framework_template(&path) {
                    sources.push(path.clone());
                }
                let contents: Vec<_> = sources
                    .into_iter()
                    .filter_map(|source| {
                        std::fs::read_to_string(&source)
                            .ok()
                            .map(|content| (relative_key(&site_dir, &source), content))
                    })
                    .collect();
                (key, path, contents)
            }));
        }
        futures_util::future::join_all(futures).await
    });

    let mut map = vfm
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (key, path, contents) in results.into_iter().flatten() {
        // Path components distinguish `views` from its sibling `views-other`.
        // Evict descendants even when a directory no longer exists or became a file.
        map.retain(|existing, _| !Path::new(existing).starts_with(Path::new(&key)));
        map.extend(contents);
        changed += 1;
        last_crepus_path = Some(path);
    }
    drop(map);
    (changed, last_crepus_path)
}

fn start_watcher(
    site_dir: PathBuf,
    vfm: Arc<RwLock<HashMap<String, String>>>,
    generation: Arc<AtomicU64>,
    last_sse_msg: Arc<RwLock<String>>,
    entry: String,
) {
    let (tx, rx) = mpsc::channel::<WatchEvent>();

    // notify watcher thread.
    let watch_dir = site_dir.clone();
    let site_for_watcher = site_dir.clone();
    std::thread::spawn(move || {
        let site_for_filter = site_for_watcher;
        let tx2 = tx.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if !matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        return;
                    }
                    for path in &event.paths {
                        if is_runtime_source_path(&site_for_filter, path) {
                            let _ = tx2.send(WatchEvent::RuntimeDirty);
                        } else if is_authored_site_path(&site_for_filter, path) {
                            let event = classify_authored_path(path);
                            let _ = tx2.send(event);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  {} watcher error: {e}", console::style("⚠").yellow());
                }
            })
            .expect("crepus web dev: cannot create file watcher");

        watcher
            .watch(&watch_dir, RecursiveMode::Recursive)
            .expect("crepus web dev: cannot watch site dir");

        // Keep watcher alive.
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });

    // Debounce thread: batch events within 50 ms then apply.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let mut pending_crepus: Vec<PathBuf> = Vec::new();
        let mut runtime_dirty = false;
        let mut island_path = None;
        let mut last_event = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(WatchEvent::Crepus(p)) => {
                    queue_template_update(p, &mut pending_crepus, &mut island_path);
                    last_event = Instant::now();
                }
                Ok(WatchEvent::IslandDirty(path)) => {
                    queue_template_update(path, &mut pending_crepus, &mut island_path);
                    last_event = Instant::now();
                }
                Ok(WatchEvent::RuntimeDirty) => {
                    runtime_dirty = true;
                    last_event = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if pending_crepus.is_empty() && !runtime_dirty && island_path.is_none() {
                        continue;
                    }
                    if last_event.elapsed() < Duration::from_millis(50) {
                        continue;
                    }
                    let apply_start = Instant::now();
                    let (mut changed, last_crepus_path) =
                        apply_template_updates(&site_dir, &vfm, &mut pending_crepus, &rt);
                    let last_island_path = island_path.take();
                    changed += usize::from(last_island_path.is_some());
                    let island_error = if last_crepus_path.is_some() || last_island_path.is_some() {
                        let map = vfm
                            .read()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        let dev_root = site_dir.join(crate::web::WEB_DEV_ARTIFACT_DIR);
                        crate::web_islands::build_web_islands(&site_dir, &dev_root, &map, &entry)
                            .err()
                            .map(|error| runtime_build_error_sse(&error))
                    } else {
                        None
                    };

                    let mut runtime_rebuilt = false;
                    let wasm_error: Option<String> = if runtime_dirty {
                        runtime_dirty = false;
                        match ensure_web_dev_artifacts(&site_dir, &entry) {
                            Ok(()) => {
                                runtime_rebuilt = true;
                                None
                            }
                            Err(e) => Some(runtime_build_error_sse(&e.to_string())),
                        }
                    } else {
                        None
                    };

                    let wasm_failed = wasm_error.is_some();
                    let sse = if let Some(err) = wasm_error {
                        err
                    } else if let Some(err) = island_error {
                        err
                    } else if let Some(ref path) = last_island_path {
                        island_rebuild_sse_message(path)
                    } else if let Some(ref path) = last_crepus_path {
                        build_sse_message(path)
                    } else if runtime_rebuilt {
                        runtime_rebuild_sse_message()
                    } else {
                        continue;
                    };

                    let reload_failed = sse.starts_with("event: crepus-error\n");
                    let should_notify =
                        changed > 0 || last_island_path.is_some() || runtime_rebuilt || wasm_failed;
                    if should_notify {
                        if let Ok(mut msg) = last_sse_msg.write() {
                            *msg = sse;
                        }
                        generation.fetch_add(1, Ordering::Release);
                        let elapsed = apply_start.elapsed();
                        let _span = tracing::info_span!(
                            "hot_reload",
                            changed_files = changed,
                            duration_ms = elapsed.as_millis()
                        )
                        .entered();
                        tracing::info!(
                            changed_files = changed,
                            duration_ms = elapsed.as_millis(),
                            "hot reloaded"
                        );
                        eprintln!(
                            "  {} processed {} changed files in {:.2}s",
                            if reload_failed {
                                console::style("✗").red()
                            } else {
                                console::style("✓").green()
                            },
                            changed,
                            elapsed.as_secs_f64()
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn start_docs_markdown_watcher(
    site_dir: PathBuf,
    hook: crate::crepus_toml::DocsHookConfig,
    docs_src: PathBuf,
    docs_out: PathBuf,
    theme: DocsHookTheme,
    site_name: String,
) {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let docs_src_watch = docs_src.clone();

    std::thread::spawn(move || {
        let tx2 = tx.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if !matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        return;
                    }
                    for path in &event.paths {
                        if path.extension().and_then(|e| e.to_str()) == Some("md") {
                            let _ = tx2.send(path.clone());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  {} docs watcher error: {e}", console::style("⚠").yellow());
                }
            })
            .expect("crepus web dev: docs watcher");

        watcher
            .watch(&docs_src_watch, RecursiveMode::Recursive)
            .expect("crepus web dev: watch docs/");

        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });

    std::thread::spawn(move || {
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut last_event = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(p) => {
                    pending.push(p);
                    last_event = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() && last_event.elapsed() >= Duration::from_millis(200) {
                        pending.clear();
                        if let Err(e) =
                            run_docs_hook(&site_dir, &docs_out, &hook, &site_name, &theme)
                        {
                            eprintln!(
                                "  {} docs rebuild failed: {}",
                                console::style("⚠").yellow(),
                                e
                            );
                        } else {
                            tracing::info!("documentation markdown rebuilt");
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// Resolve `rel` under `base`, rejecting `..` and symlink escapes.
fn resolve_under_dir(base: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty()
        || Path::new(rel)
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return None;
    }
    let base = std::fs::canonicalize(base).ok()?;
    let resolved = std::fs::canonicalize(base.join(rel)).ok()?;
    resolved.starts_with(&base).then_some(resolved)
}

fn serve_docs_path(stream: &mut TcpStream, url_path: &str, dev_root: &Path) {
    let base = dev_root.join("docs");
    if !base.is_dir() {
        let msg = "No generated docs output yet. If this site has a [targets.docs] hook, check the hook command and rebuild or restart crepus web dev.";
        let body = format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Docs</title></head>\
             <body style=\"font-family:system-ui,sans-serif;padding:2rem;max-width:40rem;line-height:1.5\">\
             <h1>Documentation</h1><p>{}</p></body></html>",
            escape_html(msg)
        );
        let resp = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }
    let mut rel = url_path
        .strip_prefix("/docs")
        .unwrap_or(url_path)
        .trim_start_matches('/');
    if rel.is_empty() {
        rel = "index.html";
    }
    let Some(path) = resolve_static_file(&base, rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file_with_mime(stream, &path, logical_path_mime(rel));
}

// ── HTTP connection handler ──────────────────────────────────────────────────

fn handle_connection(mut stream: TcpStream, ctx: DevRequestContext) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    // Consume complete headers before routing; TCP reads may split the request anywhere.
    let mut buf = [0u8; 4096];
    let mut n = 0;
    loop {
        if n == buf.len() {
            return;
        }
        match stream.read(&mut buf[n..]) {
            Ok(0) | Err(_) => return,
            Ok(read) => n += read,
        }
        if buf[..n].windows(4).any(|bytes| bytes == b"\r\n\r\n")
            || buf[..n].windows(2).any(|bytes| bytes == b"\n\n")
        {
            break;
        }
    }
    let request = std::str::from_utf8(&buf[..n]).unwrap_or("");
    let first_line = request.lines().next().unwrap_or("");

    // Parse method + path.
    let mut parts = first_line.splitn(3, ' ');
    let method = parts.next().unwrap_or("GET");
    let raw_path = parts.next().unwrap_or("/");
    // Strip query string.
    let path = raw_path.split('?').next().unwrap_or(raw_path);

    let _span = tracing::info_span!("dev_request", %method, %path).entered();

    match (method, path) {
        ("GET", "/dev-reload") => {
            long_poll_sse(stream, ctx.generation, ctx.last_sse_msg);
        }

        ("GET", "/crepus-bundle.json") => {
            serve_crepus_bundle(&mut stream, &ctx.vfm, &ctx.entry);
        }

        ("GET", "/crepus-islands.json") => {
            serve_dev_fs_file(&mut stream, &ctx.dev_root.join("crepus-islands.json"));
        }

        ("GET", "/app.js") => {
            serve_dev_fs_file(&mut stream, &ctx.dev_root.join("app.js"));
        }

        ("GET", p @ ("/runtime-worker.js" | "/runtime-client.js")) => {
            serve_static_file(&mut stream, p, &ctx.dev_root);
        }

        ("GET", "/vendor/unocss.js") => {
            serve_dev_fs_file(&mut stream, &ctx.dev_root.join("vendor/unocss.js"));
        }

        ("GET", p) if p.starts_with("/pkg/") => {
            serve_pkg_path(&mut stream, p, &ctx.dev_root);
        }

        ("GET", p) if p.starts_with("/islands/") => {
            serve_islands_path(&mut stream, p, &ctx.dev_root);
        }

        ("GET", p) if p == "/docs" || p.starts_with("/docs/") => {
            serve_docs_path(&mut stream, p, &ctx.dev_root);
        }

        ("GET", "/" | "/index.html") => {
            serve_index_document(
                &mut stream,
                &ctx.vfm,
                &ctx.entry,
                &ctx.site_dir,
                ctx.meta.as_ref(),
            );
        }

        ("GET", p) if p.ends_with(".crepus") || p.ends_with(".html") => {
            let template_key = p.trim_start_matches('/');
            let template_key = if let Some(stem) = template_key.strip_suffix(".html") {
                format!("{stem}.crepus")
            } else {
                template_key.to_string()
            };
            if template_key == ctx.entry {
                serve_index_document(
                    &mut stream,
                    &ctx.vfm,
                    &ctx.entry,
                    &ctx.site_dir,
                    ctx.meta.as_ref(),
                );
            } else {
                serve_secondary_preview(
                    &mut stream,
                    &ctx.vfm,
                    &template_key,
                    &ctx.site_dir,
                    ctx.meta.as_ref(),
                );
            }
        }

        ("GET", p) if p.starts_with("/static/") => {
            serve_static_file(&mut stream, p, &ctx.site_dir);
        }

        _ => {
            let body = "<html><body><h1>404 Not Found</h1></body></html>";
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}

/// Full document with SSR: pre-renders entry template server-side so content is visible
/// immediately while WASM loads in background for interactivity.
fn serve_index_document(
    stream: &mut TcpStream,
    vfm: &Arc<RwLock<HashMap<String, String>>>,
    entry: &str,
    site_dir: &Path,
    meta: Option<&WebTargetMeta>,
) {
    let files = vfm
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let mut html = render_dev_index_html(site_dir, &files, meta, "");

    if !html_contains_entry(&files, entry) {
        let msg = format!("file not found in virtual fs: {entry}");
        html = error_document(&msg);
    } else {
        // SSR: render entry template server-side so first paint is instant
        let ctx = TemplateContext::new();
        let needle = r#"<div id="crepus-root"></div>"#;
        if let Some(pos) = html.find(needle) {
            let ssr_inner = match render_from_files_with_ssr(&files, entry, &ctx, true) {
                Ok(h) => h,
                Err(e) => format!("<!-- ssr error: {} -->", escape_html(&e.to_string())),
            };
            // Embed bundle JSON inline to eliminate separate HTTP fetch
            let bundle = serde_json::json!({"entry": entry, "files": &files});
            let bundle_str = serde_json::to_string(&bundle).unwrap_or_default();
            let bundle_escaped = crate::web::build::ssr_escape_json(&bundle_str);
            let replacement = format!(
                r#"<div id="crepus-root">{}</div><script id="__crepus_bundle__" type="application/json">{}</script>"#,
                ssr_inner, bundle_escaped
            );
            html.replace_range(pos..pos + needle.len(), &replacement);
        }
        inject_reload_before_body_end(&mut html);
    }

    write_html_response(stream, &html);
}

fn html_contains_entry(files: &HashMap<String, String>, entry: &str) -> bool {
    if files.contains_key(entry) {
        return true;
    }
    entry
        .split_once('#')
        .is_some_and(|(file, _)| files.contains_key(file))
}

/// SSR-only preview for a non-entry template (same UnoCSS + theme, no WASM).
fn serve_secondary_preview(
    stream: &mut TcpStream,
    vfm: &Arc<RwLock<HashMap<String, String>>>,
    template_key: &str,
    site_dir: &Path,
    meta: Option<&WebTargetMeta>,
) {
    let files = vfm
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let ctx = TemplateContext::new();
    let inner = match render_from_files(&files, template_key, &ctx) {
        Ok(h) => h,
        Err(e) => format!(
            "<div style=\"font-family:monospace;padding:2rem\">\
             <h2 style=\"color:#ef4444\">Template error</h2><pre>{}</pre></div>",
            escape_html(&e.to_string())
        ),
    };

    let mut html = render_dev_index_html(site_dir, &files, meta, "");

    let needle = r#"<div id="crepus-root"></div>
  <script type="module" src="./app.js"></script>"#;
    if let Some(pos) = html.find(needle) {
        let replacement = format!(
            r#"<div id="crepus-root" style="opacity:1">{inner}</div>
  <!-- crepus web dev: preview only (entry is WASM on /) -->"#
        );
        html.replace_range(pos..pos + needle.len(), &replacement);
    } else {
        html = error_document("dev server: could not patch index shell (internal)");
    }
    html = html.replace("./vendor/unocss.js", "/vendor/unocss.js");
    inject_reload_before_body_end(&mut html);
    write_html_response(stream, &html);
}

fn render_dev_index_html(
    site_dir: &Path,
    files: &HashMap<String, String>,
    meta: Option<&WebTargetMeta>,
    template_head: &str,
) -> String {
    let head = merge_site_head_meta(load_site_head(site_dir), meta);
    let google_fonts = merged_site_google_fonts(site_dir, files, meta);
    let inline_css = merged_site_inline_css(files);
    render_index_html(&head, &google_fonts, &inline_css, template_head)
}

fn inject_reload_before_body_end(html: &mut String) {
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, RELOAD_SCRIPT);
    } else {
        html.push_str(RELOAD_SCRIPT);
    }
}

fn write_html_response(stream: &mut TcpStream, html: &str) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn error_document(message: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Error</title></head>\
         <body style=\"font-family:monospace;padding:2rem\"><pre>{}</pre></body></html>",
        escape_html(message)
    )
}

fn serve_crepus_bundle(
    stream: &mut TcpStream,
    vfm: &Arc<RwLock<HashMap<String, String>>>,
    entry: &str,
) {
    let files = vfm
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let body = json!({ "entry": entry, "files": files }).to_string();
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn serve_dev_fs_file(stream: &mut TcpStream, path: &Path) {
    let mime = guess_mime(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
    );
    serve_dev_fs_file_with_mime(stream, path, mime);
}

fn serve_dev_fs_file_with_mime(stream: &mut TcpStream, path: &Path, mime: &str) {
    match std::fs::read(path) {
        Ok(bytes) => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n",
                bytes.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&bytes);
        }
        Err(_) => write_simple_not_found(stream),
    }
}

fn serve_pkg_path(stream: &mut TcpStream, url_path: &str, dev_root: &Path) {
    let rel = url_path.trim_start_matches("/pkg/").trim_start_matches('/');
    let Some(path) = resolve_static_file(&dev_root.join("pkg"), rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file_with_mime(stream, &path, logical_path_mime(rel));
}

fn serve_islands_path(stream: &mut TcpStream, url_path: &str, dev_root: &Path) {
    let rel = url_path
        .trim_start_matches("/islands/")
        .trim_start_matches('/');
    let Some(path) = resolve_static_file(&dev_root.join("islands"), rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file_with_mime(stream, &path, logical_path_mime(rel));
}

fn write_simple_not_found(stream: &mut TcpStream) {
    let body = b"Not found";
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    let _ = stream.write_all(body);
}

fn decode_url_path(url_path: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(url_path.len());
    let mut bytes = url_path.bytes();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    let url_path = String::from_utf8(decoded).ok()?;
    if url_path.contains('\0') {
        return None;
    }
    Some(url_path)
}

fn logical_path_mime(url_path: &str) -> &'static str {
    let decoded = decode_url_path(url_path).unwrap_or_default();
    guess_mime(
        Path::new(&decoded)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
    )
}

/// Decode a URL path once and resolve it under `site_dir`, rejecting `..` and symlink escapes.
pub(crate) fn resolve_static_file(site_dir: &Path, url_path: &str) -> Option<PathBuf> {
    let decoded = decode_url_path(url_path)?;
    resolve_under_dir(site_dir, decoded.trim_start_matches('/'))
}

fn serve_static_file(stream: &mut TcpStream, url_path: &str, site_dir: &Path) {
    let Some(file_path) = resolve_static_file(site_dir, url_path) else {
        write_simple_not_found(stream);
        return;
    };

    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let mime = logical_path_mime(url_path);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\r\n",
                bytes.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&bytes);
        }
        Err(_) => {
            let body = b"Not found";
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(body);
        }
    }
}

/// Keep the SSE stream open across updates so edits during reconnect delays are not lost.
fn long_poll_sse(
    mut stream: TcpStream,
    generation: Arc<AtomicU64>,
    last_sse_msg: Arc<RwLock<String>>,
) {
    let mut start_gen = generation.load(Ordering::Acquire);
    let headers =
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();

    if stream
        .set_read_timeout(Some(Duration::from_millis(1)))
        .is_err()
    {
        return;
    }

    loop {
        std::thread::sleep(Duration::from_millis(100));
        match stream.peek(&mut [0u8; 1]) {
            Ok(0) => break,
            Err(error)
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break
            }
            _ => {}
        }
        let next_gen = generation.load(Ordering::Acquire);
        if next_gen != start_gen {
            let msg = last_sse_msg
                .read()
                .map(|g| g.clone())
                .unwrap_or_else(|_| "data: reload\n\n".to_string());
            let msg = if msg.is_empty() {
                "data: reload\n\n".to_string()
            } else {
                msg
            };
            if stream.write_all(msg.as_bytes()).is_err() || stream.flush().is_err() {
                break;
            }
            start_gen = next_gen;
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn guess_mime(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use crate::crepus_toml::WebTargetMeta;
    use std::collections::HashMap;

    #[test]
    fn directory_events_reconcile_template_descendants() {
        use std::sync::{Arc, RwLock};
        let rt = tokio::runtime::Runtime::new().unwrap();
        let site_dir = tempfile::tempdir().unwrap();
        let site = site_dir.path().canonicalize().unwrap();
        let files = Arc::new(RwLock::new(HashMap::new()));
        let apply = |paths: &[std::path::PathBuf]| {
            let mut pending = Vec::new();
            let mut island = None;
            for path in paths {
                assert!(super::is_authored_site_path(site.as_path(), path));
                match super::classify_authored_path(path) {
                    super::WatchEvent::Crepus(path) => {
                        super::queue_template_update(path, &mut pending, &mut island)
                    }
                    super::WatchEvent::IslandDirty(path) => {
                        super::queue_template_update(path, &mut pending, &mut island)
                    }
                    super::WatchEvent::RuntimeDirty => unreachable!(),
                }
            }
            super::apply_template_updates(site.as_path(), &files, &mut pending, &rt);
        };
        let folder = site.as_path().join("views");
        std::fs::create_dir_all(folder.join("nested")).unwrap();
        std::fs::write(folder.join("nested/card.crepus"), "span \"FIRST\"").unwrap();
        std::fs::write(folder.join("child.vue"), "<template><p>Vue</p></template>").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&folder, folder.join("self-cycle")).unwrap();
        std::fs::create_dir(folder.join("node_modules")).unwrap();
        std::fs::write(folder.join("node_modules/ignored.crepus"), "div").unwrap();
        apply(std::slice::from_ref(&folder));
        {
            let map = files.read().unwrap();
            assert_eq!(
                map.len(),
                2,
                "directory create/move-in must collect descendants: {map:?}"
            );
            assert!(map.contains_key("views/nested/card.crepus"));
            assert!(map.contains_key("views/child.vue"));
        }
        let renamed = site.as_path().join("renamed.vue");
        std::fs::rename(&folder, &renamed).unwrap();
        apply(&[folder, renamed.clone()]);
        {
            let map = files.read().unwrap();
            assert_eq!(
                map.len(),
                2,
                "directory rename must replace old prefix: {map:?}"
            );
            assert!(map.contains_key("renamed.vue/nested/card.crepus"));
            assert!(map.contains_key("renamed.vue/child.vue"));
            assert!(
                !map.contains_key("renamed.vue"),
                "extension-named folder is not a source"
            );
        }
        std::fs::remove_dir_all(&renamed).unwrap();
        apply(std::slice::from_ref(&renamed));
        assert!(
            files.read().unwrap().is_empty(),
            "removed directory must evict descendants"
        );
        let incoming = tempfile::tempdir().unwrap();
        std::fs::create_dir(incoming.path().join("populated")).unwrap();
        std::fs::write(
            incoming.path().join("populated/moved.astro"),
            "<p>Moved</p>",
        )
        .unwrap();
        let destination = site.as_path().join("moved-in");
        std::fs::rename(incoming.path().join("populated"), &destination).unwrap();
        apply(std::slice::from_ref(&destination));
        assert_eq!(
            files.read().unwrap()["moved-in/moved.astro"],
            "<p>Moved</p>"
        );
        for ignored in [
            "dist",
            "target",
            ".git",
            "node_modules",
            ".crepus-dev",
            "runtime",
        ] {
            assert!(!super::is_authored_site_path(
                site.as_path(),
                &site.as_path().join(ignored)
            ));
        }
    }

    #[test]
    #[cfg(unix)]
    fn directory_event_scans_preserve_descendant_event_path_policy() {
        use std::sync::{Arc, RwLock};
        let rt = tokio::runtime::Runtime::new().unwrap();
        let site_dir = tempfile::tempdir().unwrap();
        let site = site_dir.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = site.join("views");
        std::fs::create_dir_all(folder.join("runtime")).unwrap();
        std::fs::write(folder.join("runtime/private.crepus"), "PRIVATE").unwrap();
        std::fs::create_dir(folder.join("normal")).unwrap();
        std::fs::write(folder.join("normal/card.crepus"), "PUBLIC").unwrap();
        std::fs::create_dir(site.join("target")).unwrap();
        std::fs::write(site.join("target/generated.crepus"), "GENERATED").unwrap();
        std::fs::write(outside.path().join("outside.crepus"), "OUTSIDE").unwrap();
        std::os::unix::fs::symlink(outside.path(), folder.join("external")).unwrap();
        std::os::unix::fs::symlink(site.join("target"), folder.join("generated")).unwrap();
        std::os::unix::fs::symlink(folder.join("normal"), folder.join("alias")).unwrap();
        assert!(super::is_authored_site_path(&site, &folder));
        let files = Arc::new(RwLock::new(HashMap::new()));
        let mut pending = Vec::new();
        let mut island = None;
        match super::classify_authored_path(&folder) {
            super::WatchEvent::IslandDirty(path) => {
                super::queue_template_update(path, &mut pending, &mut island)
            }
            _ => panic!("directory must take full-reload route"),
        }
        super::apply_template_updates(&site, &files, &mut pending, &rt);
        let map = files.read().unwrap();
        assert_eq!(
            map.len(),
            2,
            "event scans must enforce descendant path policy: {map:?}"
        );
        assert_eq!(map["views/normal/card.crepus"], "PUBLIC");
        assert_eq!(map["views/alias/card.crepus"], "PUBLIC");
    }

    #[test]
    fn directory_reconciliation_preserves_siblings_and_handles_replacement_and_duplicate_events() {
        use std::sync::{Arc, RwLock};
        let rt = tokio::runtime::Runtime::new().unwrap();
        let site_dir = tempfile::tempdir().unwrap();
        let site = site_dir.path().canonicalize().unwrap();
        let folder = site.as_path().join("views.crepus");
        std::fs::create_dir(&folder).unwrap();
        let child = folder.join("child.vue");
        std::fs::write(&child, "<p>Child</p>").unwrap();
        let files = Arc::new(RwLock::new(HashMap::from([
            ("views.crepus-other/keep.crepus".into(), "KEEP".into()),
            ("outside.crepus".into(), "OUTSIDE".into()),
        ])));
        let mut pending = Vec::new();
        let mut island = None;
        for path in [&folder, &child, &folder] {
            match super::classify_authored_path(path) {
                super::WatchEvent::Crepus(path) | super::WatchEvent::IslandDirty(path) => {
                    super::queue_template_update(path, &mut pending, &mut island)
                }
                _ => unreachable!(),
            }
        }
        assert!(
            island.is_some(),
            "extension-named directory must request full reload"
        );
        super::apply_template_updates(site.as_path(), &files, &mut pending, &rt);
        assert_eq!(files.read().unwrap().len(), 3);
        std::fs::remove_dir_all(&folder).unwrap();
        std::fs::write(&folder, "span \"Replacement\"").unwrap();
        let mut pending = vec![folder];
        super::apply_template_updates(site.as_path(), &files, &mut pending, &rt);
        let map = files.read().unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map["views.crepus"], "span \"Replacement\"");
        assert_eq!(map["views.crepus-other/keep.crepus"], "KEEP");
        assert_eq!(map["outside.crepus"], "OUTSIDE");
        assert!(!map.contains_key("views.crepus/child.vue"));
    }

    #[test]
    fn dev_frontend_events_update_create_rename_and_remove_bundle_sources() {
        use std::sync::{Arc, RwLock};
        let rt = tokio::runtime::Runtime::new().unwrap();
        let site = tempfile::tempdir().unwrap();
        let files = Arc::new(RwLock::new(HashMap::new()));
        for extension in [
            "vue",
            "svelte",
            "jsx",
            "tsx",
            "csx",
            "astro",
            "ng",
            "ng.html",
            "component.html",
        ] {
            let path = site.path().join(format!("child.{extension}"));
            let renamed = site.path().join(format!("renamed.{extension}"));
            for value in ["FIRST", "UPDATED"] {
                std::fs::write(&path, value).unwrap();
                let super::WatchEvent::Crepus(event) = super::classify_authored_path(&path) else {
                    panic!("not a template: {}", path.display())
                };
                let mut pending = Vec::new();
                let mut island_path = None;
                super::queue_template_update(event, &mut pending, &mut island_path);
                assert_eq!(island_path.as_ref(), Some(&path));
                let (changed, last) =
                    super::apply_template_updates(site.path(), &files, &mut pending, &rt);
                assert_eq!(changed, 1);
                assert_eq!(last.as_ref(), Some(&path));
                assert!(pending.is_empty());
                assert_eq!(files.read().unwrap()[&format!("child.{extension}")], value);
            }
            std::fs::rename(&path, &renamed).unwrap();
            let mut pending = Vec::new();
            let mut island_path = None;
            for path in [&path, &renamed] {
                let super::WatchEvent::Crepus(event) = super::classify_authored_path(path) else {
                    panic!("rename must update template sources")
                };
                super::queue_template_update(event, &mut pending, &mut island_path);
            }
            assert_eq!(island_path.as_ref(), Some(&renamed));
            super::apply_template_updates(site.path(), &files, &mut pending, &rt);
            assert!(!files
                .read()
                .unwrap()
                .contains_key(&format!("child.{extension}")));
            assert_eq!(
                files.read().unwrap()[&format!("renamed.{extension}")],
                "UPDATED"
            );
            std::fs::remove_file(&renamed).unwrap();
            let super::WatchEvent::Crepus(event) = super::classify_authored_path(&renamed) else {
                panic!("removal must update template sources")
            };
            let mut pending = Vec::new();
            let mut island_path = None;
            super::queue_template_update(event, &mut pending, &mut island_path);
            assert_eq!(island_path.as_ref(), Some(&renamed));
            super::apply_template_updates(site.path(), &files, &mut pending, &rt);
            assert!(files.read().unwrap().is_empty());
        }
        for filename in ["island.js", "island.ts", "style.css"] {
            assert!(matches!(
                super::classify_authored_path(&site.path().join(filename)),
                super::WatchEvent::IslandDirty(_)
            ));
        }
    }

    #[test]
    fn raw_island_frontend_refreshes_bytes_and_requests_full_reload() {
        use std::sync::{Arc, RwLock};
        let rt = tokio::runtime::Runtime::new().unwrap();
        let site = tempfile::tempdir().unwrap();
        let path = site.path().join("Counter.tsx");
        let source = "import React from 'react'; export default () => <button>Updated</button>;";
        std::fs::write(&path, source).unwrap();
        let files = Arc::new(RwLock::new(HashMap::new()));
        let super::WatchEvent::Crepus(event) = super::classify_authored_path(&path) else {
            panic!("frontend bytes must refresh")
        };
        let mut pending = Vec::new();
        let mut island_path = None;
        super::queue_template_update(event, &mut pending, &mut island_path);
        super::apply_template_updates(site.path(), &files, &mut pending, &rt);
        assert_eq!(files.read().unwrap()["Counter.tsx"], source);
        let sse = super::island_rebuild_sse_message(island_path.as_ref().unwrap());
        assert!(sse.starts_with("event: crepus-reload\n"));
        assert!(sse.contains("\"reload\":true"));
        let mut validated = Vec::new();
        super::walk_crepus_files(site.path(), &mut validated);
        assert!(
            validated.is_empty(),
            "opaque modules must not be startup templates"
        );
    }

    #[test]
    fn dev_bundle_collects_supported_included_frontends() {
        use std::sync::{Arc, RwLock};
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
            let files = Arc::new(RwLock::new(HashMap::new()));
            super::load_all_crepus(site.path(), &files);
            let map = files.read().unwrap();
            let bundle = serde_json::json!({"entry": "index.crepus", "files": &*map}).to_string();
            let html = crepuscularity_web::render_bundle(&bundle)
                .unwrap_or_else(|error| panic!("{filename}: {error}"));
            assert!(html.contains("VISIBLE"), "{filename}: {html}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn discovery_stops_cycles_and_preserves_logical_aliases() {
        use std::os::unix::fs::symlink;
        use std::sync::{Arc, RwLock};
        let site = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::create_dir(site.path().join("src")).unwrap();
        std::fs::write(
            site.path().join("index.crepus"),
            "include alias/card.crepus",
        )
        .unwrap();
        std::fs::write(site.path().join("src/card.crepus"), "span\n  \"Card\"").unwrap();
        std::fs::write(external.path().join("remote.crepus"), "div").unwrap();
        symlink(".", site.path().join("src/again")).unwrap();
        symlink(site.path().join("src"), site.path().join("alias")).unwrap();
        symlink(site.path().join("src"), site.path().join("alias2")).unwrap();
        symlink(external.path(), site.path().join("linked")).unwrap();
        for name in ["dist", "target", ".git", "node_modules", ".crepus-dev"] {
            std::fs::create_dir(site.path().join(name)).unwrap();
            std::fs::write(site.path().join(name).join("ignored.crepus"), "div").unwrap();
        }
        let messages = super::template_validation_messages(site.path());
        assert_eq!(messages.len(), 10, "validation must stop recursive aliases");
        assert!(messages.iter().all(|message| !message.contains("again")));
        let files = Arc::new(RwLock::new(HashMap::new()));
        super::load_all_crepus(site.path(), &files);
        let map = files.read().unwrap();
        let mut keys: Vec<_> = map.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "alias/card.crepus",
                "alias2/card.crepus",
                "index.crepus",
                "linked/remote.crepus",
                "src/card.crepus"
            ]
        );
        assert_eq!(map["alias/card.crepus"], map["src/card.crepus"]);
        assert_eq!(map["alias2/card.crepus"], map["src/card.crepus"]);
    }

    #[test]
    fn island_watch_filter_accepts_authored_deleted_and_atomic_save_paths() {
        let site = tempfile::tempdir().unwrap();
        for name in [
            "Widget.tsx",
            "lib/helper.ts",
            "package.json",
            "image.svg",
            ".Widget.tsx.swp",
            "deleted.js",
        ] {
            assert!(
                super::is_authored_site_path(site.path(), &site.path().join(name)),
                "{name}"
            );
        }
        for name in [
            ".crepus-dev/islands/app.js",
            "dist/app.js",
            "nested/target/file",
            "node_modules/react/index.js",
            ".git/index",
            "runtime/src/lib.rs",
            "ui/.crepus-react-island-123.tsx",
            "../outside.ts",
        ] {
            assert!(
                !super::is_authored_site_path(site.path(), &site.path().join(name)),
                "{name}"
            );
        }
        assert!(!super::is_authored_site_path(
            site.path(),
            &site.path().with_extension("outside")
        ));
    }

    #[test]
    fn island_reload_requests_full_reload_and_error_retains_message() {
        let message =
            super::island_rebuild_sse_message(std::path::Path::new("components/Widget.tsx"));
        let payload = message
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert_eq!(payload["file"], "Widget.tsx");
        assert_eq!(payload["reload"], true);
        assert!(super::RELOAD_SCRIPT
            .contains("info.reload === true || (info.file && info.file.includes('runtime'))"));
        let error = super::runtime_build_error_sse("Bundling Widget.tsx failed: bad import");
        assert!(error.starts_with("event: crepus-error\n"));
        assert!(error.contains("Bundling Widget.tsx failed: bad import"));
    }

    #[test]
    fn template_validation_messages_are_sorted_by_relative_path() {
        let site = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(site.path().join("nested")).expect("mkdir nested");
        for path in ["z.crepus", "a.crepus", "nested/m.crepus"] {
            std::fs::write(site.path().join(path), "div").expect("write template");
        }

        let messages = super::template_validation_messages(site.path());

        assert!(messages[0].ends_with(" a.crepus"));
        assert!(messages[1].ends_with(&format!(" nested{}m.crepus", std::path::MAIN_SEPARATOR)));
        assert!(messages[2].ends_with(" z.crepus"));
    }

    #[test]
    fn static_webp_asset_has_image_content_type() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        let site = tempfile::tempdir().unwrap();
        std::fs::write(site.path().join("photo.WEBP"), b"image fixture").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let (mut server, _) = listener.accept().unwrap();
        super::serve_static_file(&mut server, "/photo.WEBP", site.path());
        drop(server);
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: image/webp\r\n"));
        assert!(response.ends_with("\r\n\r\nimage fixture"));
    }

    #[test]
    fn runtime_worker_routes_serve_dev_assets() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        let site = tempfile::tempdir().unwrap();
        let dev = site.path().join(".crepus-dev");
        std::fs::create_dir(&dev).unwrap();
        let request = |route: &str| {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (server, _) = listener.accept().unwrap();
            let ctx = super::DevRequestContext {
                vfm: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
                generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
                last_sse_msg: std::sync::Arc::new(std::sync::RwLock::new(String::new())),
                entry: "index.crepus".into(),
                site_dir: site.path().to_path_buf(),
                dev_root: dev.clone(),
                meta: None,
            };
            client
                .write_all(format!("GET {route} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
                .unwrap();
            super::handle_connection(server, ctx);
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            response
        };
        for name in ["runtime-worker.js", "runtime-client.js"] {
            std::fs::write(site.path().join(name), "wrong source directory").unwrap();
            std::fs::write(dev.join(name), "export const fixture = true;").unwrap();
            let response = request(&format!("/{name}?v=1"));
            assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
            assert!(
                response.contains("Content-Type: application/javascript"),
                "{response}"
            );
            assert!(
                response.ends_with("export const fixture = true;"),
                "{response}"
            );
            assert!(!response.contains("wrong source directory"));
            std::fs::remove_file(dev.join(name)).unwrap();
            #[cfg(unix)]
            {
                let outside = tempfile::tempdir().unwrap();
                std::fs::write(outside.path().join("private.js"), "private fixture").unwrap();
                std::os::unix::fs::symlink(outside.path().join("private.js"), dev.join(name))
                    .unwrap();
                let response = request(&format!("/{name}"));
                assert!(
                    response.starts_with("HTTP/1.1 404 Not Found\r\n"),
                    "{response}"
                );
                assert!(!response.contains("private fixture"));
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn generated_asset_routes_block_symlink_escape() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private.js"), b"private fixture").unwrap();
        for (directory, serve) in [
            (
                "pkg",
                super::serve_pkg_path as fn(&mut TcpStream, &str, &std::path::Path),
            ),
            ("islands", super::serve_islands_path),
            ("docs", super::serve_docs_path),
        ] {
            std::fs::create_dir(root.path().join(directory)).unwrap();
            std::os::unix::fs::symlink(
                outside.path().join("private.js"),
                root.path().join(directory).join("escape.js"),
            )
            .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (mut server, _) = listener.accept().unwrap();
            serve(&mut server, &format!("/{directory}/escape.js"), root.path());
            drop(server);
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 404 Not Found\r\n"),
                "{directory}: {response}"
            );
            assert!(!response.contains("private fixture"));
        }
    }

    #[test]
    #[cfg(unix)]
    fn asset_routes_use_logical_symlink_extensions_for_mime() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::sync::{Arc, RwLock};
        let site = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private.bin"), "PRIVATE").unwrap();
        for directory in ["static", "pkg", "islands", "docs"] {
            let base = site.path().join(directory);
            std::fs::create_dir(&base).unwrap();
            std::fs::write(base.join("payload.bin"), "export const marker = 'VISIBLE';").unwrap();
            std::os::unix::fs::symlink("payload.bin", base.join("module.js")).unwrap();
            std::os::unix::fs::symlink(outside.path().join("private.bin"), base.join("escape.js"))
                .unwrap();
            for (filename, success) in [
                ("module.js?revision=1", true),
                ("module%2Ejs?revision=2", true),
                ("escape.js?revision=3", false),
            ] {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                client
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let (server, _) = listener.accept().unwrap();
                let ctx = super::DevRequestContext {
                    vfm: Arc::new(RwLock::new(HashMap::new())),
                    generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                    last_sse_msg: Arc::new(RwLock::new(String::new())),
                    entry: "index.crepus".into(),
                    site_dir: site.path().into(),
                    dev_root: site.path().into(),
                    meta: None,
                };
                client
                    .write_all(
                        format!("GET /{directory}/{filename} HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            .as_bytes(),
                    )
                    .unwrap();
                super::handle_connection(server, ctx);
                let mut response = String::new();
                client.read_to_string(&mut response).unwrap();
                if success {
                    assert!(
                        response.starts_with("HTTP/1.1 200 OK\r\n"),
                        "{directory}/{filename}: {response}"
                    );
                    assert!(
                        response.contains("\r\nContent-Type: application/javascript\r\n"),
                        "{directory}/{filename}: {response}"
                    );
                    assert_eq!(
                        response.split_once("\r\n\r\n").unwrap().1,
                        "export const marker = 'VISIBLE';"
                    );
                } else {
                    assert!(
                        response.starts_with("HTTP/1.1 404 Not Found\r\n"),
                        "{response}"
                    );
                    assert!(!response.contains("PRIVATE"));
                }
            }
        }
    }

    #[test]
    fn preview_routes_preserve_directory_names_and_docs_prefixes() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        let site = tempfile::tempdir().unwrap();
        for (route, template) in [
            ("/guide.html/page.html", "guide.html/page.crepus"),
            ("/docs-example.html", "docs-example.crepus"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (server, _) = listener.accept().unwrap();
            let ctx = super::DevRequestContext {
                vfm: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::from(
                    [(template.to_string(), "p \"preview fixture\"".to_string())],
                ))),
                generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
                last_sse_msg: std::sync::Arc::new(std::sync::RwLock::new(String::new())),
                entry: "index.crepus".to_string(),
                site_dir: site.path().to_path_buf(),
                dev_root: site.path().to_path_buf(),
                meta: None,
            };
            client.write_all(b"GET ").unwrap();
            let worker = std::thread::spawn(move || super::handle_connection(server, ctx));
            std::thread::sleep(std::time::Duration::from_millis(30));
            client
                .write_all(format!("{route} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
                .unwrap();
            worker.join().unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 200 OK\r\n"),
                "{route}: {response}"
            );
            assert!(
                response.contains("<p>preview fixture</p>"),
                "{route}: {response}"
            );
            assert!(response.contains(r#"id="crepus-root" style="opacity:1""#));
            assert!(response.contains(r#"src="/vendor/unocss.js""#));
        }
    }

    #[test]
    fn docs_routes_strip_only_the_mount_prefix() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        let root = tempfile::tempdir().unwrap();
        for (directory, relative, decoy, serve) in [
            (
                "docs",
                "docsmith.html",
                "mith.html",
                super::serve_docs_path as fn(&mut TcpStream, &str, &std::path::Path),
            ),
            (
                "docs",
                "docs/guide.html",
                "guide.html",
                super::serve_docs_path,
            ),
        ] {
            let base = root.path().join(directory);
            let target = base.join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(&target, "nested intended fixture").unwrap();
            std::fs::write(base.join(decoy), "wrong flattened fixture").unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (mut server, _) = listener.accept().unwrap();
            let route = format!("/{directory}/{relative}");
            serve(&mut server, &route, root.path());
            drop(server);
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 200 OK\r\n"),
                "{route}: {response}"
            );
            assert!(
                response.ends_with("\r\n\r\nnested intended fixture"),
                "{route}: {response}"
            );
            assert!(
                !response.contains("wrong flattened fixture"),
                "{route}: {response}"
            );
        }
    }

    #[test]
    fn docs_root_serves_generated_index() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("docs")).unwrap();
        std::fs::write(root.path().join("docs/index.html"), "<p>generated docs</p>").unwrap();
        for route in ["/docs", "/docs/", "/docs/index.html"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (mut server, _) = listener.accept().unwrap();
            super::serve_docs_path(&mut server, route, root.path());
            drop(server);
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
            assert!(response.ends_with("\r\n\r\n<p>generated docs</p>"));
        }
    }

    #[test]
    fn sse_connection_delivers_multiple_updates() {
        use std::io::{BufRead, BufReader};
        use std::net::{Shutdown, TcpListener, TcpStream};
        use std::sync::{
            atomic::{AtomicU64, Ordering},
            Arc, RwLock,
        };
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let shutdown = server.try_clone().unwrap();
        let generation = Arc::new(AtomicU64::new(0));
        let message = Arc::new(RwLock::new(String::new()));
        let worker = std::thread::spawn({
            let generation = generation.clone();
            let message = message.clone();
            move || super::long_poll_sse(server, generation, message)
        });
        let mut reader = BufReader::new(client);
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        for update in 1..=2 {
            *message.write().unwrap() = format!("data: {update}\n\n");
            generation.store(update, Ordering::Release);
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            assert_eq!(line, format!("data: {update}\n"));
            line.clear();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "\n");
        }
        reader.get_ref().shutdown(Shutdown::Both).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !worker.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let disconnected = worker.is_finished();
        let _ = shutdown.shutdown(Shutdown::Both);
        for _ in 0..20 {
            if worker.is_finished() {
                break;
            }
            generation.fetch_add(1, Ordering::Release);
            std::thread::sleep(Duration::from_millis(100));
        }
        if worker.is_finished() {
            worker.join().unwrap();
        }
        assert!(disconnected, "idle disconnected SSE worker did not exit");
    }

    #[test]
    fn error_events_preserve_control_characters_and_literal_escapes() {
        let message = "界\nline\r\nnext\rdata: fake\\n";
        let event = super::runtime_build_error_sse(message);
        let lines: Vec<_> = event.lines().collect();
        assert_eq!(lines.len(), 3);
        let decoded: String =
            serde_json::from_str(lines[1].strip_prefix("data: ").expect("data line"))
                .expect("JSON error string");
        assert_eq!(decoded, message);
    }

    #[cfg(unix)]
    #[test]
    fn reload_events_escape_special_characters_in_filenames() {
        let site = tempfile::tempdir().expect("tempdir");
        let filename = "quoted\"\\\n.crepus";
        let path = site.path().join(filename);
        std::fs::write(&path, "div").expect("write template");
        let event = super::build_sse_message(&path);
        let lines: Vec<_> = event.lines().collect();
        assert_eq!(lines.len(), 3);
        let payload: serde_json::Value =
            serde_json::from_str(lines[1].strip_prefix("data: ").expect("data line"))
                .expect("valid JSON");
        assert_eq!(payload["file"], filename);
    }

    #[test]
    fn static_routes_decode_encoded_asset_names_once() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        let root = tempfile::tempdir().unwrap();
        for (filename, route) in [
            ("hello world.txt", "/hello%20world.txt"),
            ("界.txt", "/%E7%95%8C.txt"),
            ("literal%20.txt", "/literal%2520.txt"),
            ("a+b.txt", "/a+b.txt"),
        ] {
            std::fs::write(root.path().join(filename), filename).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let (mut server, _) = listener.accept().unwrap();
            super::serve_static_file(&mut server, route, root.path());
            drop(server);
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 200 OK\r\n"),
                "{route}: {response}"
            );
            assert!(
                response.ends_with(&format!("\r\n\r\n{filename}")),
                "{route}: {response}"
            );
        }
    }

    #[test]
    fn resolve_static_file_allows_dots_inside_asset_names() {
        let site = tempfile::tempdir().expect("tempdir");
        let asset = site.path().join("app..css");
        std::fs::write(&asset, "body {}").expect("write asset");
        assert_eq!(
            super::resolve_static_file(site.path(), "/app..css"),
            Some(asset.canonicalize().expect("canonicalize")),
        );
    }

    #[test]
    fn resolve_static_file_rejects_parent_segments() {
        let site = tempfile::tempdir().expect("tempdir");
        assert!(super::resolve_static_file(site.path(), "../secret.txt").is_none());
        assert!(super::resolve_static_file(site.path(), "static/../../etc/passwd").is_none());
        for path in [
            "/%2e%2e/secret.txt",
            "/static/%2E%2E/%2e%2e/secret.txt",
            "/%",
            "/%2",
            "/%GG",
            "/%FF",
            "/%00",
        ] {
            assert!(
                super::resolve_static_file(site.path(), path).is_none(),
                "{path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn resolve_static_file_blocks_symlink_escape() {
        use std::os::unix::fs::symlink;

        let site = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret.txt"), "nope").expect("write secret");
        symlink(outside.path(), site.path().join("escape")).expect("symlink");

        assert!(super::resolve_static_file(site.path(), "escape/secret.txt").is_none());
        assert!(super::resolve_static_file(site.path(), "%65scape%2fsecret.txt").is_none());
    }

    #[test]
    fn resolve_static_file_allows_in_tree_asset() {
        let site = tempfile::tempdir().expect("tempdir");
        let asset = site.path().join("static").join("app.css");
        std::fs::create_dir_all(asset.parent().expect("parent")).expect("mkdir");
        std::fs::write(&asset, "body{}").expect("write css");

        let resolved = super::resolve_static_file(site.path(), "static/app.css").expect("asset");
        assert_eq!(
            resolved,
            std::fs::canonicalize(&asset).expect("canonicalize")
        );
    }

    #[test]
    fn resolve_under_dir_allows_in_tree_and_rejects_parent() {
        let root = tempfile::tempdir().expect("tempdir");
        let pkg = root.path().join("pkg");
        std::fs::create_dir_all(&pkg).expect("mkdir pkg");
        std::fs::write(pkg.join("runtime.js"), "ok").expect("write");

        assert!(super::resolve_under_dir(&pkg, "runtime.js").is_some());
        assert!(super::resolve_under_dir(&pkg, "../secret").is_none());
        assert!(super::resolve_under_dir(&pkg, "").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_under_dir_blocks_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("tempdir");
        let pkg = root.path().join("pkg");
        std::fs::create_dir_all(&pkg).expect("mkdir pkg");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret.txt"), "nope").expect("write secret");
        symlink(outside.path(), pkg.join("escape")).expect("symlink");

        assert!(super::resolve_under_dir(&pkg, "escape/secret.txt").is_none());
    }

    #[test]
    fn dev_index_merges_target_head_html() {
        let site_dir = tempfile::tempdir().expect("tempdir");
        let files = HashMap::from([("index.crepus".to_string(), "div\n  \"Hello\"".to_string())]);
        let meta = WebTargetMeta {
            head_html: Some(r#"<style>.from-target{color:red}</style>"#.into()),
            ..WebTargetMeta::default()
        };

        let html = super::render_dev_index_html(site_dir.path(), &files, Some(&meta), "");

        assert!(html.contains(r#"<style>.from-target{color:red}</style>"#));
    }
}
