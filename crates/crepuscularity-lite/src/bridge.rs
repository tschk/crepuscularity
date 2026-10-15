//! Rust **native bridge**: plugins live in Rust; JS calls `Crepus.invoke(plugin, method, payloadJson)`.
//!
//! Threading: see `docs/THREADING.md` in the repo root.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::bench_plugin::BenchPlugin;
use crate::download_plugin::DownloadPlugin;
use crate::host::{HostSnapshot, HostState};
use crate::host_queue::{HostCommandQueue, HostDeferred};
use crate::inauguration_plugin::InaugurationPlugin;
use crate::plugins::{AppPlugin, ClipboardPlugin, CorePlugin, FsPlugin, HostPlugin, WindowPlugin};

/// Coarse sandbox knob: which plugin families are linked into this build / allowed at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Core,
    App,
    Fs,
    Clipboard,
    Window,
    Host,
    Download,
    /// Developer-only benchmark plugin (`"bench"`).  Off by default; opt in via
    /// `[capabilities] bench = true` in `crepus-lite.toml`.
    Bench,
    /// `in` toolchain helpers (`"inauguration"`). Off by default; opt in via
    /// `[capabilities] inauguration = true`.
    Inauguration,
}

/// Structured error returned inside the invoke envelope (`ok: false`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl BridgeError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            details: Some(details),
        }
    }
}

pub trait NativePlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn capability(&self) -> Capability;
    /// Allowlisted method names for this plugin (`invoke` rejects anything else).
    fn methods(&self) -> &'static [&'static str];
    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError>;
}

/// Registry + capability policy + host signals (e.g. app exit).
pub struct Bridge {
    plugins: HashMap<String, Arc<dyn NativePlugin>>,
    allowed: HashSet<Capability>,
    quit_requested: Arc<AtomicBool>,
    host_queue: Arc<HostCommandQueue>,
    host_state: Arc<HostState>,
    // ponytail: simple invoke counter prevents runaway calls
    invoke_count: AtomicU64,
    max_invocations: u64,
}

impl Bridge {
    /// Default production-style bridge: built-in plugins for all shipped capabilities.
    pub fn default_arc() -> Arc<Self> {
        let quit = Arc::new(AtomicBool::new(false));
        let host_queue = HostCommandQueue::new();
        let host_state = HostState::shared();
        let mut plugins: HashMap<String, Arc<dyn NativePlugin>> = HashMap::new();

        let core: Arc<dyn NativePlugin> = Arc::new(CorePlugin);
        plugins.insert(core.id().to_string(), core);
        let app: Arc<dyn NativePlugin> = Arc::new(AppPlugin::new(quit.clone()));
        plugins.insert(app.id().to_string(), app);
        let fs: Arc<dyn NativePlugin> = Arc::new(FsPlugin::new());
        plugins.insert(fs.id().to_string(), fs);
        let clip: Arc<dyn NativePlugin> = Arc::new(ClipboardPlugin);
        plugins.insert(clip.id().to_string(), clip);
        let win: Arc<dyn NativePlugin> = Arc::new(WindowPlugin::new(host_queue.clone()));
        plugins.insert(win.id().to_string(), win);
        let host: Arc<dyn NativePlugin> = Arc::new(HostPlugin::new(host_state.clone()));
        plugins.insert(host.id().to_string(), host);
        let download: Arc<dyn NativePlugin> = Arc::new(DownloadPlugin::new());
        plugins.insert(download.id().to_string(), download);

        let allowed = [
            Capability::Core,
            Capability::App,
            Capability::Fs,
            Capability::Clipboard,
            Capability::Window,
            Capability::Host,
            Capability::Download,
        ]
        .into_iter()
        .collect();

        Arc::new(Self {
            plugins,
            allowed,
            quit_requested: quit,
            host_queue,
            host_state,
            invoke_count: AtomicU64::new(0),
            max_invocations: 10_000,
        })
    }

    /// Minimal bridge for a **secondary isolate** (worker): only [`Capability::Core`] and [`Capability::App`].
    ///
    /// Workers must not call `fs`, `window`, or `clipboard` via `Crepus.invoke` until those plugins are
    /// audited for **concurrent** `invoke` from multiple threads. Use for CPU-only guest snippets.
    pub fn compute_only_bridge() -> Arc<Self> {
        Self::with_capabilities([Capability::Core, Capability::App].into_iter().collect())
    }

    /// `Bridge` with a subset of capabilities (plugins for disallowed caps are not registered).
    /// [`Capability::Core`] and [`Capability::App`] should normally stay enabled (see [`crate::config::CrepusLiteConfig::build_bridge`]).
    pub fn with_capabilities(allowed: HashSet<Capability>) -> Arc<Self> {
        let quit = Arc::new(AtomicBool::new(false));
        let host_queue = HostCommandQueue::new();
        let host_state = HostState::shared();
        let mut plugins: HashMap<String, Arc<dyn NativePlugin>> = HashMap::new();
        if allowed.contains(&Capability::Core) {
            let p: Arc<dyn NativePlugin> = Arc::new(CorePlugin);
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::App) {
            let p: Arc<dyn NativePlugin> = Arc::new(AppPlugin::new(quit.clone()));
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Fs) {
            let p: Arc<dyn NativePlugin> = Arc::new(FsPlugin::new());
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Clipboard) {
            let p: Arc<dyn NativePlugin> = Arc::new(ClipboardPlugin);
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Window) {
            let p: Arc<dyn NativePlugin> = Arc::new(WindowPlugin::new(host_queue.clone()));
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Host) {
            let p: Arc<dyn NativePlugin> = Arc::new(HostPlugin::new(host_state.clone()));
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Download) {
            let p: Arc<dyn NativePlugin> = Arc::new(DownloadPlugin::new());
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Bench) {
            let p: Arc<dyn NativePlugin> = Arc::new(BenchPlugin::new());
            plugins.insert(p.id().to_string(), p);
        }
        if allowed.contains(&Capability::Inauguration) {
            let p: Arc<dyn NativePlugin> = Arc::new(InaugurationPlugin::new());
            plugins.insert(p.id().to_string(), p);
        }
        Arc::new(Self {
            plugins,
            allowed,
            quit_requested: quit,
            host_queue,
            host_state,
            invoke_count: AtomicU64::new(0),
            max_invocations: 10_000,
        })
    }

