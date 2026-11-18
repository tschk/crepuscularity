//! Built-in plugins registered on [`Bridge`](crate::bridge::Bridge).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::bridge::{BridgeError, Capability, NativePlugin};
use crate::clipboard;
use crate::fs_paths::resolve_under_sandbox;
use crate::host::{require_string_field, HostEventRecord, HostNode, HostRoute, HostState};
use crate::host_queue::{DeferredWindowDecorations, HostCommandQueue, HostDeferred};

fn entropy_u32() -> u32 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    (n ^ n.rotate_left(32) ^ n.rotate_right(17)) as u32
}

pub struct CorePlugin;

impl NativePlugin for CorePlugin {
    fn id(&self) -> &'static str {
        "core"
    }

    fn capability(&self) -> Capability {
        Capability::Core
    }

    fn methods(&self) -> &'static [&'static str] {
        &["echo", "ping", "timestamp", "randomU32"]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "echo" => Ok(payload.clone()),
            "ping" => Ok(json!({ "pong": true })),
            "timestamp" => {
                let dur = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                Ok(json!({
                    "unixMs": dur.as_millis(),
                    "unixNs": dur.as_nanos().to_string(),
                }))
            }
            "randomU32" => {
                let max = payload
                    .get("max")
                    .and_then(|v| v.as_u64())
                    .filter(|m| *m > 0)
                    .unwrap_or(u32::MAX as u64)
                    .min(u32::MAX as u64);
                let r = entropy_u32() as u64 % max;
                Ok(json!({ "max": max, "value": r }))
            }
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

pub struct AppPlugin {
    quit_requested: Arc<AtomicBool>,
}

impl AppPlugin {
    pub fn new(quit_requested: Arc<AtomicBool>) -> Self {
        Self { quit_requested }
    }
}

impl NativePlugin for AppPlugin {
    fn id(&self) -> &'static str {
        "app"
    }

    fn capability(&self) -> Capability {
        Capability::App
    }

    fn methods(&self) -> &'static [&'static str] {
        &["version", "platform", "documentsFolder", "openUrl", "exit"]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "version" => Ok(json!({
                "crepuscularity_lite": env!("CARGO_PKG_VERSION"),
                "plugin": "app",
            })),
            "platform" => Ok(json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "family": std::env::consts::FAMILY,
            })),
            "documentsFolder" => {
                let folder = dirs::document_dir()
                    .or_else(dirs::home_dir)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| ".".to_string());
                Ok(json!({ "path": folder }))
            }
            "openUrl" => {
                let url = payload.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                    BridgeError::with_details(
                        "invalid_payload",
                        "expected JSON object with string field \"url\"",
                        payload.clone(),
                    )
                })?;
                if !(url.starts_with("https://") || url.starts_with("http://")) {
                    return Err(BridgeError::new(
                        "invalid_url",
                        "only http(s): URLs are allowed",
                    ));
                }
                open::that(url).map_err(|e| BridgeError::new("open_failed", format!("{e}")))?;
                Ok(json!({ "opened": url }))
            }
            "exit" => {
                self.quit_requested.store(true, Ordering::Release);
                Ok(json!({ "requested": "application_exit" }))
            }
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

/// Sandboxed files under the OS app-data directory (`…/crepuscularity-lite/sandbox`).
pub struct FsPlugin {
    root: std::path::PathBuf,
}

impl FsPlugin {
    pub fn new() -> Self {
        let root = dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("crepuscularity-lite")
            .join("sandbox");
        let _ = std::fs::create_dir_all(&root);
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Self { root }
    }

    fn resolve(&self, rel: &str) -> Result<std::path::PathBuf, BridgeError> {
        resolve_under_sandbox(&self.root, rel)
    }
}

