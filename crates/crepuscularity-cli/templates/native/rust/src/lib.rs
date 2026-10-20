use std::borrow::Cow;
use std::ffi::CStr;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::os::raw::c_char;
#[cfg(feature = "filesystem")]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

#[cfg(target_os = "android")]
use jni::objects::{JClass, JString};
#[cfg(target_os = "android")]
use jni::JNIEnv;

#[derive(Default)]
struct MobileActionState {
    sync_count: u64,
    preview_count: u64,
    last_action: String,
    last_payload: Option<String>,
}

#[derive(Default)]
struct ResultMetadata {
    last_result: String,
    last_error: Option<String>,
}

#[derive(Clone, Copy)]
enum ActionKind {
    Named(ActionHandler),
    Plugin,
}

type ActionHandler =
    fn(&mut MobileActionState, Option<&serde_json::Value>) -> Result<serde_json::Value, String>;

static ACTION_STATE: OnceLock<Mutex<MobileActionState>> = OnceLock::new();
static RESULT_METADATA: OnceLock<Mutex<ResultMetadata>> = OnceLock::new();
static VIEW_STATE: OnceLock<Mutex<serde_json::Value>> = OnceLock::new();

fn action_state() -> &'static Mutex<MobileActionState> {
    ACTION_STATE.get_or_init(|| Mutex::new(MobileActionState::default()))
}

fn view_state() -> &'static Mutex<serde_json::Value> {
    VIEW_STATE.get_or_init(|| Mutex::new(serde_json::json!({})))
}

fn lock_action_state() -> MutexGuard<'static, MobileActionState> {
    action_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock_result_metadata() -> MutexGuard<'static, ResultMetadata> {
    RESULT_METADATA
        .get_or_init(|| Mutex::new(ResultMetadata::default()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock_view_state() -> MutexGuard<'static, serde_json::Value> {
    view_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn dispatch_action(action: &str) -> bool {
    action_kind(action).is_some()
}

pub fn dispatch_action_json(action: &str) -> String {
    let request = parse_action_request(action);
    let Some(kind) = action_kind(action) else {
        let state = lock_action_state();
        let action_name = legacy_action_name(&request).unwrap_or(action);
        return action_json(false, action_name, None, Some("unknown action"), &state);
    };
    let mut state = lock_action_state();
    let action_name = request_action_name(&request);
    state.last_action = action_name.clone();
    state.last_payload = request_payload(&request).map(serde_json::Value::to_string);
    let result = match kind {
        ActionKind::Named(handler) => handler(&mut state, request_payload(&request)),
        ActionKind::Plugin => plugin_action(&mut state, &request),
    };
    let result = match result {
        Ok(value) => action_json(true, &action_name, Some(value), None, &state),
        Err(error) => action_json(false, &action_name, None, Some(&error), &state),
    };
    let mut metadata = lock_result_metadata();
    metadata.last_error = parse_action_error(&result);
    metadata.last_result = result.clone();
    drop(metadata);
    drop(state);
    result
}

fn dispatch_change_json(action: &str, bind: &str, value_json: &str) -> String {
    let value = match serde_json::from_str::<serde_json::Value>(value_json) {
        Ok(value) => value,
        Err(error) => {
            let state = lock_action_state();
            return action_json(
                false,
                action,
                None,
                Some(&format!("invalid change payload: {error}")),
                &state,
            );
        }
    };
    if action.is_empty() {
        return merge_view_value(bind, value).to_string();
    }
    let mut payload = serde_json::Map::new();
    payload.insert(bind.to_string(), value.clone());
    let request = if let Some((capability, method)) = action.split_once('.') {
        serde_json::json!({
            "kind": "plugin",
            "capability": capability,
            "method": method,
            "payload": payload,
        })
    } else {
        serde_json::json!({ "action": action, "payload": payload })
    };
    let raw = dispatch_action_json(&request.to_string());
    let Ok(mut result) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return raw;
    };
    if result.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        if request_kind(&request) == Some("plugin") {
            result["bindings"] = serde_json::json!({ bind: value });
        } else if let Some(fields) = result
            .get_mut("value")
            .and_then(serde_json::Value::as_object_mut)
        {
            fields.entry(bind.to_string()).or_insert(value);
        }
    }
    result.to_string()
}

fn write_binding_path(
    root: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    // Leave two nesting levels for the result envelope and its state object.
    if path.split('.').take(127).count() > 126 {
        return Err("binding path exceeds 126 segments".to_string());
    }
    let mut segments = path.split('.').peekable();
    let mut current = root;
    while let Some(segment) = segments.next() {
        let last = segments.peek().is_none();
        match current {
            serde_json::Value::Object(map) => {
                if last {
                    map.insert(segment.to_string(), value);
                    return Ok(());
                }
                current = map
                    .entry(segment.to_string())
                    .or_insert_with(|| serde_json::json!({}));
            }
            serde_json::Value::Array(items) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| format!("invalid array index in binding path: {path}"))?;
                current = items
                    .get_mut(index)
                    .ok_or_else(|| format!("array index out of bounds in binding path: {path}"))?;
                if last {
                    *current = value;
                    return Ok(());
                }
            }
            _ => return Err(format!("cannot traverse scalar in binding path: {path}")),
        }
    }
    Ok(())
}

fn merge_view_value(bind: &str, value: serde_json::Value) -> serde_json::Value {
    let mut state = lock_view_state();
    let mut updated = if state.is_object() {
        state.clone()
    } else {
        serde_json::json!({})
    };
    if let Err(error) = write_binding_path(&mut updated, bind, value.clone()) {
        return serde_json::json!({ "ok": false, "action": "bind", "error": error });
    }
    *state = updated;
    serde_json::json!({
        "ok": true,
        "action": "bind",
        "value": { bind: value },
        "state": state.clone(),
    })
}

fn parse_action_error(result: &str) -> Option<String> {
    let payload = serde_json::from_str::<serde_json::Value>(result).ok()?;
    action_error_from_value(&payload, result)
}

fn action_error_from_value(payload: &serde_json::Value, result: &str) -> Option<String> {
    if payload.get("ok").and_then(|value| value.as_bool()) == Some(false) {
        return Some(
            payload
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or(result)
                .to_string(),
        );
    }
    None
}

fn action_kind(action: &str) -> Option<ActionKind> {
    let request = parse_action_request(action);
    if request_kind(&request) == Some("plugin") {
        return Some(ActionKind::Plugin);
    }
    match legacy_action_name(&request).unwrap_or(action) {
        "sync" => Some(ActionKind::Named(sync_action)),
        "preview" => Some(ActionKind::Named(preview_action)),
        _ => None,
    }
}

fn sync_action(
    state: &mut MobileActionState,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    state.sync_count += 1;
    Ok(serde_json::json!({
        "message": payload_message(payload).unwrap_or("synced"),
        "syncCount": state.sync_count,
    }))
}

fn preview_action(
    state: &mut MobileActionState,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    state.preview_count += 1;
    Ok(serde_json::json!({
        "message": payload_message(payload).unwrap_or("previewed"),
        "previewCount": state.preview_count,
    }))
}

fn plugin_action(
    state: &mut MobileActionState,
    request: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let capability = request
        .get("capability")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "plugin request missing capability".to_string())?;
    let method = request
        .get("method")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "plugin request missing method".to_string())?;
    let payload = request.get("payload");
    let value = match capability {
        "preferences" => preferences_backend(method, payload)?,
        #[cfg(feature = "filesystem")]
        "filesystem" => filesystem_backend(method, payload)?,
        "device" => device_backend(method)?,
        "app" => app_backend(state, method)?,
        "network" => network_backend(method, payload)?,
        "http" => http_backend(method, payload)?,
        other => return Err(format!("unsupported capability: {other}")),
    };
    Ok(serde_json::json!({
        "capability": capability,
        "method": method,
        "value": value,
    }))
}

fn preferences_backend(
    method: &str,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let mut prefs = load_json_map(&preferences_file_path())?;
    match method {
        "get" => {
            let key = payload_string(payload, "key")?;
            Ok(prefs.get(&key).cloned().unwrap_or(serde_json::Value::Null))
        }
        "set" => {
            let key = payload_string(payload, "key")?;
            let value = payload
                .and_then(|value| value.get("value"))
                .cloned()
                .ok_or_else(|| "preferences.set requires payload.value".to_string())?;
            prefs.insert(key.clone(), value.clone());
            save_json_map(&preferences_file_path(), &prefs)?;
            Ok(serde_json::json!({ "key": key, "value": value }))
        }
        "remove" => {
            let key = payload_string(payload, "key")?;
            let removed = prefs.remove(&key).is_some();
            save_json_map(&preferences_file_path(), &prefs)?;
            Ok(serde_json::json!({ "key": key, "removed": removed }))
        }
        "keys" => {
            let mut keys = prefs.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            Ok(serde_json::json!(keys))
        }
        "clear" => {
            prefs.clear();
            save_json_map(&preferences_file_path(), &prefs)?;
            Ok(serde_json::json!({ "cleared": true }))
        }
        other => Err(format!("unsupported preferences method: {other}")),
    }
}