    /// Dispatch a native call. Returns JSON envelope: `{ "ok": true, "data": ... }` or `{ "ok": false, "error": { code, message, details? } }`.
    pub fn invoke_envelope(&self, plugin_id: &str, method: &str, payload: &Value) -> Value {
        match self.invoke_inner(plugin_id, method, payload) {
            Ok(data) => json!({ "ok": true, "data": data }),
            Err(e) => json!({ "ok": false, "error": e }),
        }
    }

    fn invoke_inner(
        &self,
        plugin_id: &str,
        method: &str,
        payload: &Value,
    ) -> Result<Value, BridgeError> {
        // ponytail: rate limit check
        let count = self.invoke_count.fetch_add(1, Ordering::Relaxed);
        if count >= self.max_invocations {
            return Err(BridgeError::new("rate_limited", "too many invocations"));
        }
        // ponytail: cap JSON payload at 1 MB
        let payload_len = serde_json::to_string(payload).map(|s| s.len()).unwrap_or(0);
        if payload_len > 1_000_000 {
            return Err(BridgeError::new(
                "payload_too_large",
                format!("payload {} bytes exceeds 1 MB limit", payload_len),
            ));
        }
        let plugin = self.plugins.get(plugin_id).ok_or_else(|| {
            BridgeError::new(
                "unknown_plugin",
                format!("no plugin registered as {plugin_id:?}"),
            )
        })?;
        if !self.allowed.contains(&plugin.capability()) {
            return Err(BridgeError::new(
                "capability_denied",
                format!("capability {:?} is not enabled", plugin.capability()),
            ));
        }
        if !plugin.methods().contains(&method) {
            return Err(BridgeError::with_details(
                "unknown_method",
                format!("plugin {plugin_id:?} has no method {method:?}"),
                json!({ "plugin": plugin_id, "method": method }),
            ));
        }
        plugin.invoke(method, payload)
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested.load(Ordering::Acquire)
    }

    pub fn clear_quit_flag(&self) {
        self.quit_requested.store(false, Ordering::Release);
    }

    /// Consume UI-thread deferred commands (e.g. window title). Call from GPUI with a live [`gpui::Window`].
    pub fn drain_host_commands(&self) -> Vec<HostDeferred> {
        self.host_queue.drain()
    }

    pub fn host_snapshot(&self) -> HostSnapshot {
        self.host_state.snapshot()
    }

    pub fn host_state(&self) -> Arc<HostState> {
        self.host_state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn core_echo_envelope_ok() {
        let b = Bridge::default_arc();
        let v = b.invoke_envelope("core", "echo", &json!({"x": 1}));
        assert_eq!(v["ok"], true);
        assert_eq!(v["data"]["x"], 1);
    }

    #[test]
    fn unknown_method_envelope_err() {
        let b = Bridge::default_arc();
        let v = b.invoke_envelope("core", "nope", &json!({}));
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "unknown_method");
    }

    #[test]
    fn host_tree_and_storage_roundtrip() {
        let b = Bridge::default_arc();
        let rendered = b.invoke_envelope(
            "host",
            "renderTree",
            &json!({
                "tree": {
                    "type": "View",
                    "children": [{ "type": "Text", "text": "hello" }]
                }
            }),
        );
        assert_eq!(rendered["ok"], true);
        assert_eq!(rendered["data"]["render_count"], 1);

        let stored = b.invoke_envelope(
            "host",
            "storageSet",
            &json!({
                "key": "channel",
                "value": { "id": "123" }
            }),
        );
        assert_eq!(stored["ok"], true);

        let fetched = b.invoke_envelope("host", "storageGet", &json!({ "key": "channel" }));
        assert_eq!(fetched["ok"], true);
        assert_eq!(fetched["data"]["value"]["id"], "123");
    }

    #[test]
    fn test_compute_only_bridge() {
        let b = Bridge::compute_only_bridge();

        // Core capability should be allowed
        let v = b.invoke_envelope("core", "echo", &json!({"x": 42}));
        assert_eq!(v["ok"], true);
        assert_eq!(v["data"]["x"], 42);

        // App capability should be allowed
        let v = b.invoke_envelope("app", "exit", &json!({}));
        assert_eq!(v["ok"], true);
        assert!(b.quit_requested());

        // Other capabilities (e.g., Fs) should be denied
        // Note: fs plugin won't even be registered because compute_only_bridge only sets up Core and App
        let v = b.invoke_envelope("fs", "readTextFile", &json!({"path": "dummy.txt"}));
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "unknown_plugin");
    }
}