impl Default for FsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePlugin for FsPlugin {
    fn id(&self) -> &'static str {
        "fs"
    }

    fn capability(&self) -> Capability {
        Capability::Fs
    }

    fn methods(&self) -> &'static [&'static str] {
        &[
            "readText",
            "writeText",
            "list",
            "mkdir",
            "removeFile",
            "removeDir",
            "exists",
            "stat",
        ]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        let path = || {
            payload.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                BridgeError::with_details(
                    "invalid_payload",
                    "expected string field \"path\" (relative to sandbox)",
                    payload.clone(),
                )
            })
        };
        match method {
            "readText" => self.handle_read_text(path()?),
            "writeText" => self.handle_write_text(path()?, payload),
            "list" => {
                let rel = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                self.handle_list(rel)
            }
            "mkdir" => self.handle_mkdir(path()?),
            "removeFile" => self.handle_remove_file(path()?),
            "removeDir" => self.handle_remove_dir(path()?),
            "exists" => self.handle_exists(path()?),
            "stat" => self.handle_stat(path()?),
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

impl FsPlugin {
    fn handle_read_text(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        let text = std::fs::read_to_string(&p)
            .map_err(|e| BridgeError::new("fs_error", format!("read failed: {e}")))?;
        Ok(json!({ "path": rel, "text": text }))
    }

    fn handle_write_text(&self, rel: &str, payload: &Value) -> Result<Value, BridgeError> {
        let content = payload
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BridgeError::with_details(
                    "invalid_payload",
                    "writeText requires string field \"content\"",
                    payload.clone(),
                )
            })?;
        let p = self.resolve(rel)?;
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BridgeError::new("fs_error", format!("create_dir_all: {e}")))?;
        }
        std::fs::write(&p, content.as_bytes())
            .map_err(|e| BridgeError::new("fs_error", format!("write failed: {e}")))?;
        Ok(json!({ "path": rel, "bytes": content.len() }))
    }

    fn handle_list(&self, rel: &str) -> Result<Value, BridgeError> {
        let dir = self.resolve(rel)?;
        let read = std::fs::read_dir(&dir)
            .map_err(|e| BridgeError::new("fs_error", format!("list failed: {e}")))?;
        let mut entries = Vec::new();
        for e in read {
            let e =
                e.map_err(|err| BridgeError::new("fs_error", format!("read_dir entry: {err}")))?;
            let t = e
                .file_type()
                .map_err(|err| BridgeError::new("fs_error", format!("file_type: {err}")))?;
            entries.push(json!({
                "name": e.file_name().to_string_lossy(),
                "isDir": t.is_dir(),
            }));
        }
        Ok(json!({ "path": rel, "entries": entries }))
    }

    fn handle_mkdir(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        std::fs::create_dir_all(&p)
            .map_err(|e| BridgeError::new("fs_error", format!("mkdir: {e}")))?;
        Ok(json!({ "path": rel }))
    }

    fn handle_remove_file(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        if p.is_dir() {
            return Err(BridgeError::new(
                "fs_error",
                "removeFile only deletes files; use removeDir for directories",
            ));
        }
        std::fs::remove_file(&p)
            .map_err(|e| BridgeError::new("fs_error", format!("remove_file: {e}")))?;
        Ok(json!({ "path": rel }))
    }

    fn handle_remove_dir(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        if !p.is_dir() {
            return Err(BridgeError::new(
                "fs_error",
                "removeDir expects a directory path",
            ));
        }
        std::fs::remove_dir_all(&p)
            .map_err(|e| BridgeError::new("fs_error", format!("remove_dir_all: {e}")))?;
        Ok(json!({ "path": rel }))
    }

    fn handle_exists(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        Ok(json!({ "path": rel, "exists": p.exists() }))
    }

    fn handle_stat(&self, rel: &str) -> Result<Value, BridgeError> {
        let p = self.resolve(rel)?;
        let meta = std::fs::metadata(&p)
            .map_err(|e| BridgeError::new("fs_error", format!("metadata failed: {e}")))?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis());
        Ok(json!({
            "path": rel,
            "isFile": meta.is_file(),
            "isDir": meta.is_dir(),
            "len": meta.len(),
            "modifiedMs": modified_ms,
        }))
    }
}