#[cfg(feature = "filesystem")]
fn filesystem_backend(
    method: &str,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    match method {
        "readText" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            Ok(serde_json::json!({ "path": path_string(&path), "text": text }))
        }
        "writeText" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            let text = payload_string(payload, "text")?;
            ensure_parent_dir(&path)?;
            fs::write(&path, text.as_bytes())
                .map_err(|error| format!("write {}: {error}", path.display()))?;
            Ok(file_stat_json(&path)?)
        }
        "delete" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            let deleted = if path.is_dir() {
                fs::remove_dir_all(&path).is_ok()
            } else {
                fs::remove_file(&path).is_ok()
            };
            Ok(serde_json::json!({ "path": path_string(&path), "deleted": deleted }))
        }
        "mkdir" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            fs::create_dir_all(&path)
                .map_err(|error| format!("mkdir {}: {error}", path.display()))?;
            Ok(file_stat_json(&path)?)
        }
        "list" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            let mut entries = fs::read_dir(&path)
                .map_err(|error| format!("list {}: {error}", path.display()))?
                .map(|entry| {
                    let entry = entry.map_err(|error| error.to_string())?;
                    let entry_path = entry.path();
                    let metadata = entry
                        .metadata()
                        .map_err(|error| format!("stat {}: {error}", entry_path.display()))?;
                    Ok(serde_json::json!({
                        "name": entry.file_name().to_string_lossy().to_string(),
                        "path": path_string(&entry_path),
                        "isDir": metadata.is_dir(),
                        "bytes": metadata.len(),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            entries.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
            Ok(serde_json::json!({ "path": path_string(&path), "entries": entries }))
        }
        "stat" => {
            let path = scoped_data_path(&payload_string(payload, "path")?)?;
            file_stat_json(&path)
        }
        other => Err(format!("unsupported filesystem method: {other}")),
    }
}

fn device_backend(method: &str) -> Result<serde_json::Value, String> {
    match method {
        "info" => Ok(serde_json::json!({
            "targetOs": std::env::consts::OS,
            "targetArch": std::env::consts::ARCH,
            "targetFamily": std::env::consts::FAMILY,
            "tempDir": path_string(&data_root_dir()),
        })),
        other => Err(format!("unsupported device method: {other}")),
    }
}

fn app_backend(state: &MobileActionState, method: &str) -> Result<serde_json::Value, String> {
    match method {
        "info" => Ok(serde_json::json!({
            "syncCount": state.sync_count,
            "previewCount": state.preview_count,
            "lastAction": state.last_action,
            "lastPayload": state.last_payload,
            "dataRoot": path_string(&data_root_dir()),
        })),
        other => Err(format!("unsupported app method: {other}")),
    }
}

fn network_backend(
    method: &str,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    match method {
        "status" => {
            let host = payload
                .and_then(|value| value.get("host"))
                .and_then(|value| value.as_str())
                .unwrap_or("1.1.1.1");
            let port = payload
                .and_then(|value| value.get("port"))
                .and_then(|value| value.as_u64())
                .unwrap_or(53) as u16;
            let timeout_ms = payload
                .and_then(|value| value.get("timeoutMs"))
                .and_then(|value| value.as_u64())
                .unwrap_or(1000);
            let reachable = probe_socket(host, port, timeout_ms);
            Ok(serde_json::json!({
                "host": host,
                "port": port,
                "reachable": reachable,
            }))
        }
        other => Err(format!("unsupported network method: {other}")),
    }
}

fn http_backend(
    method: &str,
    payload: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let payload = payload.ok_or_else(|| "http request requires payload".to_string())?;
    let url = payload_string(Some(payload), "url")?;
    let verb = match method {
        "get" => "GET",
        "post" => "POST",
        "put" => "PUT",
        "delete" => "DELETE",
        "request" => payload
            .get("verb")
            .and_then(|value| value.as_str())
            .unwrap_or("GET"),
        other => return Err(format!("unsupported http method: {other}")),
    };
    let parsed = parse_http_url(&url)?;
    let body = payload
        .get("body")
        .map(|body| match body {
            serde_json::Value::String(text) => Ok(text.clone()),
            other => serde_json::to_string(&other).map_err(|error| error.to_string()),
        })
        .transpose()?
        .unwrap_or_default();
    let response = send_http_request(verb, &parsed, payload.get("headers"), &body)?;
    Ok(serde_json::json!({
        "status": response.status,
        "ok": (200..=299).contains(&response.status),
        "text": response.body,
    }))
}

fn payload_message(payload: Option<&serde_json::Value>) -> Option<&str> {
    payload
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
}

fn parse_action_request(input: &str) -> serde_json::Value {
    serde_json::from_str(input)
        .ok()
        .filter(|value: &serde_json::Value| {
            value.get("action").is_some() || value.get("kind").is_some()
        })
        .unwrap_or_else(|| {
            if let Some((capability, method)) = input.split_once('.') {
                serde_json::json!({
                    "kind": "plugin",
                    "capability": capability,
                    "method": method,
                })
            } else {
                serde_json::json!({ "action": input })
            }
        })
}

fn legacy_action_name(request: &serde_json::Value) -> Option<&str> {
    request.get("action").and_then(|value| value.as_str())
}

fn request_kind(request: &serde_json::Value) -> Option<&str> {
    request.get("kind").and_then(|value| value.as_str())
}

fn request_action_name(request: &serde_json::Value) -> String {
    if let Some(action) = legacy_action_name(request) {
        return action.to_string();
    }
    if request_kind(request) == Some("plugin") {
        let capability = request
            .get("capability")
            .and_then(|value| value.as_str())
            .unwrap_or("plugin");
        let method = request
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or("call");
        return format!("{capability}.{method}");
    }
    request_kind(request).unwrap_or("").to_string()
}

fn request_payload(request: &serde_json::Value) -> Option<&serde_json::Value> {
    request.get("payload")
}

fn payload_string(payload: Option<&serde_json::Value>, key: &str) -> Result<String, String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("payload.{key} must be a string"))
}

fn data_root_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("CREPUS_NATIVE_STATE_DIR") {
        return PathBuf::from(path);
    }
    std::env::temp_dir().join("crepus_mobile_actions")
}

fn probe_socket(host: &str, port: u16, timeout_ms: u64) -> bool {
    resolve_socket_addrs(host, port).into_iter().any(|address| {
        TcpStream::connect_timeout(&address, Duration::from_millis(timeout_ms)).is_ok()
    })
}

fn resolve_socket_addrs(host: &str, port: u16) -> Vec<SocketAddr> {
    (host, port)
        .to_socket_addrs()
        .map(|addresses| addresses.collect())
        .unwrap_or_default()
}

struct ParsedHttpUrl {
    host: String,
    port: u16,
    path: String,
}

struct HttpResponse {
    status: u16,
    body: String,
}

fn parse_http_url(url: &str) -> Result<ParsedHttpUrl, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// URLs are supported".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{}", path)),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(']') => {
            let port = port
                .parse::<u16>()
                .map_err(|_| format!("invalid port in URL: {url}"))?;
            (host.to_string(), port)
        }
        _ => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return Err("URL host must not be empty".to_string());
    }
    Ok(ParsedHttpUrl { host, port, path })
}

fn send_http_request(
    verb: &str,
    url: &ParsedHttpUrl,
    headers: Option<&serde_json::Value>,
    body: &str,
) -> Result<HttpResponse, String> {
    let address = resolve_socket_addrs(&url.host, url.port)
        .into_iter()
        .next()
        .ok_or_else(|| format!("failed to resolve {}:{}", url.host, url.port))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|error| format!("connect {}: {error}", address))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;

    let mut request = format!(
        "{verb} {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n",
        url.path, url.host, url.port
    );
    if let Some(headers) = headers.and_then(|value| value.as_object()) {
        for (key, value) in headers {
            let Some(value) = value.as_str() else {
                return Err(format!("header {key} must be a string"));
            };
            request.push_str(&format!("{key}: {value}\r\n"));
        }
    }
    if !body.is_empty() {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("write request: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("read response: {error}"))?;
    parse_http_response(&response)
}

fn parse_http_response(response: &str) -> Result<HttpResponse, String> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid HTTP response".to_string())?;
    let mut lines = head.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "missing status line".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "missing status code".to_string())?
        .parse::<u16>()
        .map_err(|_| "invalid status code".to_string())?;
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

fn preferences_file_path() -> PathBuf {
    data_root_dir().join("preferences.json")
}

fn load_json_map(path: &Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }
    let text =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{} is not a JSON object", path.display()))
}

fn save_json_map(
    path: &Path,
    map: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let json = serde_json::to_vec_pretty(map).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| format!("write {}: {error}", path.display()))
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|error| format!("mkdir {}: {error}", parent.display()))
}

#[cfg(feature = "filesystem")]
fn scoped_data_path(raw: &str) -> Result<PathBuf, String> {
    let relative = sanitize_relative_path(raw)?;
    Ok(data_root_dir().join(relative))
}

#[cfg(feature = "filesystem")]
fn sanitize_relative_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err("absolute paths are not allowed".to_string());
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err("parent segments are not allowed".to_string()),
            _ => return Err("unsupported path component".to_string()),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err("path must not be empty".to_string());
    }
    Ok(clean)
}

#[cfg(feature = "filesystem")]
fn file_stat_json(path: &Path) -> Result<serde_json::Value, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("stat {}: {error}", path.display()))?;
    Ok(serde_json::json!({
        "path": path_string(path),
        "isDir": metadata.is_dir(),
        "bytes": metadata.len(),
    }))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn action_json(
    ok: bool,
    action: &str,
    value: Option<serde_json::Value>,
    error: Option<&str>,
    state: &MobileActionState,
) -> String {
    let mut out = serde_json::json!({
        "ok": ok,
        "action": action,
        "state": {
            "syncCount": state.sync_count,
            "previewCount": state.preview_count,
            "lastAction": state.last_action,
            "lastPayload": state.last_payload,
        }
    });
    if let Some(value) = value {
        out["value"] = value;
    }
    if let Some(error) = error {
        out["error"] = serde_json::Value::String(error.to_string());
    }
    out.to_string()
}

#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch(action_ptr: *const c_char, action_len: usize) -> bool {
    action_from_ffi(action_ptr, action_len)
        .map(|action| dispatch_action(action.as_ref()))
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch_json(
    action_ptr: *const c_char,
    action_len: usize,
    output_ptr: *mut c_char,
    output_len: usize,
) -> usize {
    copy_json_to_output(
        &dispatch_json_from_ffi(action_ptr, action_len),
        output_ptr,
        output_len,
    )
}

fn dispatch_json_from_ffi(action_ptr: *const c_char, action_len: usize) -> String {
    action_from_ffi(action_ptr, action_len)
        .map(|action| dispatch_action_json(action.as_ref()))
        .unwrap_or_else(|| {
            action_json(
                false,
                "",
                None,
                Some("invalid action pointer"),
                &MobileActionState::default(),
            )
        })
}

