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
    state.last_error = parse_action_error(&result);
    state.last_result = result.clone();
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
    payload.insert(bind.to_string(), value);
    if let Some((capability, method)) = action.split_once('.') {
        return dispatch_action_json(
            &serde_json::json!({
                "kind": "plugin",
                "capability": capability,
                "method": method,
                "payload": payload,
            })
            .to_string(),
        );
    }
    if !action.is_empty() {
        payload.insert(
            "action".to_string(),
            serde_json::Value::String(action.to_string()),
        );
    }
    dispatch_action_json(&serde_json::Value::Object(payload).to_string())
}

fn merge_view_value(bind: &str, value: serde_json::Value) -> serde_json::Value {
    let mut state = lock_view_state();
    if !state.is_object() {
        *state = serde_json::json!({});
    }
    if let Some(map) = state.as_object_mut() {
        map.insert(bind.to_string(), value.clone());
    }
    serde_json::json!({
        "ok": true,
        "action": "bind",
        "value": { bind: value },
        "state": state.clone(),
    })
}

fn parse_action_error(result: &str) -> Option<String> {
    let payload = serde_json::from_str::<serde_json::Value>(result).ok()?;
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
        .unwrap_or_else(|| serde_json::json!({ "action": input }))
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
    let result = action_from_ffi(action_ptr, action_len)
        .map(|action| dispatch_action_json(action.as_ref()))
        .unwrap_or_else(|| {
            action_json(
                false,
                "",
                None,
                Some("invalid action pointer"),
                &MobileActionState::default(),
            )
        });
    copy_json_to_output(&result, output_ptr, output_len)
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
    let Some(action) = action_from_ffi(action_ptr, action_len) else {
        let result = action_json(
            false,
            "",
            None,
            Some("invalid action pointer"),
            &MobileActionState::default(),
        );
        return copy_json_to_output(&result, output_ptr, output_len);
    };
    let Some(bind) = action_from_ffi(bind_ptr, bind_len) else {
        let result = action_json(
            false,
            action.as_ref(),
            None,
            Some("invalid bind pointer"),
            &MobileActionState::default(),
        );
        return copy_json_to_output(&result, output_ptr, output_len);
    };
    let Some(value_json) = action_from_ffi(value_ptr, value_len) else {
        let result = action_json(
            false,
            action.as_ref(),
            None,
            Some("invalid value pointer"),
            &MobileActionState::default(),
        );
        return copy_json_to_output(&result, output_ptr, output_len);
    };
    copy_json_to_output(
        &dispatch_change_json(action.as_ref(), bind.as_ref(), value_json.as_ref()),
        output_ptr,
        output_len,
    )
}

#[no_mangle]
pub extern "C" fn crepus_mobile_last_result() -> *mut c_char {
    let state = lock_action_state();
    alloc_c_string(&state.last_result)
}

#[no_mangle]
pub extern "C" fn crepus_mobile_last_error() -> *mut c_char {
    let state = lock_action_state();
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
            let mut state = lock_action_state();
            state.last_error = parse_action_error(raw);
            state.last_result = raw.to_string();
            drop(state);
            if let Some(fields) = value.get("value").and_then(|value| value.as_object()) {
                let mut view_state = lock_view_state();
                if !view_state.is_object() {
                    *view_state = serde_json::json!({});
                }
                if let Some(map) = view_state.as_object_mut() {
                    for (key, value) in fields {
                        map.insert(key.clone(), value.clone());
                    }
                }
            }
            true
        }
        Err(_) => false,
    }
}

fn eval_text(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> String {
    stringify_value(&resolve_expr(expr, scope_name, scope_json))
}

fn eval_bool(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> bool {
    truthy_value(&resolve_expr(expr, scope_name, scope_json))
}

fn eval_number(expr: &str, scope_name: Option<&str>, scope_json: Option<&str>) -> f64 {
    number_value(&resolve_expr(expr, scope_name, scope_json)).unwrap_or(0.0)
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
    if let Some(rest) = trimmed.strip_prefix('!') {
        return serde_json::Value::Bool(!truthy_value(&resolve_expr(rest, scope_name, scope_json)));
    }
    for op in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some(index) = trimmed.find(op) {
            let left = resolve_path(trimmed[..index].trim(), scope_name, scope_json);
            let right = resolve_path(trimmed[index + op.len()..].trim(), scope_name, scope_json);
            return serde_json::Value::Bool(compare_values(&left, &right, op));
        }
    }
    resolve_path(trimmed, scope_name, scope_json)
}

fn resolve_path(
    expr: &str,
    scope_name: Option<&str>,
    scope_json: Option<&str>,
) -> serde_json::Value {
    if let Some(literal) = literal_value(expr) {
        return literal;
    }
    if let Some(scope) = scope_json.and_then(|value| serde_json::from_str(value).ok()) {
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
    let mut current = root;
    for segment in path.split('.') {
        match current {
            serde_json::Value::Object(map) => current = map.get(segment)?,
            serde_json::Value::Array(items) => {
                current = items.get(segment.parse::<usize>().ok()?)?
            }
            _ => return None,
        }
    }
    Some(current.clone())
}

fn literal_value(expr: &str) -> Option<serde_json::Value> {
    match expr {
        "true" => Some(serde_json::Value::Bool(true)),
        "false" => Some(serde_json::Value::Bool(false)),
        "null" => Some(serde_json::Value::Null),
        _ if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 => Some(
            serde_json::Value::String(expr[1..expr.len() - 1].to_string()),
        ),
        _ => expr
            .parse::<i64>()
            .map(|value| serde_json::json!(value))
            .or_else(|_| expr.parse::<f64>().map(|value| serde_json::json!(value)))
            .ok(),
    }
}

fn compare_values(left: &serde_json::Value, right: &serde_json::Value, op: &str) -> bool {
    if let (Some(left), Some(right)) = (number_value(left), number_value(right)) {
        return match op {
            ">=" => left >= right,
            "<=" => left <= right,
            ">" => left > right,
            "<" => left < right,
            "==" => left == right,
            "!=" => left != right,
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
    env.new_string(result).unwrap_or_default()
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
    env.new_string(result).unwrap_or_default()
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
    env.new_string(result).unwrap_or_default()
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
        .unwrap_or_default()
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
    .unwrap_or_default()
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
    env.new_string(result).unwrap_or_default()
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
    env.new_string(result).unwrap_or_default()
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
}