pub struct ClipboardPlugin;

impl NativePlugin for ClipboardPlugin {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn capability(&self) -> Capability {
        Capability::Clipboard
    }

    fn methods(&self) -> &'static [&'static str] {
        &["readText", "writeText"]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "readText" => {
                let text =
                    clipboard::read_text().map_err(|e| BridgeError::new("clipboard_error", e))?;
                Ok(json!({ "text": text }))
            }
            "writeText" => {
                let text = payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BridgeError::with_details(
                            "invalid_payload",
                            "expected string field \"text\"",
                            payload.clone(),
                        )
                    })?;
                clipboard::write_text(text).map_err(|e| BridgeError::new("clipboard_error", e))?;
                Ok(json!({ "written": true, "chars": text.len() }))
            }
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

pub struct WindowPlugin {
    queue: Arc<HostCommandQueue>,
}

impl WindowPlugin {
    pub fn new(queue: Arc<HostCommandQueue>) -> Self {
        Self { queue }
    }
}

pub struct HostPlugin {
    state: Arc<HostState>,
}

impl HostPlugin {
    pub fn new(state: Arc<HostState>) -> Self {
        Self { state }
    }
}

impl NativePlugin for HostPlugin {
    fn id(&self) -> &'static str {
        "host"
    }

    fn capability(&self) -> Capability {
        Capability::Host
    }

    fn methods(&self) -> &'static [&'static str] {
        &[
            "renderTree",
            "snapshot",
            "storageGet",
            "storageSet",
            "storageRemove",
            "navigationPush",
            "navigationReplace",
            "navigationPop",
            "recordEvent",
            "channelSend",
            "channelPoll",
        ]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "renderTree" => {
                let tree_value = payload.get("tree").cloned().ok_or_else(|| {
                    BridgeError::with_details(
                        "invalid_payload",
                        "renderTree requires object field \"tree\"",
                        payload.clone(),
                    )
                })?;
                let tree: HostNode = serde_json::from_value(tree_value).map_err(|err| {
                    BridgeError::new("invalid_payload", format!("invalid host tree: {err}"))
                })?;
                Ok(json!(self.state.render_tree(tree)))
            }
            "snapshot" => Ok(json!(self.state.snapshot())),
            "storageGet" => {
                let key = require_string_field(payload, "key")?;
                Ok(json!({ "key": key, "value": self.state.storage_get(key) }))
            }
            "storageSet" => {
                let key = require_string_field(payload, "key")?;
                let value = payload.get("value").cloned().unwrap_or(Value::Null);
                Ok(json!(self.state.storage_set(key, value)))
            }
            "storageRemove" => {
                let key = require_string_field(payload, "key")?;
                Ok(json!(self.state.storage_remove(key)))
            }
            "navigationPush" => {
                let name = require_string_field(payload, "name")?;
                let params = payload.get("params").cloned().unwrap_or_else(|| json!({}));
                Ok(json!(self.state.navigation_push(HostRoute {
                    name: name.to_string(),
                    params,
                })))
            }
            "navigationReplace" => {
                let name = require_string_field(payload, "name")?;
                let params = payload.get("params").cloned().unwrap_or_else(|| json!({}));
                Ok(json!(self.state.navigation_replace(HostRoute {
                    name: name.to_string(),
                    params,
                })))
            }
            "navigationPop" => Ok(json!(self.state.navigation_pop())),
            "recordEvent" => {
                let kind = require_string_field(payload, "kind")?;
                let handler_id = payload
                    .get("handlerId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.state.record_event(HostEventRecord {
                    kind: kind.to_string(),
                    handler_id: handler_id.to_string(),
                    payload: payload.get("payload").cloned().unwrap_or(Value::Null),
                });
                Ok(json!(self.state.snapshot()))
            }
            "channelSend" => {
                let channel = require_string_field(payload, "channel")?;
                let message = payload.get("message").cloned().unwrap_or(Value::Null);
                Ok(json!(self.state.channel_send(channel.to_string(), message)))
            }
            "channelPoll" => {
                let channel = require_string_field(payload, "channel")?;
                Ok(json!({
                    "channel": channel,
                    "message": self.state.channel_poll(channel),
                }))
            }
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

