//! `crepus web dev` — hot-reload dev server for `.crepus` templates.
//!
//! # How it works
//!
//! 1. Walks `site_dir` and loads all `*.crepus` files into an in-memory
//!    **virtual file map** (`Arc<RwLock<HashMap<String, String>>>`).
//! 2. A `notify` watcher fires on every `.crepus` change; a debounce thread
//!    batches events within a 50 ms window then updates the virtual file map
//!    and bumps a generation counter.
//! 3. A plain `std::net::TcpListener` accept-loop spawns one thread per
//!    connection — fine for a dev server with ≤ a handful of browser tabs.
//! 4. `GET /dev-reload` is a **Server-Sent Events** endpoint: the browser
//!    keeps the connection open; when the generation counter changes the
//!    server sends `data: reload\n\n` and closes the connection. The browser's
//!    `EventSource` auto-reconnects on the next page load.
//! 5. On startup the server runs the same **wasm32 + wasm-bindgen** step as `crepus web build`,
//!    writing UnoCSS, `app.js`, and `pkg/` under **`.crepus-dev/`** beside the site. Saving
//!    **`runtime/**/*.rs`** (or `Cargo.toml` / `Cargo.lock` in that crate) repeats that build and
//!    triggers a full page reload — same idea as `cargo-leptos` rebuilding the client WASM, except
//!    the Rust compiler still recompiles the world (there is no incremental “hot swap” of native code).
//! 6. **`GET /crepus-bundle.json`** returns the live virtual `.crepus` map (for the browser WASM renderer).
//! 7. **`GET /**` for the configured entry serves the real HTML shell** (`vendor/unocss.js`, `app.js`, `#crepus-root`)
//!    plus a dev-reload script. Subordinate `.html` routes get a static preview shell (no WASM) with UnoCSS.
//! 8. Static assets **`/static/*`** continue to resolve under the site directory.

use std::collections::HashMap;
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

/// Watcher event: template source or site `runtime/` (WASM crate).
enum WatchEvent {
    Crepus(PathBuf),
    RuntimeDirty,
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
    let escaped = message.replace('\n', "\\n");
    format!("event: crepus-error\ndata: {escaped}\n\n")
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
    if (info.file && info.file.includes('runtime')) {
      showToast('\u{21bb} '+(info.file||'template')+' updated');
      setTimeout(function(){location.reload();},150);
    } else {
      fetch('./crepus-bundle.json', {cache: 'no-store'}).then(r => r.json()).then(function(bundle) {
        currentBundle = bundle;
        rerender();
        showToast('\u{21bb} '+(info.file||'template')+' updated');
      }).catch(function(){
        location.reload();
      });
    }
  });
  es.addEventListener('crepus-error',function(e){
    showErrorOverlay(e.data.replace(/\\\\n/g,'\\n'));
  });
  es.onmessage=function(e){if(e.data==='reload'){dismissErrorOverlay();location.reload();}};
  es.onerror=function(){};
})();
</script>";

// ── Startup validation ───────────────────────────────────────────────────────

/// Recursively collect `.crepus` file paths under `dir`.
fn walk_crepus_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_crepus_files(&path, out);
        } else if path.extension().and_then(|x| x.to_str()) == Some("crepus") {
            out.push(path);
        }
    }
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
pub fn run(opts: ServeOptions) {
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
    if let Err(e) = ensure_web_dev_artifacts(&site_dir) {
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
    );

    // Bind TCP listener.
    let addr = format!("127.0.0.1:{}", opts.port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("crepus web dev: cannot bind {addr}: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "\n  {} crepus web dev\n  {} http://localhost:{}\n  {} edit .crepus or runtime/ — templates hot-reload; Rust changes rebuild WASM\n",
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

/// Walk `site_dir` recursively and load every `*.crepus` file into `vfm`.
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "dist" | "target" | ".git" | "node_modules" | ".crepus-dev"
            ) {
                continue;
            }
            load_dir_recursive_collect(&path, paths);
        } else if path.extension().is_some_and(|e| e == "crepus") {
            paths.push(path);
        }
    }
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
            "event: crepus-reload\ndata: {{\"file\":\"{}\",\"ts\":{}}}\n\n",
            filename, ts
        ),
        Ok(Err(e)) => {
            let escaped = e.to_string().replace('\n', "\\n");
            format!("event: crepus-error\ndata: {escaped}\n\n")
        }
        Err(e) => {
            let escaped = e.to_string().replace('\n', "\\n");
            format!("event: crepus-error\ndata: read error: {escaped}\n\n")
        }
    }
}