/// Execute once and return the complete JSON; release with crepus_mobile_free_string.
#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch_json_owned(
    action_ptr: *const c_char,
    action_len: usize,
) -> *mut c_char {
    alloc_c_string(&dispatch_json_from_ffi(action_ptr, action_len))
}

#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch_and_store_json(
    action_ptr: *const c_char,
    action_len: usize,
    output_ptr: *mut c_char,
    output_len: usize,
) -> usize {
    crepus_mobile_dispatch_json(action_ptr, action_len, output_ptr, output_len)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch_change_json(
    action_ptr: *const c_char,
    action_len: usize,
    bind_ptr: *const c_char,
    bind_len: usize,
    value_ptr: *const c_char,
    value_len: usize,
    output_ptr: *mut c_char,
    output_len: usize,
) -> usize {
    copy_json_to_output(
        &dispatch_change_from_ffi(
            action_ptr, action_len, bind_ptr, bind_len, value_ptr, value_len,
        ),
        output_ptr,
        output_len,
    )
}

fn dispatch_change_from_ffi(
    action_ptr: *const c_char,
    action_len: usize,
    bind_ptr: *const c_char,
    bind_len: usize,
    value_ptr: *const c_char,
    value_len: usize,
) -> String {
    let Some(action) = action_from_ffi(action_ptr, action_len) else {
        let result = action_json(
            false,
            "",
            None,
            Some("invalid action pointer"),
            &MobileActionState::default(),
        );
        return result;
    };
    let Some(bind) = action_from_ffi(bind_ptr, bind_len) else {
        let result = action_json(
            false,
            action.as_ref(),
            None,
            Some("invalid bind pointer"),
            &MobileActionState::default(),
        );
        return result;
    };
    let Some(value_json) = action_from_ffi(value_ptr, value_len) else {
        let result = action_json(
            false,
            action.as_ref(),
            None,
            Some("invalid value pointer"),
            &MobileActionState::default(),
        );
        return result;
    };
    dispatch_change_json(action.as_ref(), bind.as_ref(), value_json.as_ref())
}

/// Execute once and return the complete JSON; release with crepus_mobile_free_string.
#[no_mangle]
pub extern "C" fn crepus_mobile_dispatch_change_json_owned(
    action_ptr: *const c_char,
    action_len: usize,
    bind_ptr: *const c_char,
    bind_len: usize,
    value_ptr: *const c_char,
    value_len: usize,
) -> *mut c_char {
    alloc_c_string(&dispatch_change_from_ffi(
        action_ptr, action_len, bind_ptr, bind_len, value_ptr, value_len,
    ))
}

#[no_mangle]
pub extern "C" fn crepus_mobile_last_result() -> *mut c_char {
    let state = lock_result_metadata();
    alloc_c_string(&state.last_result)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_last_error() -> *mut c_char {
    let state = lock_result_metadata();
    alloc_c_string(state.last_error.as_deref().unwrap_or(""))
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn crepus_mobile_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        libc::free(ptr.cast());
    }
}

fn alloc_c_string(result: &str) -> *mut c_char {
    let bytes = result.as_bytes();
    let ptr = unsafe { libc::malloc(bytes.len() + 1) as *mut c_char };
    if ptr.is_null() {
        return ptr;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
        *ptr.add(bytes.len()) = 0;
    }
    ptr
}

fn action_from_ffi(action_ptr: *const c_char, action_len: usize) -> Option<Cow<'static, str>> {
    if action_ptr.is_null() {
        return None;
    }
    Some(
        if action_len == 0 {
            unsafe { CStr::from_ptr(action_ptr) }.to_string_lossy()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(action_ptr.cast::<u8>(), action_len) };
            String::from_utf8_lossy(bytes)
        }
        .into_owned()
        .into(),
    )
}

fn copy_json_to_output(result: &str, output_ptr: *mut c_char, output_len: usize) -> usize {
    let bytes = result.as_bytes();
    if output_ptr.is_null() || output_len == 0 {
        return bytes.len();
    }
    let copy_len = bytes.len().min(output_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), output_ptr.cast::<u8>(), copy_len);
        *output_ptr.add(copy_len) = 0;
    }
    bytes.len()
}

fn store_result_json(raw: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => {
            let mut state = lock_result_metadata();
            state.last_result = raw.to_string();
            if value.get("ok").is_some_and(|status| !status.is_boolean()) {
                state.last_error = Some("result.ok must be a boolean".to_string());
                return false;
            }
            state.last_error = action_error_from_value(&value, raw);
            drop(state);
            if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
                return true;
            }
            let fields = value.get("value").and_then(serde_json::Value::as_object);
            let bindings = value.get("bindings").and_then(serde_json::Value::as_object);
            if fields.is_some() || bindings.is_some() {
                let mut view_state = lock_view_state();
                if view_state.is_object()
                    && fields.map_or(0, |fields| fields.len())
                        + bindings.map_or(0, |bindings| bindings.len())
                        == 1
                {
                    let (key, value) = fields
                        .into_iter()
                        .chain(bindings)
                        .flat_map(|fields| fields.iter())
                        .next()
                        .expect("one result field");
                    // One path cannot fail after its first insertion creates an object-only suffix.
                    if let Err(error) = write_binding_path(&mut view_state, key, value.clone()) {
                        drop(view_state);
                        lock_result_metadata().last_error = Some(error);
                        return false;
                    }
                    return true;
                }
                // shortcut: multi-field commits still clone for rollback; revisit if they become a hot path.
                let mut updated = if view_state.is_object() {
                    view_state.clone()
                } else {
                    serde_json::json!({})
                };
                for fields in fields.into_iter().chain(bindings) {
                    for (key, value) in fields {
                        if let Err(error) = write_binding_path(&mut updated, key, value.clone()) {
                            drop(view_state);
                            lock_result_metadata().last_error = Some(error);
                            return false;
                        }
                    }
                }
                *view_state = updated;
            }
            true
        }
        Err(_) => false,
    }
}

fn eval_text(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> String {
    display_value(&resolve_expr(expr, scope_name, scope_json))
}

fn eval_bool(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> bool {
    truthy_value(&resolve_expr(expr, scope_name, scope_json))
}

fn eval_number(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> f64 {
    number_value(&resolve_expr(expr, scope_name, scope_json))
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn eval_items_json(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> String {
    match resolve_path(expr, scope_name, scope_json) {
        serde_json::Value::Array(items) => serde_json::Value::Array(items).to_string(),
        _ => "[]".to_string(),
    }
}

fn resolve_expr(
    expr: &str,
    scope_name: Option<&str>,
    scope_json: Option<&str>,
) -> serde_json::Value {
    let trimmed = expr.trim();
    if trimmed.starts_with('!') {
        let mut rest = trimmed;
        let mut negate = false;
        while let Some(next) = rest.strip_prefix('!') {
            negate = !negate;
            rest = next.trim_start();
        }
        let value = truthy_value(&resolve_expr(rest, scope_name, scope_json));
        return serde_json::Value::Bool(value != negate);
    }
    for op in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some(index) = comparison_index(trimmed, op) {
            let left = resolve_path(trimmed[..index].trim(), scope_name, scope_json);
            let right = resolve_path(trimmed[index + op.len()..].trim(), scope_name, scope_json);
            return serde_json::Value::Bool(compare_values(&left, &right, op));
        }
    }
    resolve_path(trimmed, scope_name, scope_json)
}

fn comparison_index(expr: &str, op: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in expr.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
        } else if expr[index..].starts_with(op) {
            return Some(index);
        }
    }
    None
}

fn resolve_path(
    expr: &str,
    scope_name: Option<&str>,
    scope_json: Option<&str>,
) -> serde_json::Value {
    if let Some(literal) = literal_value(expr) {
        return literal;
    }
    if expr
        .split_once('.')
        .is_some_and(|(root, _)| matches!(root.trim(), "null" | "none" | "None" | "Null"))
    {
        return serde_json::Value::Null;
    }
    let scope = scope_json.and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
    if scope_name == Some("@crepus:lexical") {
        let Some((aliases, local)) = scope
            .as_ref()
            .and_then(|scope| Some((scope.get("aliases")?.as_object()?, scope.get("local")?)))
        else {
            return serde_json::Value::Null;
        };
        let (root, member) = expr
            .split_once('.')
            .map_or((expr, None), |(root, member)| (root, Some(member)));
        if let Some(item) = aliases.get(root) {
            return match member {
                Some(path) => lookup_path(path, item).unwrap_or(serde_json::Value::Null),
                None => item.clone(),
            };
        }
        return lookup_path(expr, local)
            .or_else(|| lookup_path(expr, &lock_view_state()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Some(scope) = scope {
        if let Some(scope_name) = scope_name {
            if expr == scope_name {
                return scope;
            }
            if let Some(path) = expr
                .strip_prefix(scope_name)
                .and_then(|rest| rest.strip_prefix('.'))
            {
                return lookup_path(path, &scope).unwrap_or(serde_json::Value::Null);
            }
        }
        return lookup_path(expr, &scope)
            .or_else(|| {
                let state = lock_view_state();
                lookup_path(expr, &state)
            })
            .unwrap_or(serde_json::Value::Null);
    }
    let state = lock_view_state();
    lookup_path(expr, &state).unwrap_or(serde_json::Value::Null)
}

fn lookup_path(path: &str, root: &serde_json::Value) -> Option<serde_json::Value> {
    if path.is_empty() {
        return Some(root.clone());
    }
    let mut current = Cow::Borrowed(root);
    for segment in path.split('.') {
        current = match current {
            Cow::Borrowed(value) => lookup_property(value, segment)?,
            Cow::Owned(value) => Cow::Owned(lookup_property(&value, segment)?.into_owned()),
        };
    }
    Some(current.into_owned())
}

fn lookup_property<'a>(
    value: &'a serde_json::Value,
    property: &str,
) -> Option<Cow<'a, serde_json::Value>> {
    use serde_json::{json, Value};
    let computed = match value {
        Value::Object(map) => return map.get(property).map(Cow::Borrowed),
        Value::Array(items) => match property {
            "len" | "length" => json!(items.len()),
            "is_empty" | "empty" => json!(items.is_empty()),
            _ => {
                return items
                    .get(property.parse::<usize>().ok()?)
                    .map(Cow::Borrowed)
            }
        },
        Value::String(text) => match property {
            "len" | "length" => json!(text.chars().count()),
            "upper" => json!(text.to_uppercase()),
            "lower" => json!(text.to_lowercase()),
            "trim" => json!(text.trim()),
            "is_empty" | "empty" => json!(text.is_empty()),
            _ => return None,
        },
        _ => return None,
    };
    Some(Cow::Owned(computed))
}

fn literal_value(expr: &str) -> Option<serde_json::Value> {
    match expr {
        "true" => Some(serde_json::Value::Bool(true)),
        "false" => Some(serde_json::Value::Bool(false)),
        "null" | "none" | "None" | "Null" => Some(serde_json::Value::Null),
        _ if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 => {
            serde_json::from_str::<String>(expr)
                .ok()
                .map(serde_json::Value::String)
        }
        _ => expr
            .parse::<i64>()
            .map(|value| serde_json::json!(value))
            .or_else(|_| expr.parse::<u64>().map(|value| serde_json::json!(value)))
            .or_else(|_| expr.parse::<f64>().map(|value| serde_json::json!(value)))
            .ok(),
    }
}

fn compare_values(left: &serde_json::Value, right: &serde_json::Value, op: &str) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    if let (Some(left_number), Some(right_number)) = (number_value(left), number_value(right)) {
        let order = match (integer_value(left), integer_value(right)) {
            (Some(left), Some(right)) => Some(left.cmp(&right)),
            (Some(left), None) => compare_integer_float(left, right_number),
            (None, Some(right)) => {
                compare_integer_float(right, left_number).map(|order| order.reverse())
            }
            (None, None) => left_number.partial_cmp(&right_number),
        };
        return match op {
            ">=" => matches!(order, Some(Greater | Equal)),
            "<=" => matches!(order, Some(Less | Equal)),
            ">" => order == Some(Greater),
            "<" => order == Some(Less),
            "==" => order == Some(Equal),
            "!=" => order != Some(Equal),
            _ => false,
        };
    }
    let left = stringify_value(left);
    let right = stringify_value(right);
    match op {
        "==" => left == right,
        "!=" => left != right,
        _ => false,
    }
}

fn compare_integer_float(integer: i128, float: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::{Equal, Greater, Less};
    if float.is_nan() {
        return None;
    }
    if float >= -(i128::MIN as f64) {
        return Some(Less);
    }
    if float < i128::MIN as f64 {
        return Some(Greater);
    }
    let order = integer.cmp(&(float.trunc() as i128));
    if order == Equal {
        0.0_f64.partial_cmp(&float.fract())
    } else {
        Some(order)
    }
}

fn integer_value(value: &serde_json::Value) -> Option<i128> {
    match value {
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from)),
        serde_json::Value::String(text) => text.parse::<i128>().ok(),
        _ => None,
    }
}