impl NativePlugin for WindowPlugin {
    fn id(&self) -> &'static str {
        "window"
    }

    fn capability(&self) -> Capability {
        Capability::Window
    }

    fn methods(&self) -> &'static [&'static str] {
        &[
            "setTitle",
            "setContentSize",
            "toggleFullscreen",
            "minimize",
            "zoom",
            "showCharacterPalette",
            "setDocumentEdited",
            "startWindowMove",
            "refresh",
            "requestDecorations",
        ]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "setTitle" => {
                let title = payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BridgeError::with_details(
                            "invalid_payload",
                            "expected string field \"title\"",
                            payload.clone(),
                        )
                    })?;
                self.queue
                    .push(HostDeferred::SetWindowTitle(title.to_string()));
                Ok(json!({ "scheduled": "setTitle" }))
            }
            "setContentSize" => {
                let w = payload
                    .get("width")
                    .and_then(|v| v.as_f64())
                    .filter(|x| *x > 0.0);
                let h = payload
                    .get("height")
                    .and_then(|v| v.as_f64())
                    .filter(|x| *x > 0.0);
                let (width, height) = match (w, h) {
                    (Some(width), Some(height)) => (width as f32, height as f32),
                    _ => {
                        return Err(BridgeError::with_details(
                            "invalid_payload",
                            "setContentSize requires positive numeric \"width\" and \"height\" (logical px)",
                            payload.clone(),
                        ));
                    }
                };
                self.queue
                    .push(HostDeferred::SetContentSize { width, height });
                Ok(json!({ "scheduled": "setContentSize", "width": width, "height": height }))
            }
            "toggleFullscreen" => {
                self.queue.push(HostDeferred::ToggleFullscreen);
                Ok(json!({ "scheduled": "toggleFullscreen" }))
            }
            "minimize" => {
                self.queue.push(HostDeferred::Minimize);
                Ok(json!({ "scheduled": "minimize" }))
            }
            "zoom" => {
                self.queue.push(HostDeferred::Zoom);
                Ok(json!({ "scheduled": "zoom" }))
            }
            "showCharacterPalette" => {
                self.queue.push(HostDeferred::ShowCharacterPalette);
                Ok(json!({ "scheduled": "showCharacterPalette" }))
            }
            "setDocumentEdited" => {
                let edited = payload
                    .get("edited")
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| {
                        BridgeError::with_details(
                            "invalid_payload",
                            "setDocumentEdited requires boolean field \"edited\"",
                            payload.clone(),
                        )
                    })?;
                self.queue.push(HostDeferred::SetWindowEdited(edited));
                Ok(json!({ "scheduled": "setDocumentEdited", "edited": edited }))
            }
            "startWindowMove" => {
                self.queue.push(HostDeferred::StartWindowMove);
                Ok(json!({ "scheduled": "startWindowMove" }))
            }
            "refresh" => {
                self.queue.push(HostDeferred::RefreshWindow);
                Ok(json!({ "scheduled": "refresh" }))
            }
            "requestDecorations" => {
                let style = payload
                    .get("style")
                    .and_then(|v| v.as_str())
                    .unwrap_or("server");
                let dec = match style {
                    "client" => DeferredWindowDecorations::Client,
                    _ => DeferredWindowDecorations::Server,
                };
                self.queue.push(HostDeferred::RequestDecorations(dec));
                Ok(json!({ "scheduled": "requestDecorations", "style": style }))
            }
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}