// ── File watcher + debounce ──────────────────────────────────────────────────

fn start_watcher(
    site_dir: PathBuf,
    vfm: Arc<RwLock<HashMap<String, String>>>,
    generation: Arc<AtomicU64>,
    last_sse_msg: Arc<RwLock<String>>,
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
                        if path.extension().and_then(|e| e.to_str()) == Some("crepus") {
                            let _ = tx2.send(WatchEvent::Crepus(path.clone()));
                        } else if is_runtime_source_path(&site_for_filter, path) {
                            let _ = tx2.send(WatchEvent::RuntimeDirty);
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
        let mut last_event = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(WatchEvent::Crepus(p)) => {
                    pending_crepus.push(p);
                    last_event = Instant::now();
                }
                Ok(WatchEvent::RuntimeDirty) => {
                    runtime_dirty = true;
                    last_event = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if pending_crepus.is_empty() && !runtime_dirty {
                        continue;
                    }
                    if last_event.elapsed() < Duration::from_millis(50) {
                        continue;
                    }
                    let apply_start = Instant::now();
                    let mut changed = 0usize;
                    let mut last_crepus_path: Option<PathBuf> = None;

                    if !pending_crepus.is_empty() {
                        let results = rt.block_on(async {
                            let mut futures = Vec::new();
                            for path in pending_crepus.drain(..) {
                                let site_dir = site_dir.clone();
                                futures.push(tokio::task::spawn(async move {
                                    let key = relative_key(&site_dir, &path);
                                    let res = tokio::fs::read_to_string(&path).await;
                                    (key, path, res)
                                }));
                            }
                            futures_util::future::join_all(futures).await
                        });

                        let mut map = vfm
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        for (key, path, result) in results.into_iter().flatten() {
                            match result {
                                Ok(content) => {
                                    map.insert(key, content);
                                    changed += 1;
                                    last_crepus_path = Some(path);
                                }
                                Err(_) => {
                                    map.remove(&key);
                                    changed += 1;
                                    last_crepus_path = Some(path);
                                }
                            }
                        }
                        drop(map);
                        let map = vfm
                            .read()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        let dev_root = site_dir.join(crate::web::WEB_DEV_ARTIFACT_DIR);
                        if let Err(e) =
                            crate::web_islands::build_web_islands(&site_dir, &dev_root, &map)
                        {
                            eprintln!("  {} island build failed: {e}", console::style("✗").red());
                        }
                    }

                    let mut runtime_rebuilt = false;
                    let wasm_error: Option<String> = if runtime_dirty {
                        runtime_dirty = false;
                        match ensure_web_dev_artifacts(&site_dir) {
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
                    } else if let Some(ref path) = last_crepus_path {
                        build_sse_message(path)
                    } else if runtime_rebuilt {
                        runtime_rebuild_sse_message()
                    } else {
                        continue;
                    };

                    let should_notify = changed > 0 || runtime_rebuilt || wasm_failed;
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
                            "  {} hot reloaded {} files in {:.2}s",
                            console::style("✓").green(),
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
    if rel.is_empty() || rel.contains("..") {
        return None;
    }
    let base = std::fs::canonicalize(base).ok()?;
    let candidate = base.join(rel);
    let resolved = std::fs::canonicalize(&candidate).ok()?;
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
    let mut rel = url_path.trim_start_matches("/docs").trim_start_matches('/');
    if rel.is_empty() {
        rel = "index.html";
    }
    let Some(path) = resolve_under_dir(&base, rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file(stream, &path);
}

// ── HTTP connection handler ──────────────────────────────────────────────────

fn handle_connection(mut stream: TcpStream, ctx: DevRequestContext) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    // Read the request line (first line only).
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
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

        ("GET", "/vendor/unocss.js") => {
            serve_dev_fs_file(&mut stream, &ctx.dev_root.join("vendor/unocss.js"));
        }

        ("GET", p) if p.starts_with("/pkg/") => {
            serve_pkg_path(&mut stream, p, &ctx.dev_root);
        }

        ("GET", p) if p.starts_with("/islands/") => {
            serve_islands_path(&mut stream, p, &ctx.dev_root);
        }

        ("GET", p) if p.starts_with("/docs") => {
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
            let template_key = if template_key.ends_with(".html") {
                template_key.replace(".html", ".crepus")
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
            let bundle_escaped = bundle_str
                .replace("</script>", "<\\/script>")
                .replace("</Script>", "<\\/Script>");
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
            r#"<div id="crepus-root">{inner}</div>
  <!-- crepus web dev: preview only (entry is WASM on /) -->"#
        );
        html.replace_range(pos..pos + needle.len(), &replacement);
    } else {
        html = error_document("dev server: could not patch index shell (internal)");
    }
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
    match std::fs::read(path) {
        Ok(bytes) => {
            let mime = guess_mime(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
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
    let Some(path) = resolve_under_dir(&dev_root.join("pkg"), rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file(stream, &path);
}

fn serve_islands_path(stream: &mut TcpStream, url_path: &str, dev_root: &Path) {
    let rel = url_path
        .trim_start_matches("/islands/")
        .trim_start_matches('/');
    let Some(path) = resolve_under_dir(&dev_root.join("islands"), rel) else {
        write_simple_not_found(stream);
        return;
    };
    serve_dev_fs_file(stream, &path);
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

/// Resolve a static asset under `site_dir`, rejecting `..` and symlink escapes.
pub(crate) fn resolve_static_file(site_dir: &Path, url_path: &str) -> Option<PathBuf> {
    resolve_under_dir(site_dir, url_path.trim_start_matches('/'))
}

fn serve_static_file(stream: &mut TcpStream, url_path: &str, site_dir: &Path) {
    let Some(file_path) = resolve_static_file(site_dir, url_path) else {
        write_simple_not_found(stream);
        return;
    };

    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let mime = guess_mime(file_path.extension().and_then(|e| e.to_str()).unwrap_or(""));
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

/// Server-Sent Events long-poll: blocks until the generation counter changes,
/// then sends a typed SSE message (crepus-reload or crepus-error) and closes.
fn long_poll_sse(
    mut stream: TcpStream,
    generation: Arc<AtomicU64>,
    last_sse_msg: Arc<RwLock<String>>,
) {
    let headers =
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();

    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let start_gen = generation.load(Ordering::Acquire);

    loop {
        std::thread::sleep(Duration::from_millis(100));
        if generation.load(Ordering::Acquire) != start_gen {
            let msg = last_sse_msg
                .read()
                .map(|g| g.clone())
                .unwrap_or_else(|_| "data: reload\n\n".to_string());
            let msg = if msg.is_empty() {
                "data: reload\n\n".to_string()
            } else {
                msg
            };
            let _ = stream.write_all(msg.as_bytes());
            let _ = stream.flush();
            break;
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn guess_mime(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
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
    fn resolve_static_file_rejects_parent_segments() {
        let site = tempfile::tempdir().expect("tempdir");
        assert!(super::resolve_static_file(site.path(), "../secret.txt").is_none());
        assert!(super::resolve_static_file(site.path(), "static/../../etc/passwd").is_none());
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