fn number_value(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn truthy_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(number) => number.as_f64().unwrap_or_default() != 0.0,
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
    }
}

fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => format!("[{} items]", items.len()),
        serde_json::Value::Object(_) => String::new(),
        _ => stringify_value(value),
    }
}

fn stringify_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => value.to_string(),
    }
}

#[no_mangle]
pub extern "C" fn crepus_mobile_store_result_json(
    json_ptr: *const c_char,
    json_len: usize,
) -> bool {
    action_from_ffi(json_ptr, json_len)
        .map(|json| store_result_json(json.as_ref()))
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_eval_text(
    expr_ptr: *const c_char,
    expr_len: usize,
    scope_name_ptr: *const c_char,
    scope_name_len: usize,
    scope_ptr: *const c_char,
    scope_len: usize,
    output_ptr: *mut c_char,
    output_len: usize,
) -> usize {
    let result = action_from_ffi(expr_ptr, expr_len)
        .map(|expr| {
            let scope_name = action_from_ffi(scope_name_ptr, scope_name_len);
            let scope = action_from_ffi(scope_ptr, scope_len);
            eval_text(expr.as_ref(), scope_name.as_deref(), scope.as_deref())
        })
        .unwrap_or_default();
    copy_json_to_output(&result, output_ptr, output_len)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_eval_bool(
    expr_ptr: *const c_char,
    expr_len: usize,
    scope_name_ptr: *const c_char,
    scope_name_len: usize,
    scope_ptr: *const c_char,
    scope_len: usize,
) -> bool {
    action_from_ffi(expr_ptr, expr_len)
        .map(|expr| {
            let scope_name = action_from_ffi(scope_name_ptr, scope_name_len);
            let scope = action_from_ffi(scope_ptr, scope_len);
            eval_bool(expr.as_ref(), scope_name.as_deref(), scope.as_deref())
        })
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_eval_number(
    expr_ptr: *const c_char,
    expr_len: usize,
    scope_name_ptr: *const c_char,
    scope_name_len: usize,
    scope_ptr: *const c_char,
    scope_len: usize,
) -> f64 {
    action_from_ffi(expr_ptr, expr_len)
        .map(|expr| {
            let scope_name = action_from_ffi(scope_name_ptr, scope_name_len);
            let scope = action_from_ffi(scope_ptr, scope_len);
            eval_number(expr.as_ref(), scope_name.as_deref(), scope.as_deref())
        })
        .unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_eval_items_json(
    expr_ptr: *const c_char,
    expr_len: usize,
    scope_name_ptr: *const c_char,
    scope_name_len: usize,
    scope_ptr: *const c_char,
    scope_len: usize,
    output_ptr: *mut c_char,
    output_len: usize,
) -> usize {
    let result = action_from_ffi(expr_ptr, expr_len)
        .map(|expr| {
            let scope_name = action_from_ffi(scope_name_ptr, scope_name_len);
            let scope = action_from_ffi(scope_ptr, scope_len);
            eval_items_json(expr.as_ref(), scope_name.as_deref(), scope.as_deref())
        })
        .unwrap_or_else(|| "[]".to_string());
    copy_json_to_output(&result, output_ptr, output_len)
}

#[cfg(test)]
fn reset_action_state() {
    *lock_action_state() = MobileActionState::default();
    *lock_result_metadata() = ResultMetadata::default();
}

#[cfg(test)]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
fn reset_test_data_root() {
    let root = data_root_dir();
    let _ = fs::remove_dir_all(root);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_dispatchAction(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    action: JString<'_>,
) -> bool {
    match env.get_string(&action) {
        Ok(action) => dispatch_action(action.to_string_lossy().as_ref()),
        Err(_) => false,
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_dispatchActionJson<
    'a,
>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    action: JString<'a>,
) -> JString<'a> {
    let result = match env.get_string(&action) {
        Ok(action) => dispatch_action_json(action.to_string_lossy().as_ref()),
        Err(_) => "{\"ok\":false,\"action\":\"\",\"error\":\"invalid action string\",\"state\":{\"syncCount\":0,\"previewCount\":0,\"lastAction\":\"\"}}".to_string(),
    };
    env.new_string(result).unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_dispatchAndStoreJson<
    'a,
>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    action: JString<'a>,
) -> JString<'a> {
    let result = match env.get_string(&action) {
        Ok(action) => dispatch_action_json(action.to_string_lossy().as_ref()),
        Err(_) => action_json(
            false,
            "",
            None,
            Some("invalid action pointer"),
            &MobileActionState::default(),
        ),
    };
    env.new_string(result).unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_dispatchChangeJson<
    'a,
>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    action: JString<'a>,
    bind: JString<'a>,
    value_json: JString<'a>,
) -> JString<'a> {
    let result = match (
        env.get_string(&action),
        env.get_string(&bind),
        env.get_string(&value_json),
    ) {
        (Ok(action), Ok(bind), Ok(value_json)) => dispatch_change_json(
            action.to_string_lossy().as_ref(),
            bind.to_string_lossy().as_ref(),
            value_json.to_string_lossy().as_ref(),
        ),
        _ => action_json(
            false,
            "",
            None,
            Some("invalid change payload"),
            &MobileActionState::default(),
        ),
    };
    env.new_string(result).unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_storeResultJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    json: JString<'_>,
) -> bool {
    match env.get_string(&json) {
        Ok(json) => store_result_json(json.to_string_lossy().as_ref()),
        Err(_) => false,
    }
}

#[cfg(target_os = "android")]
fn get_jni_string(env: &mut JNIEnv<'_>, j_str: &JString<'_>) -> String {
    env.get_string(j_str)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(target_os = "android")]
fn get_jni_string_opt(env: &mut JNIEnv<'_>, j_str: &JString<'_>) -> Option<String> {
    env.get_string(j_str)
        .ok()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_evalText<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    expr: JString<'a>,
    scope_name: JString<'a>,
    scope: JString<'a>,
) -> JString<'a> {
    let expr = get_jni_string(&mut env, &expr);
    let scope_name = get_jni_string_opt(&mut env, &scope_name);
    let scope = get_jni_string_opt(&mut env, &scope);
    env.new_string(eval_text(&expr, scope_name.as_deref(), scope.as_deref()))
        .unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_evalBool(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    expr: JString<'_>,
    scope_name: JString<'_>,
    scope: JString<'_>,
) -> bool {
    let expr = get_jni_string(&mut env, &expr);
    let scope_name = get_jni_string_opt(&mut env, &scope_name);
    let scope = get_jni_string_opt(&mut env, &scope);
    eval_bool(&expr, scope_name.as_deref(), scope.as_deref())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_evalNumber(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    expr: JString<'_>,
    scope_name: JString<'_>,
    scope: JString<'_>,
) -> f64 {
    let expr = get_jni_string(&mut env, &expr);
    let scope_name = get_jni_string_opt(&mut env, &scope_name);
    let scope = get_jni_string_opt(&mut env, &scope);
    eval_number(&expr, scope_name.as_deref(), scope.as_deref())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_evalItemsJson<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    expr: JString<'a>,
    scope_name: JString<'a>,
    scope: JString<'a>,
) -> JString<'a> {
    let expr = get_jni_string(&mut env, &expr);
    let scope_name = get_jni_string_opt(&mut env, &scope_name);
    let scope = get_jni_string_opt(&mut env, &scope);
    env.new_string(eval_items_json(
        &expr,
        scope_name.as_deref(),
        scope.as_deref(),
    ))
    .unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_lastResult<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    let ptr = crepus_mobile_last_result();
    let result = if ptr.is_null() {
        String::new()
    } else {
        let result = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { crepus_mobile_free_string(ptr) };
        result
    };
    env.new_string(result).unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_lastError<'a>(
    env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    let ptr = crepus_mobile_last_error();
    let result = if ptr.is_null() {
        String::new()
    } else {
        let result = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { crepus_mobile_free_string(ptr) };
        result
    };
    env.new_string(result).unwrap()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_initAndroid<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    _context: jni::objects::JObject<'a>,
) {
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_dev_crepuscularity_nativeshell_CrepusRustActions_shutdownAndroid<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    #[ignore = "manual release measurement of result storage"]
    fn result_storage_measurement() {
        let _guard = test_lock();
        let previous = lock_view_state().clone();
        for count in [1_000, 50_000] {
            let rows: Vec<_> = (0..count)
                .map(|i| format!("row-{i:08}-雪-abcdefghijklmnop"))
                .collect();
            let raw = serde_json::json!({"ok": true, "value": {"rows": rows}}).to_string();
            for _ in 0..3 {
                assert!(store_result_json(&raw));
            }
            let mut samples = Vec::new();
            for _ in 0..11 {
                let start = std::time::Instant::now();
                for _ in 0..5 {
                    assert!(store_result_json(std::hint::black_box(&raw)));
                }
                samples.push(start.elapsed().as_secs_f64() * 1_000_000.0 / 5.0);
            }
            samples.sort_by(f64::total_cmp);
            assert_eq!(lock_view_state()["rows"].as_array().unwrap().len(), count);
            println!(
                "RESULT_STORE count={count} bytes={} median_us={:.3}",
                raw.len(),
                samples[5]
            );
        }
        *lock_view_state() = previous;
    }

    #[test]
    fn lexical_scope_resolves_nested_aliases_local_values_and_globals() {
        let _guard = test_lock();
        let previous = std::mem::replace(
            &mut *lock_view_state(),
            serde_json::json!({
                "row": {"title": "global row", "secret": "global secret"},
                "title": "global title", "global": {"name": "shared"}
            }),
        );
        let scope = serde_json::json!({
            "aliases": {
                "row": {"title": "outer"}, "child": {"title": "inner"},
                "number": 7, "list": ["one", {"title": "second"}],
                "word": " A ", "empty": null,
                "null": {"title": "collision"}, "none": {"title": "collision"},
                "None": {"title": "collision"}, "Null": {"title": "collision"}
            },
            "local": {"title": "inner", "row": {"secret": "local secret"}},
            "title": "wrapper title"
        })
        .to_string();
        for (expr, expected) in [
            ("row.title == child.title", serde_json::json!(false)),
            ("title == child.title", serde_json::json!(true)),
            ("title", serde_json::json!("inner")),
            ("row", serde_json::json!({"title": "outer"})),
            ("row.secret", serde_json::Value::Null),
            ("global.name", serde_json::json!("shared")),
            ("number", serde_json::json!(7)),
            ("number.title", serde_json::Value::Null),
            ("list", serde_json::json!(["one", {"title": "second"}])),
            ("list.1.title", serde_json::json!("second")),
            ("list.length", serde_json::json!(2)),
            ("list.2", serde_json::Value::Null),
            ("word.trim", serde_json::json!("A")),
            ("empty", serde_json::Value::Null),
            ("empty.title", serde_json::Value::Null),
        ] {
            assert_eq!(
                resolve_expr(expr, Some("@crepus:lexical"), Some(&scope)),
                expected,
                "{expr}"
            );
        }
        for alias in ["null", "none", "None", "Null"] {
            for expr in [alias.to_string(), format!("{alias}.title")] {
                assert_eq!(
                    resolve_expr(&expr, Some("@crepus:lexical"), Some(&scope)),
                    serde_json::Value::Null,
                    "{expr}"
                );
            }
        }
        // The supplied map already contains the nearest binding for a shadowed alias.
        let shadowed =
            r#"{"aliases":{"row":{"title":"inner"},"child":{"title":"inner"}},"local":null}"#;
        assert_eq!(
            resolve_expr(
                "row.title == child.title",
                Some("@crepus:lexical"),
                Some(shadowed)
            ),
            serde_json::json!(true)
        );
        // Ordinary scope transport remains an item rather than a lexical wrapper.
        assert_eq!(
            resolve_expr("row.title", Some("row"), Some(r#"{"title":"ordinary"}"#)),
            serde_json::json!("ordinary")
        );
        *lock_view_state() = previous;
    }

    #[test]
    fn malformed_lexical_scope_never_falls_back_to_global_state() {
        let _guard = test_lock();
        let previous = std::mem::replace(
            &mut *lock_view_state(),
            serde_json::json!({"title":"global", "row":{"secret":"global"}}),
        );
        for scope in [
            None,
            Some("invalid"),
            Some("null"),
            Some("[]"),
            Some("{}"),
            Some(r#"{"aliases":{},"title":"wrapper"}"#),
            Some(r#"{"aliases":null,"local":{}}"#),
            Some(r#"{"aliases":[],"local":{"title":"local"}}"#),
        ] {
            for expr in ["title", "row.secret", "aliases", "local"] {
                assert_eq!(
                    resolve_expr(expr, Some("@crepus:lexical"), scope),
                    serde_json::Value::Null,
                    "{expr}: {scope:?}"
                );
            }
            assert_eq!(
                resolve_expr("42", Some("@crepus:lexical"), scope),
                serde_json::json!(42)
            );
        }
        *lock_view_state() = previous;
    }

    #[test]
    fn c_abi_eval_accepts_lexical_scope_without_signature_changes() {
        let expr = "row.title == child.title";
        let scope_name = "@crepus:lexical";
        let scope = r#"{"aliases":{"row":{"title":"same"},"child":{"title":"same"}},"local":{"title":"local"}}"#;
        assert!(crepus_mobile_eval_bool(
            expr.as_ptr().cast(),
            expr.len(),
            scope_name.as_ptr().cast(),
            scope_name.len(),
            scope.as_ptr().cast(),
            scope.len()
        ));
        let expr = "title";
        let mut output = [0 as c_char; 32];
        let written = crepus_mobile_eval_text(
            expr.as_ptr().cast(),
            expr.len(),
            scope_name.as_ptr().cast(),
            scope_name.len(),
            scope.as_ptr().cast(),
            scope.len(),
            output.as_mut_ptr(),
            output.len(),
        );
        assert_eq!(written, 5);
        assert_eq!(
            unsafe { CStr::from_ptr(output.as_ptr()) }.to_str().unwrap(),
            "local"
        );
    }

    #[test]
    fn display_values_do_not_replace_collection_transport_or_comparison() {
        let scope = serde_json::json!({
            "yes": true, "no": false, "number": 1.5, "literal": "{keep}",
            "items": [{"name": "one"}, {"name": "two"}], "empty": [],
            "object": {"name": "one"}, "null_value": null,
            "other_items": [{"name": "different"}, {"name": "two"}],
            "other_object": {"name": "different"}
        })
        .to_string();
        for (path, expected) in [
            ("yes", "true"),
            ("no", "false"),
            ("number", "1.5"),
            ("literal", "{keep}"),
            ("items", "[2 items]"),
            ("empty", "[0 items]"),
            ("object", ""),
            ("null_value", ""),
            ("missing", ""),
        ] {
            assert_eq!(eval_text(path, None, Some(&scope)), expected, "{path}");
        }
        assert_eq!(
            eval_text("item", Some("item"), Some(r#"{"name":"one"}"#)),
            ""
        );
        let transported: serde_json::Value =
            serde_json::from_str(&eval_items_json("items", None, Some(&scope))).unwrap();
        assert_eq!(
            transported,
            serde_json::json!([{"name":"one"},{"name":"two"}])
        );
        assert!(!eval_bool("items == other_items", None, Some(&scope)));
        assert!(!eval_bool("object == other_object", None, Some(&scope)));
    }

    #[test]
    fn string_and_list_properties_match_template_values() {
        let state = serde_json::json!({
            "name": " A😀é ", "empty": "", "items": [{"name": "one"}, {"name": "two"}],
            "empty_items": [], "object": {"length": "field", "upper": "also field"}
        });
        for (path, expected) in [
            ("name.length", serde_json::json!(5)),
            ("name.len", serde_json::json!(5)),
            ("name.trim.upper", serde_json::json!("A😀É")),
            ("name.trim.lower.length", serde_json::json!(3)),
            ("empty.is_empty", serde_json::json!(true)),
            ("name.empty", serde_json::json!(false)),
            ("items.len", serde_json::json!(2)),
            ("items.length", serde_json::json!(2)),
            ("items.empty", serde_json::json!(false)),
            ("empty_items.is_empty", serde_json::json!(true)),
            ("items.1.name.upper", serde_json::json!("TWO")),
            ("object.length", serde_json::json!("field")),
            ("object.upper", serde_json::json!("also field")),
        ] {
            assert_eq!(lookup_path(path, &state), Some(expected.clone()), "{path}");
            assert_eq!(
                resolve_expr(path, None, Some(&state.to_string())),
                expected,
                "{path}"
            );
        }
        for path in [
            "missing.length",
            "items.-1",
            "items.2",
            "empty_items.0",
            "name.unknown",
        ] {
            assert_eq!(lookup_path(path, &state), None, "{path}");
        }
    }

    #[test]
    fn null_aliases_are_literals_even_when_scope_has_matching_keys() {
        let scope = r#"{"none":{"field":"collision"},"None":{"field":"collision"},"Null":{"field":"collision"},"object":{"None":"field"}}"#;
        for alias in ["null", "none", "None", "Null"] {
            assert_eq!(
                resolve_expr(alias, Some("item"), Some(scope)),
                serde_json::Value::Null
            );
            assert_eq!(eval_text(alias, Some("item"), Some(scope)), "");
            assert!(!eval_bool(alias, Some("item"), Some(scope)));
            assert_eq!(
                resolve_expr(&format!("{alias}.field"), None, Some(scope)),
                serde_json::Value::Null
            );
        }
        assert_eq!(eval_text("object.None", None, Some(scope)), "field");
    }

    #[test]
    fn dispatch_action_json_mutates_state() {
        let _guard = test_lock();
        reset_action_state();
        let first = dispatch_action_json("sync");
        let second = dispatch_action_json("preview");

        let first: serde_json::Value = serde_json::from_str(&first).expect("json");
        let second: serde_json::Value = serde_json::from_str(&second).expect("json");
        assert_eq!(first["ok"], true);
        assert_eq!(first["action"], "sync");
        assert_eq!(first["state"]["syncCount"], 1);
        assert_eq!(second["ok"], true);
        assert_eq!(second["action"], "preview");
        assert_eq!(second["state"]["syncCount"], 1);
        assert_eq!(second["state"]["previewCount"], 1);
        assert_eq!(second["state"]["lastAction"], "preview");
    }

    #[test]
    fn dispatch_action_json_reports_unknown_actions() {
        let _guard = test_lock();
        reset_action_state();
        let result = dispatch_action_json("missing");

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], false);
        assert_eq!(result["action"], "missing");
        assert_eq!(result["error"], "unknown action");
    }

    #[test]
    fn dispatch_known_action_checks_registry_without_mutation() {
        let _guard = test_lock();
        reset_action_state();
        assert!(dispatch_action("sync"));
        assert!(dispatch_action("preview"));
        assert!(dispatch_action(
            r#"{"kind":"plugin","capability":"device","method":"info"}"#
        ));
        let state = lock_action_state();
        assert_eq!(state.sync_count, 0);
        assert_eq!(state.preview_count, 0);
    }

    #[test]
    fn dispatch_action_json_accepts_typed_payloads() {
        let _guard = test_lock();
        reset_action_state();
        let result = dispatch_action_json(r#"{"action":"sync","payload":{"message":"hydrate"}}"#);

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["action"], "sync");
        assert_eq!(result["value"]["message"], "hydrate");
        assert_eq!(result["state"]["lastPayload"], r#"{"message":"hydrate"}"#);
    }

    #[test]
    fn dispatch_change_json_builds_action_payload_in_rust() {
        let _guard = test_lock();
        reset_action_state();
        let result = dispatch_change_json("sync", "enabled", "true");

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["action"], "sync");
        assert_eq!(result["value"]["enabled"], true);
        assert_eq!(result["state"]["lastPayload"], r#"{"enabled":true}"#);
    }

    #[test]
    fn named_control_bindings_do_not_become_request_metadata() {
        let _guard = test_lock();
        for bind in ["action", "kind", "payload", "capability", "method"] {
            reset_action_state();
            *lock_view_state() = serde_json::json!({});
            let raw = dispatch_change_json("sync", bind, "\"plugin\"");
            let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
            assert_eq!(result["ok"], true, "{bind}: {raw}");
            assert_eq!(result["action"], "sync", "{bind}: {raw}");
            let payload: serde_json::Value =
                serde_json::from_str(result["state"]["lastPayload"].as_str().expect("payload"))
                    .expect("payload json");
            assert_eq!(payload[bind], "plugin");
            assert!(store_result_json(&raw));
            assert_eq!(eval_text(bind, None, None), "plugin", "{bind}: {raw}");
        }
        let raw = dispatch_change_json("unknown", "method", "\"ignored\"");
        let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(result["ok"], false);
        assert!(store_result_json(&raw));
        assert_eq!(eval_text("method", None, None), "plugin");
    }

    #[test]
    fn plugin_control_changes_keep_bindings_separate_from_plugin_results() {
        let _guard = test_lock();
        for bind in ["enabled", "capability", "method", "value"] {
            reset_action_state();
            *lock_view_state() = serde_json::json!({});
            let raw = dispatch_change_json("device.info", bind, "true");
            let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
            assert_eq!(result["ok"], true, "{bind}: {raw}");
            assert_eq!(result["value"]["capability"], "device");
            assert_eq!(result["value"]["method"], "info");
            assert!(result["value"]["value"].is_object());
            assert!(store_result_json(&raw));
            assert!(eval_bool(bind, None, None), "{bind}: {raw}");
        }
        let raw = dispatch_change_json("device.missing", "value", "false");
        let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(result["ok"], false);
        assert!(result.get("bindings").is_none());
        assert!(store_result_json(&raw));
        assert!(eval_bool("value", None, None));
    }

    #[test]
    fn dotted_plugin_actions_use_the_typed_request_route() {
        let _guard = test_lock();
        reset_action_state();
        for action in ["device.info", "app.info"] {
            let raw = dispatch_action_json(action);
            let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
            assert_eq!(result["ok"], true, "{raw}");
            assert_eq!(result["action"], action);
            assert!(result["value"]["value"].is_object());
        }
        let raw = dispatch_action_json("missing.info");
        let result: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"], "unsupported capability: missing");
        let request = r#"{"action":"sync","payload":{"message":"example.com"}}"#;
        assert_eq!(
            parse_action_request(request)["payload"]["message"],
            "example.com"
        );
    }

    #[test]
    fn native_expressions_preserve_quoted_operators_and_decode_escapes() {
        let _guard = test_lock();
        for text in [
            "a>b",
            "a<=b",
            "a==b",
            "a!=b",
            "quote \" > 🧪",
            "line\nnext",
            "slash\\end",
        ] {
            let expr = serde_json::to_string(text).unwrap();
            assert_eq!(eval_text(&expr, None, None), text);
            assert!(eval_bool(&format!("{expr} == {expr}"), None, None));
            assert!(!eval_bool(&format!("{expr} != {expr}"), None, None));
        }
        *lock_view_state() = serde_json::json!({"title":"a>b", "count": 3});
        assert!(eval_bool(r#"title == "a>b""#, None, None));
        assert!(eval_bool("count >= 3", None, None));
        assert!(!eval_bool("count < 3", None, None));
        assert_eq!(literal_value(r#""bad\q""#), None);
    }

    #[test]
    fn unary_negation_chains_use_bounded_stack_space() {
        let _guard = test_lock();
        assert!(!eval_bool("!true", None, None));
        assert!(eval_bool("! ! true", None, None));
        assert_eq!(eval_text("!!42", None, None), "true");
        assert!(eval_bool("! 1 > 2", None, None));
        let even = format!("{}true", "!".repeat(100_000));
        assert!(eval_bool(&even, None, None));
        assert!(!eval_bool(&format!("!{even}"), None, None));
    }

    #[test]
    fn integer_comparisons_preserve_precision_above_double_range() {
        let _guard = test_lock();
        *lock_view_state() = serde_json::json!({
            "low": 9007199254740992_i64, "high": 9007199254740993_i64,
            "unsigned": u64::MAX, "signed": i64::MAX,
            "text": "9007199254740993"
        });
        for expr in [
            "low < high",
            "low != high",
            "high > low",
            "high == text",
            "unsigned > signed",
            "unsigned == 18446744073709551615",
            "-9223372036854775808 < signed",
            "1.5 < 2.0",
        ] {
            assert!(eval_bool(expr, None, None), "{expr}");
        }
        for expr in ["low == high", "high <= low", "unsigned == signed"] {
            assert!(!eval_bool(expr, None, None), "{expr}");
        }
    }

    #[test]
    fn mixed_numeric_comparisons_do_not_round_integer_operands() {
        let _guard = test_lock();
        for expr in [
            "9007199254740993 > 9007199254740992.0",
            "9007199254740992.0 < 9007199254740993",
            "18446744073709551615 < 18446744073709551616.0",
            "-9223372036854775807 > -9223372036854775808.0",
            "1 < 1.5",
            "-1 > -1.5",
            "1 == 1.0",
            "0 == -0.0",
        ] {
            assert!(eval_bool(expr, None, None), "{expr}");
        }
        assert!(!eval_bool(
            "9007199254740993 == 9007199254740992.0",
            None,
            None
        ));
        assert_eq!(
            compare_integer_float(i128::MAX, -(i128::MIN as f64)),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_integer_float(i128::MIN, i128::MIN as f64),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(compare_integer_float(0, f64::NAN), None);
        assert_eq!(
            compare_integer_float(0, f64::INFINITY),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_integer_float(0, f64::NEG_INFINITY),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn result_decoding_preserves_floating_point_boundary_values() {
        let _guard = test_lock();
        let expected = -9.223372036854775e18_f64;
        assert!(store_result_json(
            r#"{"value":{"decimal":-9.223372036854775e18,"integer":-9223372036854775808}}"#
        ));
        assert_eq!(
            eval_number("decimal", None, None).to_bits(),
            expected.to_bits()
        );
        assert!(eval_bool("integer < decimal", None, None));
    }

    #[test]
    fn numeric_model_reads_reject_nonfinite_strings() {
        let _guard = test_lock();
        for text in ["NaN", "inf", "-inf", "1e309"] {
            *lock_view_state() = serde_json::json!({ "number": text });
            assert_eq!(eval_number("number", None, None), 0.0, "{text}");
        }
        *lock_view_state() = serde_json::json!({ "number": "0.125" });
        assert_eq!(eval_number("number", None, None), 0.125);
        *lock_view_state() = serde_json::json!({ "number": 1e100 });
        assert_eq!(eval_number("number", None, None), 1e100);
    }

    #[test]
    fn rejected_results_record_errors_without_applying_model_fields() {
        let _guard = test_lock();
        reset_action_state();
        *lock_view_state() = serde_json::json!({"enabled":true,"title":"kept"});
        let raw = r#"{"ok":false,"error":"rejected","value":{"enabled":false},"bindings":{"title":"lost"}}"#;
        assert!(store_result_json(raw));
        assert!(eval_bool("enabled", None, None));
        assert_eq!(eval_text("title", None, None), "kept");
        let state = lock_result_metadata();
        assert_eq!(state.last_error.as_deref(), Some("rejected"));
        assert_eq!(state.last_result, raw);
        drop(state);
        for (raw, expected) in [
            (r#" { "ok": false } "#, Some(r#" { "ok": false } "#)),
            (
                r#"{"ok":false,"error":5}"#,
                Some(r#"{"ok":false,"error":5}"#),
            ),
            (r#"{"ok":false,"error":""}"#, Some("")),
            (r#"{"ok":true,"error":"ignored"}"#, None),
            (r#"{}"#, None),
        ] {
            assert_eq!(parse_action_error(raw).as_deref(), expected);
            assert!(store_result_json(raw));
            let metadata = lock_result_metadata();
            assert_eq!(metadata.last_error.as_deref(), expected);
            assert_eq!(metadata.last_result, raw);
        }
        assert_eq!(parse_action_error("not json"), None);
        assert!(store_result_json(r#"{"value":{"title":"legacy"}}"#));
        assert_eq!(eval_text("title", None, None), "legacy");
    }

    #[test]
    fn dispatch_diagnostics_recover_without_publishing_unhandled_requests() {
        let _guard = test_lock();
        reset_action_state();
        let failed = dispatch_action_json("missing.info");
        {
            let metadata = lock_result_metadata();
            assert_eq!(metadata.last_result, failed);
            assert_eq!(
                metadata.last_error.as_deref(),
                Some("unsupported capability: missing")
            );
        }
        let successful = dispatch_action_json("sync");
        dispatch_action_json("unknown");
        dispatch_change_json("sync", "enabled", "invalid json");
        assert!(!store_result_json("invalid json"));
        let metadata = lock_result_metadata();
        assert_eq!(metadata.last_result, successful);
        assert_eq!(metadata.last_error, None);
    }

    #[test]
    fn result_sink_and_diagnostics_do_not_wait_for_running_handler() {
        let _guard = test_lock();
        reset_action_state();
        *lock_view_state() = serde_json::json!({"enabled":true});
        let handler = lock_action_state();
        let (completed, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let read = |ptr: *mut c_char| {
                assert!(!ptr.is_null());
                let text = unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { crepus_mobile_free_string(ptr) };
                text
            };
            let success = r#"{"ok":true,"value":{"enabled":false}}"#;
            assert!(store_result_json(success));
            assert_eq!(read(crepus_mobile_last_result()), success);
            assert_eq!(read(crepus_mobile_last_error()), "");
            assert!(!eval_bool("enabled", None, None));
            let rejected = r#"{"ok":false,"error":"rejected","value":{"enabled":true}}"#;
            assert!(store_result_json(rejected));
            assert_eq!(read(crepus_mobile_last_result()), rejected);
            assert_eq!(read(crepus_mobile_last_error()), "rejected");
            assert!(!eval_bool("enabled", None, None));
            let invalid = r#"{"ok":true,"bindings":{"enabled.child":true}}"#;
            assert!(!store_result_json(invalid));
            assert_eq!(read(crepus_mobile_last_result()), invalid);
            assert!(read(crepus_mobile_last_error()).contains("cannot traverse scalar"));
            assert!(!eval_bool("enabled", None, None));
            completed.send(()).unwrap();
        });
        let result = receiver.recv_timeout(Duration::from_secs(5));
        // Release before assertions/join so a regressed worker cannot stay blocked.
        drop(handler);
        worker.join().expect("result worker panicked");
        result.expect("result sink waited for handler lock");
    }

    #[test]
    fn malformed_result_statuses_cannot_apply_model_updates() {
        let _guard = test_lock();
        for status in [
            serde_json::json!("false"),
            serde_json::json!(0),
            serde_json::Value::Null,
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            reset_action_state();
            *lock_view_state() = serde_json::json!({"enabled":true});
            let raw=serde_json::json!({"ok":status,"value":{"enabled":false},"bindings":{"enabled":false}}).to_string();
            assert!(!store_result_json(&raw), "{raw}");
            assert!(eval_bool("enabled", None, None), "{raw}");
            let state = lock_result_metadata();
            assert_eq!(
                state.last_error.as_deref(),
                Some("result.ok must be a boolean")
            );
            assert_eq!(state.last_result, raw);
        }
    }

    #[test]
    fn dotted_bindings_update_nested_objects_and_existing_array_items() {
        let _guard = test_lock();
        *lock_view_state() = serde_json::json!({"settings":{"theme":"dark","enabled":false},"items":[{"enabled":false,"id":42}]});
        let raw = dispatch_change_json("", "settings.enabled", "true");
        assert!(
            serde_json::from_str::<serde_json::Value>(&raw).unwrap()["ok"]
                .as_bool()
                .unwrap()
        );
        assert!(eval_bool("settings.enabled", None, None));
        assert_eq!(eval_text("settings.theme", None, None), "dark");
        let raw = dispatch_change_json("sync", "items.0.enabled", "true");
        assert!(store_result_json(&raw));
        assert!(eval_bool("items.0.enabled", None, None));
        assert_eq!(eval_number("items.0.id", None, None), 42.0);
        assert!(lock_view_state().get("settings.enabled").is_none());
        assert!(store_result_json(
            r#"{"bindings":{"new.branch.title":"created"}}"#
        ));
        assert_eq!(eval_text("new.branch.title", None, None), "created");
        let path = std::iter::repeat_n("child", 126)
            .collect::<Vec<_>>()
            .join(".");
        let raw = dispatch_change_json("", &path, "true");
        assert!(store_result_json(&raw));
        assert!(eval_bool(&path, None, None));
    }

    #[test]
    fn invalid_binding_paths_do_not_partially_update_state() {
        let _guard = test_lock();
        let original = serde_json::json!({"items":[{"id":42}],"enabled":true});
        *lock_view_state() = original.clone();
        let raw = r#"{"ok":true,"value":{"enabled":false},"bindings":{"items.999.id":0}}"#;
        assert!(!store_result_json(raw));
        assert_eq!(*lock_view_state(), original);
        for path in ["items.bad.id", "items.999.id", "new.branch.items.0.child"] {
            let raw = serde_json::json!({
                "value": {"enabled": false},
                "bindings": {path: true, "items.bad.id": false}
            })
            .to_string();
            assert!(!store_result_json(&raw), "{path}");
            assert_eq!(*lock_view_state(), original, "{path}");
        }
        let raw = dispatch_change_json("", "enabled.child", "true");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&raw).unwrap()["ok"],
            false
        );
        assert_eq!(*lock_view_state(), original);
        let path = std::iter::repeat_n("child", 127)
            .collect::<Vec<_>>()
            .join(".");
        let raw = dispatch_change_json("", &path, "true");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&raw).unwrap()["ok"],
            false
        );
        assert_eq!(*lock_view_state(), original);
    }

    #[test]
    fn preferences_round_trip_persists_values() {
        let _guard = test_lock();
        reset_action_state();
        reset_test_data_root();
        let set = dispatch_action_json(
            r#"{"kind":"plugin","capability":"preferences","method":"set","payload":{"key":"theme","value":"light"}}"#,
        );
        let get = dispatch_action_json(
            r#"{"kind":"plugin","capability":"preferences","method":"get","payload":{"key":"theme"}}"#,
        );
        let keys =
            dispatch_action_json(r#"{"kind":"plugin","capability":"preferences","method":"keys"}"#);

        let set: serde_json::Value = serde_json::from_str(&set).expect("json");
        let get: serde_json::Value = serde_json::from_str(&get).expect("json");
        let keys: serde_json::Value = serde_json::from_str(&keys).expect("json");
        assert_eq!(set["ok"], true);
        assert_eq!(get["value"]["value"], "light");
        assert_eq!(keys["value"]["value"][0], "theme");
    }

    #[cfg(feature = "filesystem")]
    #[test]
    fn filesystem_round_trip_handles_text_files() {
        let _guard = test_lock();
        reset_action_state();
        reset_test_data_root();
        let write = dispatch_action_json(
            r#"{"kind":"plugin","capability":"filesystem","method":"writeText","payload":{"path":"notes/hello.txt","text":"hi"}}"#,
        );
        let read = dispatch_action_json(
            r#"{"kind":"plugin","capability":"filesystem","method":"readText","payload":{"path":"notes/hello.txt"}}"#,
        );
        let list = dispatch_action_json(
            r#"{"kind":"plugin","capability":"filesystem","method":"list","payload":{"path":"notes"}}"#,
        );

        let write: serde_json::Value = serde_json::from_str(&write).expect("json");
        let read: serde_json::Value = serde_json::from_str(&read).expect("json");
        let list: serde_json::Value = serde_json::from_str(&list).expect("json");
        assert_eq!(write["ok"], true);
        assert_eq!(read["value"]["value"]["text"], "hi");
        assert_eq!(list["value"]["value"]["entries"][0]["name"], "hello.txt");
    }

    #[cfg(feature = "filesystem")]
    #[test]
    fn filesystem_rejects_parent_escape() {
        let _guard = test_lock();
        reset_action_state();
        let result = dispatch_action_json(
            r#"{"kind":"plugin","capability":"filesystem","method":"readText","payload":{"path":"../secret.txt"}}"#,
        );

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"], "parent segments are not allowed");
    }

    #[cfg(not(feature = "filesystem"))]
    #[test]
    fn filesystem_requires_the_installed_feature() {
        let _guard = test_lock();
        reset_action_state();
        let result = dispatch_action_json(
            r#"{"kind":"plugin","capability":"filesystem","method":"readText","payload":{"path":"notes/hello.txt"}}"#,
        );

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"], "unsupported capability: filesystem");
    }

    #[test]
    fn device_and_app_requests_return_real_values() {
        let _guard = test_lock();
        reset_action_state();
        let device =
            dispatch_action_json(r#"{"kind":"plugin","capability":"device","method":"info"}"#);
        let app = dispatch_action_json(r#"{"kind":"plugin","capability":"app","method":"info"}"#);

        let device: serde_json::Value = serde_json::from_str(&device).expect("json");
        let app: serde_json::Value = serde_json::from_str(&app).expect("json");
        assert_eq!(device["ok"], true);
        assert!(device["value"]["value"]["targetOs"].is_string());
        assert_eq!(app["ok"], true);
        assert_eq!(app["value"]["value"]["syncCount"], 0);
    }

    #[test]
    fn bind_only_change_updates_view_state() {
        let _guard = test_lock();
        reset_action_state();
        *lock_view_state() = serde_json::json!({});

        let result = dispatch_change_json("", "setup_secret", "\"Smoke-Setup-2026\"");

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["action"], "bind");
        assert_eq!(eval_text("setup_secret", None, None), "Smoke-Setup-2026");
    }

    #[test]
    fn result_sink_keeps_bound_input_state() {
        let _guard = test_lock();
        reset_action_state();
        *lock_view_state() = serde_json::json!({});
        let result = dispatch_change_json("", "setup_secret", "\"Smoke-Setup-2026\"");

        assert!(store_result_json(&result));

        assert_eq!(eval_text("setup_secret", None, None), "Smoke-Setup-2026");
    }

    #[test]
    fn network_status_reports_reachable_loopback_port() {
        let _guard = test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = thread::spawn(move || {
            let _ = listener.accept();
        });

        let result = dispatch_action_json(&format!(
            r#"{{"kind":"plugin","capability":"network","method":"status","payload":{{"host":"127.0.0.1","port":{port},"timeoutMs":100}}}}"#
        ));
        server.join().expect("server thread");
        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["value"]["value"]["reachable"], true);
    }

    #[test]
    fn http_get_returns_text_from_loopback_server() {
        let _guard = test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).expect("read request");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Type: text/plain\r\n\r\nhello",
                )
                .expect("write response");
        });

        let result = dispatch_action_json(&format!(
            r#"{{"kind":"plugin","capability":"http","method":"get","payload":{{"url":"http://127.0.0.1:{port}/ping"}}}}"#
        ));
        server.join().expect("server thread");

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["value"]["value"]["status"], 200);
        assert_eq!(result["value"]["value"]["text"], "hello");
    }

    #[test]
    fn dispatch_action_json_recovers_from_poisoned_state_lock() {
        let _guard = test_lock();
        reset_action_state();
        let _ = std::thread::spawn(|| {
            let _state = action_state().lock().unwrap();
            panic!("poison state lock");
        })
        .join();

        let result = dispatch_action_json("sync");

        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], true);
        assert_eq!(result["action"], "sync");
        assert_eq!(result["state"]["syncCount"], 1);
        reset_action_state();
    }

    #[test]
    fn c_abi_json_null_pointer_reports_full_state_shape() {
        let _guard = test_lock();
        reset_action_state();
        let mut output = [0 as c_char; 256];

        let written =
            crepus_mobile_dispatch_json(std::ptr::null(), 0, output.as_mut_ptr(), output.len());

        assert!(written > 0);
        let result = unsafe { CStr::from_ptr(output.as_ptr()) }.to_string_lossy();
        let result: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"], "invalid action pointer");
        assert_eq!(result["state"]["syncCount"], 0);
        assert_eq!(result["state"]["previewCount"], 0);
        assert_eq!(result["state"]["lastAction"], "");
        assert!(result["state"].get("lastPayload").is_some());
    }

    #[test]
    fn owned_action_results_preserve_large_values_without_replaying_mutations() {
        let _guard = test_lock();
        reset_action_state();
        fn take(pointer: *mut c_char) -> serde_json::Value {
            assert!(!pointer.is_null());
            let result = unsafe { CStr::from_ptr(pointer) }
                .to_str()
                .unwrap()
                .to_owned();
            unsafe { crepus_mobile_free_string(pointer) };
            serde_json::from_str(&result).unwrap()
        }
        let large = "雪\0😀".repeat(6000);
        let action =
            serde_json::json!({"action": "sync", "payload": {"message": large}}).to_string();
        let first = take(crepus_mobile_dispatch_json_owned(
            action.as_ptr().cast(),
            action.len(),
        ));
        assert_eq!(first["ok"], true);
        assert_eq!(first["value"]["message"], large);
        assert_eq!(first["state"]["syncCount"], 1);
        let value = serde_json::to_string(&large).unwrap();
        let changed = take(crepus_mobile_dispatch_change_json_owned(
            b"sync".as_ptr().cast(),
            4,
            b"title".as_ptr().cast(),
            5,
            value.as_ptr().cast(),
            value.len(),
        ));
        assert_eq!(changed["ok"], true);
        assert_eq!(changed["value"]["title"], large);
        assert_eq!(changed["state"]["syncCount"], 2);
        let invalid = take(crepus_mobile_dispatch_change_json_owned(
            b"sync".as_ptr().cast(),
            4,
            std::ptr::null(),
            0,
            value.as_ptr().cast(),
            value.len(),
        ));
        assert_eq!(invalid["error"], "invalid bind pointer");
        assert_eq!(lock_action_state().sync_count, 2);
        reset_action_state();
    }

    #[test]
    fn c_abi_accepts_len_prefixed_action() {
        let _guard = test_lock();
        reset_action_state();
        let action = "sync";
        assert!(crepus_mobile_dispatch(
            action.as_ptr().cast::<c_char>(),
            action.len()
        ));
    }

    #[test]
    fn c_abi_accepts_c_string_action() {
        let _guard = test_lock();
        reset_action_state();
        let action = CString::new("preview").unwrap();
        assert!(crepus_mobile_dispatch(action.as_ptr(), 0));
    }

    #[test]
    fn unknown_action_returns_false() {
        let _guard = test_lock();
        reset_action_state();
        assert!(!dispatch_action("missing"));
    }
    #[test]
    fn single_result_field_preserves_unrelated_collection_allocations() {
        let _guard = test_lock();
        for raw in [
            r#"{"value":{"title":"updated"}}"#,
            r#"{"value":{},"bindings":{"title":"updated"}}"#,
            r#"{"value":{"title":"updated"},"bindings":{}}"#,
            r#"{"bindings":{"new.branch.title":"updated"}}"#,
        ] {
            *lock_view_state() =
                serde_json::json!({"large":[{"payload":"x".repeat(16_384)}],"title":"old"});
            let pointers = || {
                let state = lock_view_state();
                (
                    state["large"].as_array().unwrap().as_ptr() as usize,
                    state["large"][0]["payload"].as_str().unwrap().as_ptr() as usize,
                )
            };
            let before = pointers();
            assert!(store_result_json(raw));
            assert_eq!(pointers(), before, "{raw}");
            let title = if raw.contains("new.branch") {
                "new.branch.title"
            } else {
                "title"
            };
            assert_eq!(eval_text(title, None, None), "updated");
        }
    }

    #[test]
    fn single_invalid_result_path_preserves_state_and_records_error() {
        let _guard = test_lock();
        let original = serde_json::json!({"items":[{"id":42}],"enabled":true});
        for path in [
            "items.bad.id".to_owned(),
            "items.999.id".to_owned(),
            "enabled.child".to_owned(),
            std::iter::repeat_n("child", 127)
                .collect::<Vec<_>>()
                .join("."),
        ] {
            *lock_view_state() = original.clone();
            let raw = serde_json::json!({"bindings": {path: false}}).to_string();
            assert!(!store_result_json(&raw));
            assert_eq!(*lock_view_state(), original);
            let metadata = lock_result_metadata();
            assert_eq!(metadata.last_result, raw);
            assert!(metadata.last_error.is_some());
        }
    }

    #[test]
    fn multiple_overlapping_result_fields_keep_atomic_rollback() {
        let _guard = test_lock();
        let original = serde_json::json!({"items":[{"id":42}],"title":"old"});
        for raw in [
            r#"{"value":{"items":false},"bindings":{"items.0.id":3}}"#,
            r#"{"value":{"title":"changed"},"bindings":{"items.99.id":3}}"#,
        ] {
            *lock_view_state() = original.clone();
            assert!(!store_result_json(raw));
            assert_eq!(*lock_view_state(), original);
        }
        assert!(store_result_json(
            r#"{"value":{"title":"first"},"bindings":{"title":"second"}}"#
        ));
        assert_eq!(eval_text("title", None, None), "second");
    }

    #[test]
    fn empty_result_maps_and_nonobject_state_keep_existing_behavior() {
        let _guard = test_lock();
        *lock_view_state() = serde_json::json!(42);
        let path = std::iter::repeat_n("child", 127)
            .collect::<Vec<_>>()
            .join(".");
        assert!(!store_result_json(
            &serde_json::json!({"bindings": {path: true}}).to_string()
        ));
        assert_eq!(*lock_view_state(), serde_json::json!(42));
        assert!(store_result_json(r#"{"value":1,"bindings":null}"#));
        assert_eq!(*lock_view_state(), serde_json::json!(42));
        assert!(store_result_json(r#"{"value":{},"bindings":{}}"#));
        assert_eq!(*lock_view_state(), serde_json::json!({}));
        *lock_view_state() = serde_json::json!(42);
        assert!(store_result_json(r#"{"bindings":{"title":"created"}}"#));
        assert_eq!(*lock_view_state(), serde_json::json!({"title":"created"}));
    }
}
