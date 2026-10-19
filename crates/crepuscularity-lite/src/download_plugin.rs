//! Native download manager used by the Motrix example.
//!
//! This is intentionally small and self-contained: it stores task state in Rust,
//! downloads with a blocking HTTP client on background threads, and exposes a
//! Motrix-shaped task list over the bridge.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::header::{
    HeaderMap, HeaderValue, CONTENT_LENGTH, CONTENT_RANGE, ETAG, IF_RANGE, RANGE,
};
use serde_json::{json, Value};

use crate::bridge::{BridgeError, Capability, NativePlugin};

const STATUS_ACTIVE: &str = "active";
const STATUS_WAITING: &str = "waiting";
const STATUS_PAUSED: &str = "paused";
const STATUS_ERROR: &str = "error";
const STATUS_COMPLETE: &str = "complete";
const STATUS_REMOVED: &str = "removed";

fn parse_byte_count(value: &str) -> Option<u64> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse().ok())
        .flatten()
}

fn write_download_chunk(
    writer: &mut impl Write,
    mut bytes: &[u8],
    completed: &mut u64,
) -> std::io::Result<()> {
    while !bytes.is_empty() {
        match writer.write(bytes) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(written) => {
                *completed += written as u64;
                bytes = &bytes[written..];
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn resumed_response_length(headers: &HeaderMap, offset: u64) -> Result<(u64, u64), String> {
    let invalid = || "invalid Content-Range for resumed download".to_string();
    let range = headers
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(invalid)?;
    let (bounds, total) = range
        .strip_prefix("bytes ")
        .and_then(|v| v.split_once('/'))
        .ok_or_else(invalid)?;
    let (start, end) = bounds.split_once('-').ok_or_else(invalid)?;
    let start = parse_byte_count(start).ok_or_else(invalid)?;
    let end = parse_byte_count(end).ok_or_else(invalid)?;
    let length = end
        .checked_sub(start)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(invalid)?;
    let complete_length = end.checked_add(1).ok_or_else(invalid)?;
    let total_length = if total == "*" {
        0
    } else {
        parse_byte_count(total).ok_or_else(invalid)?
    };
    if start != offset || (total != "*" && total_length < complete_length) {
        return Err(invalid());
    }
    if let Some(content_length) = headers.get(CONTENT_LENGTH) {
        if content_length.to_str().ok().and_then(parse_byte_count) != Some(length) {
            return Err(invalid());
        }
    }
    Ok((complete_length, total_length))
}

#[derive(Debug, Clone)]
struct DownloadTask {
    gid: String,
    status: String,
    url: String,
    filename: String,
    output_path: PathBuf,
    completed_length: u64,
    total_length: u64,
    download_speed: u64,
    paused: bool,
    removed: bool,
    worker_running: bool,
    etag: Option<HeaderValue>,
    error: Option<String>,
}

impl DownloadTask {
    fn snapshot(&self) -> Value {
        json!({
            "gid": self.gid,
            "status": self.status,
            "url": self.url,
            "filename": self.filename,
            "completedLength": self.completed_length,
            "totalLength": self.total_length,
            "downloadSpeed": self.download_speed,
            "outputPath": self.output_path.to_string_lossy(),
            "error": self.error,
        })
    }
}

#[derive(Debug, Default)]
struct DownloadManager {
    tasks: HashMap<String, Arc<Mutex<DownloadTask>>>,
    order: Vec<String>,
    revision: u64,
}

impl DownloadManager {
    fn insert(&mut self, task: DownloadTask) -> Arc<Mutex<DownloadTask>> {
        let gid = task.gid.clone();
        let task = Arc::new(Mutex::new(task));
        self.order.push(gid.clone());
        self.tasks.insert(gid, task.clone());
        self.revision = self.revision.saturating_add(1);
        task
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    fn get(&self, gid: &str) -> Option<Arc<Mutex<DownloadTask>>> {
        self.tasks.get(gid).cloned()
    }

    fn snapshot(&self) -> Vec<Value> {
        self.order
            .iter()
            .filter_map(|gid| self.tasks.get(gid))
            .filter_map(|task| task.lock().ok().map(|t| t.snapshot()))
            .collect()
    }
}

pub struct DownloadPlugin {
    manager: Arc<Mutex<DownloadManager>>,
}

impl Default for DownloadPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadPlugin {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(DownloadManager::default())),
        }
    }

    fn task_dir() -> PathBuf {
        dirs::download_dir()
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(std::env::temp_dir)
            .join("crepuscularity-lite")
            .join("downloads")
    }

    fn make_filename(url: &str, requested: Option<&str>) -> String {
        let raw = if let Some(name) = requested.filter(|s| !s.trim().is_empty()) {
            name.to_string()
        } else {
            reqwest::Url::parse(url)
                .ok()
                .and_then(|url| {
                    url.path_segments()?
                        .rfind(|part| !part.is_empty())
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| "download.bin".to_string())
        };
        raw.replace(['/', '\\', '\0'], "_")
    }

    fn create_task(
        &self,
        url: &str,
        filename: Option<&str>,
    ) -> Result<Arc<Mutex<DownloadTask>>, BridgeError> {
        let valid_url = reqwest::Url::parse(url)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
        if valid_url.is_none() {
            return Err(BridgeError::new(
                "invalid_url",
                "a valid http(s) URL with a host is required",
            ));
        }
        let gid = Self::generate_gid();
        let output_dir = Self::task_dir();
        let display_name = Self::make_filename(url, filename);
        let output_path = output_dir.join(format!("{gid}-{display_name}"));
        if !output_path.starts_with(&output_dir) {
            return Err(BridgeError::new(
                "path_escape",
                "download filename escapes output directory",
            ));
        }
        let task = DownloadTask {
            gid,
            status: STATUS_WAITING.to_string(),
            url: url.to_string(),
            filename: display_name,
            output_path,
            completed_length: 0,
            total_length: 0,
            download_speed: 0,
            paused: false,
            removed: false,
            worker_running: false,
            etag: None,
            error: None,
        };
        let task = {
            let mut guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
            let task = guard.insert(task);
            let _ = std::fs::create_dir_all(Self::task_dir());
            task
        };
        Ok(task)
    }

    fn generate_gid() -> String {
        static LAST_GID: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut previous = LAST_GID.load(Ordering::Relaxed);
        let gid = loop {
            let next = nanos.max(previous.saturating_add(1));
            match LAST_GID.compare_exchange_weak(
                previous,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break next,
                Err(current) => previous = current,
            }
        };
        format!("{gid:016X}")
    }

    fn start_download(&self, task: Arc<Mutex<DownloadTask>>) {
        {
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            if t.worker_running || t.removed || t.paused || t.status == STATUS_COMPLETE {
                return;
            }
            t.worker_running = true;
        }
        let manager = self.manager.clone();
        thread::spawn(move || loop {
            let result = Self::run_download(manager.clone(), task.clone());
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            if !t.paused && !t.removed && t.status == STATUS_PAUSED && result.is_ok() {
                drop(t);
                continue;
            }
            let mut state_changed = false;
            if t.removed {
                if let Err(err) = std::fs::remove_file(&t.output_path) {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        t.error = Some(format!("remove download failed: {err}"));
                        state_changed = true;
                    }
                }
                t.download_speed = 0;
            }
            t.worker_running = false;
            if let Err(err) = result {
                state_changed = true;
                if !t.removed {
                    t.status = if t.paused {
                        STATUS_PAUSED
                    } else {
                        STATUS_ERROR
                    }
                    .to_string();
                    t.error = Some(err);
                    t.download_speed = 0;
                }
            }
            drop(t);
            if state_changed {
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
            }
            break;
        });
    }

    fn run_download(
        manager: Arc<Mutex<DownloadManager>>,
        task: Arc<Mutex<DownloadTask>>,
    ) -> Result<(), String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("client build failed: {e}"))?;

        loop {
            let (url, output_path, etag) = {
                let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
                if t.removed {
                    return Ok(());
                }
                if t.paused {
                    t.status = STATUS_PAUSED.to_string();
                    return Ok(());
                }
                t.status = STATUS_ACTIVE.to_string();
                t.error = None;
                (t.url.clone(), t.output_path.clone(), t.etag.clone())
            };

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create_dir_all failed: {e}"))?;
            }

            let existing_metadata = match std::fs::symlink_metadata(&output_path) {
                Ok(metadata) if metadata.is_file() => Some(metadata),
                Ok(_) => return Err("download output must be a regular file".to_string()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                Err(err) => return Err(format!("download output metadata failed: {err}")),
            };
            let mut existing_len = existing_metadata
                .as_ref()
                .map_or(0, |metadata| metadata.len());
            let mut req = client.get(&url);
            if existing_len > 0 {
                req = req.header(RANGE, format!("bytes={existing_len}-"));
                if let Some(etag) = &etag {
                    req = req.header(IF_RANGE, etag.clone());
                }
            }

            let response = req.send().map_err(|e| format!("request failed: {e}"))?;

            if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing_len > 0 {
                let total = response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("bytes */"))
                    .and_then(parse_byte_count);
                if total == Some(existing_len) {
                    if let (Some(previous), Some(current)) = (&etag, response.headers().get(ETAG)) {
                        if previous != current {
                            return Err("resumed download ETag changed".to_string());
                        }
                    }
                    let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
                    if !t.removed {
                        t.status = STATUS_COMPLETE.to_string();
                    }
                    t.completed_length = existing_len;
                    t.total_length = existing_len;
                    t.download_speed = 0;
                    drop(t);
                    manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .bump_revision();
                    return Ok(());
                }
            }

            if !response.status().is_success()
                && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            {
                return Err(format!("http status {}", response.status()));
            }

            let resume_supported = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
            if resume_supported {
                if let (Some(previous), Some(current)) = (&etag, response.headers().get(ETAG)) {
                    if previous != current {
                        return Err("resumed download ETag changed".to_string());
                    }
                }
            }
            let response_etag = response
                .headers()
                .get(ETAG)
                .filter(|value| {
                    let bytes = value.as_bytes();
                    bytes.len() >= 2 && bytes.first() == Some(&b'"') && bytes.last() == Some(&b'"')
                })
                .cloned();
            if !resume_supported {
                existing_len = 0;
            }
            let (response_end, total_length) = if resume_supported {
                resumed_response_length(response.headers(), existing_len)?
            } else {
                let length = response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_byte_count)
                    .unwrap_or(0);
                (length, length)
            };

            let file = OpenOptions::new()
                .create_new(existing_metadata.is_none())
                .append(resume_supported)
                .write(true)
                .open(&output_path)
                .map_err(|e| format!("open output failed: {e}"))?;
            if let Some(previous) = &existing_metadata {
                let current = file
                    .metadata()
                    .map_err(|e| format!("open output metadata failed: {e}"))?;
                let path_metadata = std::fs::symlink_metadata(&output_path)
                    .map_err(|e| format!("output path metadata failed: {e}"))?;
                if !current.is_file() || !path_metadata.is_file() || previous.len() != current.len()
                {
                    return Err("download output changed during request".to_string());
                }
                // shortcut: file identity is checked only on Unix; add native IDs for other hosts.
                #[cfg(unix)]
                if previous.dev() != current.dev()
                    || previous.ino() != current.ino()
                    || path_metadata.dev() != current.dev()
                    || path_metadata.ino() != current.ino()
                {
                    return Err("download output changed during request".to_string());
                }
            }
            if !resume_supported {
                file.set_len(0)
                    .map_err(|e| format!("truncate output failed: {e}"))?;
            }

            {
                let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
                t.completed_length = existing_len;
                t.total_length = total_length;
                t.download_speed = 0;
                t.etag = if resume_supported {
                    etag.or(response_etag)
                } else {
                    response_etag
                };
            }

            let completed = Self::transfer_loop(
                manager.clone(),
                task.clone(),
                response,
                file,
                existing_len,
                response_end,
                output_path,
            )?;

            if !completed {
                return Ok(());
            }

            if response_end < total_length || (resume_supported && total_length == 0) {
                continue;
            }
            if let Ok(mut t) = task.lock() {
                if !t.removed {
                    t.status = STATUS_COMPLETE.to_string();
                }
                t.download_speed = 0;
                t.error = None;
            }
            if let Ok(mut m) = manager.lock() {
                m.bump_revision();
            }

            return Ok(());
        }
    }

    fn transfer_loop(
        manager: Arc<Mutex<DownloadManager>>,
        task: Arc<Mutex<DownloadTask>>,
        mut body: reqwest::blocking::Response,
        mut file: std::fs::File,
        mut existing_len: u64,
        total_length: u64,
        output_path: std::path::PathBuf,
    ) -> Result<bool, String> {
        let mut buf = [0u8; 8192];
        let mut last_tick = Instant::now();
        let mut last_completed = existing_len;
        loop {
            let (paused, removed) = {
                let t = task.lock().unwrap_or_else(|e| e.into_inner());
                (t.paused, t.removed)
            };
            if removed {
                let _ = std::fs::remove_file(&output_path);
                if let Ok(mut t) = task.lock() {
                    t.status = STATUS_REMOVED.to_string();
                    t.download_speed = 0;
                }
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
                return Ok(false);
            }
            if paused {
                if let Ok(mut t) = task.lock() {
                    t.status = STATUS_PAUSED.to_string();
                    t.download_speed = 0;
                }
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
                return Ok(false);
            }

            let read = body
                .read(&mut buf)
                .map_err(|e| format!("read failed: {e}"))?;
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            if t.paused || t.removed {
                drop(t);
                continue;
            }
            if read == 0 {
                break;
            }
            let next_length = existing_len
                .checked_add(read as u64)
                .ok_or_else(|| "download byte count overflow".to_string())?;
            if total_length > 0 && next_length > total_length {
                return Err("download body exceeds declared length".to_string());
            }
            let write_result = write_download_chunk(&mut file, &buf[..read], &mut existing_len);
            t.completed_length = existing_len;
            if total_length == 0 {
                t.total_length = existing_len;
            }
            drop(t);
            write_result.map_err(|e| format!("write failed: {e}"))?;
            let elapsed = last_tick.elapsed().as_secs_f32();
            if elapsed >= 0.25 {
                let delta = existing_len.saturating_sub(last_completed);
                let speed = if elapsed > 0.0 {
                    (delta as f32 / elapsed) as u64
                } else {
                    0
                };
                if let Ok(mut t) = task.lock() {
                    t.download_speed = speed;
                }
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
                last_tick = Instant::now();
                last_completed = existing_len;
            }
        }

        if total_length > 0 && existing_len != total_length {
            return Err("download body is shorter than declared length".to_string());
        }
        if let Ok(mut t) = task.lock() {
            t.completed_length = existing_len;
            t.total_length = t.total_length.max(existing_len);
        }

        Ok(true)
    }
}

impl NativePlugin for DownloadPlugin {
    fn id(&self) -> &'static str {
        "download"
    }

    fn capability(&self) -> Capability {
        Capability::Download
    }

    fn methods(&self) -> &'static [&'static str] {
        &["addUri", "list", "pause", "resume", "remove"]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "addUri" => self.handle_add_uri(payload),
            "list" => self.handle_list(payload),
            "pause" => self.handle_pause(payload),
            "resume" => self.handle_resume(payload),
            "remove" => self.handle_remove(payload),
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

impl DownloadPlugin {
    fn handle_add_uri(&self, payload: &Value) -> Result<Value, BridgeError> {
        let url = payload.get("url").and_then(Value::as_str).ok_or_else(|| {
            BridgeError::with_details(
                "invalid_payload",
                "addUri requires string field \"url\"",
                payload.clone(),
            )
        })?;
        let filename = match payload.get("filename") {
            None => None,
            Some(Value::String(filename)) => Some(filename.as_str()),
            _ => {
                return Err(BridgeError::new(
                    "invalid_payload",
                    "filename must be a string when supplied",
                ))
            }
        };
        let task = self.create_task(url, filename)?;
        self.start_download(task.clone());
        let snapshot = task.lock().unwrap_or_else(|e| e.into_inner()).snapshot();
        Ok(snapshot)
    }

    fn handle_list(&self, _payload: &Value) -> Result<Value, BridgeError> {
        let guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
        Ok(json!({ "revision": guard.revision, "tasks": guard.snapshot() }))
    }

    fn handle_pause(&self, payload: &Value) -> Result<Value, BridgeError> {
        let gid = payload.get("gid").and_then(Value::as_str).ok_or_else(|| {
            BridgeError::with_details(
                "invalid_payload",
                "pause requires string field \"gid\"",
                payload.clone(),
            )
        })?;
        let guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
        let task = guard.get(gid).ok_or_else(|| {
            BridgeError::new("not_found", format!("unknown download gid {gid:?}"))
        })?;
        drop(guard);
        {
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            if t.removed || t.status == STATUS_COMPLETE {
                return Err(BridgeError::new(
                    "invalid_state",
                    "cannot pause a removed or completed download",
                ));
            }
            t.paused = true;
            if !t.worker_running {
                t.status = STATUS_PAUSED.to_string();
                t.download_speed = 0;
            }
        }
        if let Ok(mut m) = self.manager.lock() {
            m.bump_revision();
        }
        Ok(json!({"gid": gid, "paused": true}))
    }

    fn handle_resume(&self, payload: &Value) -> Result<Value, BridgeError> {
        let gid = payload.get("gid").and_then(Value::as_str).ok_or_else(|| {
            BridgeError::with_details(
                "invalid_payload",
                "resume requires string field \"gid\"",
                payload.clone(),
            )
        })?;
        let guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
        let task = guard.get(gid).ok_or_else(|| {
            BridgeError::new("not_found", format!("unknown download gid {gid:?}"))
        })?;
        drop(guard);
        {
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            if t.removed || t.status == STATUS_COMPLETE {
                return Err(BridgeError::new(
                    "invalid_state",
                    "cannot resume a removed or completed download",
                ));
            }
            t.paused = false;
            if !t.worker_running {
                t.status = STATUS_WAITING.to_string();
            }
        }
        if let Ok(mut m) = self.manager.lock() {
            m.bump_revision();
        }
        self.start_download(task.clone());
        Ok(json!({"gid": gid, "resumed": true}))
    }

    fn handle_remove(&self, payload: &Value) -> Result<Value, BridgeError> {
        let gid = payload.get("gid").and_then(Value::as_str).ok_or_else(|| {
            BridgeError::with_details(
                "invalid_payload",
                "remove requires string field \"gid\"",
                payload.clone(),
            )
        })?;
        let guard = self.manager.lock().unwrap_or_else(|e| e.into_inner());
        let task = guard.get(gid).ok_or_else(|| {
            BridgeError::new("not_found", format!("unknown download gid {gid:?}"))
        })?;
        drop(guard);
        if let Ok(mut t) = task.lock() {
            if !t.worker_running {
                match std::fs::remove_file(&t.output_path) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        return Err(BridgeError::new(
                            "fs_error",
                            format!("remove download failed: {err}"),
                        ))
                    }
                }
            }
            t.removed = true;
            t.status = STATUS_REMOVED.to_string();
            t.download_speed = 0;
        }
        if let Ok(mut m) = self.manager.lock() {
            m.bump_revision();
        }
        Ok(json!({"gid": gid, "removed": true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::NativePlugin;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn invalid_filenames_do_not_create_download_tasks() {
        let plugin = DownloadPlugin::new();
        for filename in [Value::Null, json!(5), json!(false), json!([]), json!({})] {
            assert_eq!(
                plugin
                    .invoke(
                        "addUri",
                        &json!({"url": "http://127.0.0.1/file.bin", "filename": filename})
                    )
                    .unwrap_err()
                    .code,
                "invalid_payload"
            );
            let manager = plugin.manager.lock().unwrap();
            assert!(manager.tasks.is_empty());
            assert!(manager.order.is_empty());
            assert_eq!(manager.revision, 0);
        }
    }

    #[test]
    fn failed_idle_removal_preserves_the_task_and_can_be_retried() {
        let plugin = DownloadPlugin::new();
        let task = plugin
            .create_task("http://127.0.0.1/file.bin", None)
            .unwrap();
        let (gid, path) = {
            let mut t = task.lock().unwrap();
            t.status = STATUS_PAUSED.to_owned();
            t.paused = true;
            (t.gid.clone(), t.output_path.clone())
        };
        std::fs::create_dir(&path).unwrap();
        let sentinel = path.join("keep.txt");
        std::fs::write(&sentinel, b"keep").unwrap();
        let before = plugin.invoke("list", &json!({})).unwrap();
        assert_eq!(
            plugin
                .invoke("remove", &json!({"gid": gid}))
                .unwrap_err()
                .code,
            "fs_error"
        );
        assert_eq!(plugin.invoke("list", &json!({})).unwrap(), before);
        assert!(!task.lock().unwrap().removed);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        std::fs::remove_file(&sentinel).unwrap();
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"download").unwrap();
        plugin.invoke("remove", &json!({"gid": gid})).unwrap();
        assert!(!path.exists());
        assert!(task.lock().unwrap().removed);
    }

    #[test]
    fn partial_write_failures_preserve_the_actual_byte_count() {
        struct FailingWriter {
            interrupted: bool,
            bytes: Vec<u8>,
        }
        impl Write for FailingWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                if !self.bytes.is_empty() {
                    return Err(std::io::Error::other("disk failure"));
                }
                self.bytes.extend_from_slice(&bytes[..3]);
                Ok(3)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = FailingWriter {
            interrupted: false,
            bytes: Vec::new(),
        };
        let mut completed = 4;
        assert!(write_download_chunk(&mut writer, b"abcdef", &mut completed).is_err());
        assert_eq!(completed, 7);
        assert_eq!(writer.bytes, b"abc");
        let mut output = Vec::new();
        write_download_chunk(&mut output, b"def", &mut completed).unwrap();
        assert_eq!(completed, 10);
        assert_eq!(output, b"def");
        let mut full = std::io::Cursor::new(&mut [] as &mut [u8]);
        assert_eq!(
            write_download_chunk(&mut full, b"x", &mut completed)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WriteZero
        );
        assert_eq!(completed, 10);
    }

    #[test]
    fn pause_rejects_terminal_tasks_and_updates_idle_tasks() {
        let plugin = DownloadPlugin::new();
        for status in [STATUS_COMPLETE, STATUS_REMOVED] {
            let task = plugin
                .create_task("http://127.0.0.1/file.bin", None)
                .unwrap();
            let gid = {
                let mut t = task.lock().unwrap();
                t.status = status.to_owned();
                t.removed = status == STATUS_REMOVED;
                t.gid.clone()
            };
            let before = plugin.invoke("list", &json!({})).unwrap();
            assert_eq!(
                plugin
                    .invoke("pause", &json!({"gid": gid}))
                    .unwrap_err()
                    .code,
                "invalid_state"
            );
            assert_eq!(plugin.invoke("list", &json!({})).unwrap(), before);
            assert!(!task.lock().unwrap().paused);
        }
        let task = plugin
            .create_task("http://127.0.0.1/file.bin", None)
            .unwrap();
        let gid = task.lock().unwrap().gid.clone();
        plugin.invoke("pause", &json!({"gid": gid})).unwrap();
        let t = task.lock().unwrap();
        assert!(t.paused);
        assert_eq!(t.status, STATUS_PAUSED);
        assert_eq!(t.download_speed, 0);
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        request
    }

    #[cfg(unix)]
    #[test]
    fn outputs_replaced_by_symlinks_during_requests_are_not_truncated() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let path = task.lock().unwrap().output_path.clone();
        let target = path.with_extension("original");
        std::fs::write(&path, b"partial").unwrap();
        std::fs::write(&target, b"keep").unwrap();
        let server_path = path.clone();
        let server_target = target.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            std::fs::remove_file(&server_path).unwrap();
            std::os::unix::fs::symlink(&server_target, &server_path).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew")
                .unwrap();
        });
        let result = DownloadPlugin::run_download(plugin.manager.clone(), task);
        server.join().unwrap();
        let contents = std::fs::read(&target).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&target).unwrap();
        assert!(result.unwrap_err().contains("output changed"));
        assert_eq!(contents, b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn fresh_outputs_created_during_requests_are_not_overwritten() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let path = task.lock().unwrap().output_path.clone();
        let target = path.with_extension("original");
        std::fs::write(&target, b"keep").unwrap();
        let server_path = path.clone();
        let server_target = target.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            std::os::unix::fs::symlink(&server_target, &server_path).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew")
                .unwrap();
        });
        let result = DownloadPlugin::run_download(plugin.manager.clone(), task);
        server.join().unwrap();
        let contents = std::fs::read(&target).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&target).unwrap();
        assert!(result.unwrap_err().contains("open output failed"));
        assert_eq!(contents, b"keep");
    }

    #[test]
    fn download_ids_are_unique_across_concurrent_creators() {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                thread::spawn(|| {
                    (0..2000)
                        .map(|_| DownloadPlugin::generate_gid())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let ids: std::collections::HashSet<_> = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(ids.len(), 16000);
        assert!(ids
            .iter()
            .all(|id| id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit())));
    }

    #[cfg(unix)]
    #[test]
    fn download_outputs_do_not_follow_symlinks() {
        let plugin = DownloadPlugin::new();
        let task = plugin
            .create_task("http://127.0.0.1/file.bin", None)
            .unwrap();
        let path = task.lock().unwrap().output_path.clone();
        let target = path.with_extension("original");
        std::fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        task.lock().unwrap().url = "http://".to_owned();
        let result = DownloadPlugin::run_download(plugin.manager.clone(), task);
        let contents = std::fs::read(&target).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&target).unwrap();
        assert!(result.unwrap_err().contains("regular file"));
        assert_eq!(contents, b"keep");
    }

    #[test]
    fn download_outputs_reject_directories_before_requesting() {
        let plugin = DownloadPlugin::new();
        let task = plugin
            .create_task("http://127.0.0.1/file.bin", None)
            .unwrap();
        let path = task.lock().unwrap().output_path.clone();
        std::fs::create_dir(&path).unwrap();
        task.lock().unwrap().url = "http://".to_owned();
        let result = DownloadPlugin::run_download(plugin.manager.clone(), task);
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
        assert!(result.unwrap_err().contains("regular file"));
    }

    #[test]
    fn invalid_download_urls_do_not_create_tasks() {
        let plugin = DownloadPlugin::new();
        for url in [
            "http://",
            "https://?q=1",
            "http://exa mple.test/file",
            "file:///tmp/file",
            "ftp://example.test/file",
        ] {
            assert_eq!(
                plugin
                    .invoke("addUri", &json!({"url": url}))
                    .unwrap_err()
                    .code,
                "invalid_url",
                "{url}"
            );
            let manager = plugin.manager.lock().unwrap();
            assert!(manager.tasks.is_empty());
            assert!(manager.order.is_empty());
            assert_eq!(manager.revision, 0);
        }
    }

    #[test]
    fn download_filenames_use_url_paths_without_query_or_fragment() {
        for (url, expected) in [
            (
                "https://example.test/releases/file.zip?token=a/b#section",
                "file.zip",
            ),
            ("https://example.test/file.zip#section", "file.zip"),
            ("https://example.test/?filename=file.zip", "download.bin"),
            ("https://example.test", "download.bin"),
            ("https://example.test/releases/", "releases"),
        ] {
            assert_eq!(DownloadPlugin::make_filename(url, None), expected, "{url}");
            assert_eq!(DownloadPlugin::make_filename(url, Some(" ")), expected);
        }
        assert_eq!(
            DownloadPlugin::make_filename(
                "https://example.test/file.zip",
                Some("custom/file\\name\0.zip")
            ),
            "custom_file_name_.zip"
        );
    }

    #[test]
    fn short_partial_ranges_continue_until_the_resource_is_complete() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for (offset, response) in [
                (4, &b"HTTP/1.1 206 Partial Content\r\nETag: \"stable\"\r\nContent-Range: bytes 4-5/8\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab"[..]),
                (6, &b"HTTP/1.1 206 Partial Content\r\nETag: \"stable\"\r\nContent-Range: bytes 6-7/8\r\nContent-Length: 2\r\nConnection: close\r\n\r\ncd"[..]),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
                assert!(request.contains(&format!("range: bytes={offset}-\r\n")));
                if offset == 6 { assert!(request.contains("if-range: \"stable\"\r\n")); }
                stream.write_all(response).unwrap();
            }
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let path = task.lock().unwrap().output_path.clone();
        std::fs::write(&path, b"keep").unwrap();
        DownloadPlugin::run_download(plugin.manager.clone(), task.clone()).unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"keepabcd");
        std::fs::remove_file(&path).unwrap();
        let t = task.lock().unwrap();
        assert_eq!(t.status, STATUS_COMPLETE);
        assert_eq!(t.completed_length, 8);
        assert_eq!(t.total_length, 8);
    }

    #[test]
    fn unknown_total_ranges_finish_only_after_confirming_the_file_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for (offset, response) in [
                (4, &b"HTTP/1.1 206 Partial Content\r\nETag: \"stable\"\r\nContent-Range: bytes 4-5/*\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab"[..]),
                (6, &b"HTTP/1.1 416 Range Not Satisfiable\r\nETag: \"stable\"\r\nContent-Range: bytes */6\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..]),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
                assert!(request.contains(&format!("range: bytes={offset}-\r\n")));
                if offset == 6 { assert!(request.contains("if-range: \"stable\"\r\n")); }
                stream.write_all(response).unwrap();
            }
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let path = task.lock().unwrap().output_path.clone();
        std::fs::write(&path, b"keep").unwrap();
        DownloadPlugin::run_download(plugin.manager.clone(), task.clone()).unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"keepab");
        std::fs::remove_file(&path).unwrap();
        let t = task.lock().unwrap();
        assert_eq!(t.status, STATUS_COMPLETE);
        assert_eq!(t.completed_length, 6);
        assert_eq!(t.total_length, 6);
    }

    #[test]
    fn paused_body_bytes_are_discarded_and_resume_uses_the_saved_offset() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            read_http_request(&mut first);
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc")
                .unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let _ = first.write_all(b"def");
            drop(first);
            let (mut second, _) = listener.accept().unwrap();
            let request = String::from_utf8(read_http_request(&mut second)).unwrap();
            assert!(
                request.to_ascii_lowercase().contains("range: bytes=3-\r\n"),
                "{request}"
            );
            second.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 3-5/6\r\nContent-Length: 3\r\nConnection: close\r\n\r\ndef").unwrap();
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let gid = task.lock().unwrap().gid.clone();
        plugin.start_download(task.clone());
        for _ in 0..100 {
            if task.lock().unwrap().completed_length == 3 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(task.lock().unwrap().completed_length, 3);
        plugin.invoke("pause", &json!({"gid": gid})).unwrap();
        release_tx.send(()).unwrap();
        for _ in 0..100 {
            if !task.lock().unwrap().worker_running {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        {
            let t = task.lock().unwrap();
            assert!(!t.worker_running);
            assert_eq!(t.status, STATUS_PAUSED);
            assert_eq!(t.completed_length, 3);
            assert_eq!(std::fs::read(&t.output_path).unwrap(), b"abc");
        }
        plugin.invoke("resume", &json!({"gid": gid})).unwrap();
        server.join().unwrap();
        for _ in 0..100 {
            if !task.lock().unwrap().worker_running {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let t = task.lock().unwrap();
        assert!(!t.worker_running);
        assert_eq!(t.status, STATUS_COMPLETE);
        assert_eq!(t.completed_length, 6);
        assert_eq!(std::fs::read(&t.output_path).unwrap(), b"abcdef");
        std::fs::remove_file(&t.output_path).unwrap();
    }

    #[test]
    fn removing_idle_downloads_deletes_their_output() {
        let plugin = DownloadPlugin::new();
        for status in [STATUS_PAUSED, STATUS_COMPLETE] {
            let task = plugin
                .create_task("http://127.0.0.1/file.bin", None)
                .unwrap();
            let (gid, path) = {
                let mut t = task.lock().unwrap();
                t.status = status.to_owned();
                (t.gid.clone(), t.output_path.clone())
            };
            std::fs::write(&path, b"downloaded").unwrap();
            plugin.invoke("remove", &json!({"gid": gid})).unwrap();
            assert!(!path.exists());
            assert_eq!(task.lock().unwrap().status, STATUS_REMOVED);
            plugin.invoke("remove", &json!({"gid": gid})).unwrap();
            assert_eq!(
                plugin
                    .invoke("resume", &json!({"gid": gid}))
                    .unwrap_err()
                    .code,
                "invalid_state"
            );
        }
    }

    #[test]
    fn repeated_resume_calls_keep_a_single_worker() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let server_listener = listener.try_clone().unwrap();
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = server_listener.accept().unwrap();
            accepted_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nend")
                .unwrap();
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let gid = task.lock().unwrap().gid.clone();
        plugin.start_download(task.clone());
        accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..20 {
            plugin.invoke("pause", &json!({"gid": gid})).unwrap();
            plugin.invoke("resume", &json!({"gid": gid})).unwrap();
        }
        release_tx.send(()).unwrap();
        server.join().unwrap();
        for _ in 0..100 {
            if !task.lock().unwrap().worker_running {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let t = task.lock().unwrap();
        assert!(!t.worker_running);
        assert_eq!(t.status, STATUS_COMPLETE);
        assert_eq!(std::fs::read(&t.output_path).unwrap(), b"end");
        std::fs::remove_file(&t.output_path).unwrap();
        drop(t);
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(
            plugin
                .invoke("resume", &json!({"gid": gid}))
                .unwrap_err()
                .code,
            "invalid_state"
        );
    }

    #[test]
    fn controls_during_failed_requests_preserve_the_requested_state() {
        for command in ["remove", "pause"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
            let server_listener = listener.try_clone().unwrap();
            let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let server = thread::spawn(move || {
                let (mut stream, _) = server_listener.accept().unwrap();
                accepted_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                read_http_request(&mut stream);
                stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 3\r\nConnection: close\r\n\r\nend")
                .unwrap();
            });
            let plugin = DownloadPlugin::new();
            let task = plugin.create_task(&url, None).unwrap();
            let gid = task.lock().unwrap().gid.clone();
            let path = task.lock().unwrap().output_path.clone();
            std::fs::write(&path, b"partial").unwrap();
            plugin.start_download(task.clone());
            accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            plugin.invoke(command, &json!({"gid": gid})).unwrap();
            release_tx.send(()).unwrap();
            server.join().unwrap();
            for _ in 0..100 {
                if !task.lock().unwrap().worker_running {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let t = task.lock().unwrap();
            assert!(!t.worker_running);
            if command == "pause" {
                assert_eq!(t.status, STATUS_PAUSED);
                assert!(t.paused);
                assert!(t.error.is_some());
                assert_eq!(std::fs::read(&t.output_path).unwrap(), b"partial");
            } else {
                assert_eq!(t.status, STATUS_REMOVED);
                assert!(!t.output_path.exists());
            }
            drop(t);
            if command == "pause" {
                plugin.invoke("remove", &json!({"gid": gid})).unwrap();
            }
            listener.set_nonblocking(true).unwrap();
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert_eq!(
                plugin
                    .invoke("resume", &json!({"gid": gid}))
                    .unwrap_err()
                    .code,
                "invalid_state"
            );
        }
    }

    #[test]
    fn asynchronous_removal_errors_advance_the_revision() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let server_listener = listener.try_clone().unwrap();
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = server_listener.accept().unwrap();
            read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\n")
                .unwrap();
            accepted_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            stream.write_all(b"end").unwrap();
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        let gid = task.lock().unwrap().gid.clone();
        let path = task.lock().unwrap().output_path.clone();
        std::fs::write(&path, b"partial").unwrap();
        plugin.start_download(task.clone());
        accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..100 {
            if task.lock().unwrap().total_length == 3 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(task.lock().unwrap().total_length, 3);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        plugin.invoke("remove", &json!({"gid": gid})).unwrap();
        let before = plugin.manager.lock().unwrap().revision;
        release_tx.send(()).unwrap();
        server.join().unwrap();
        for _ in 0..100 {
            if !task.lock().unwrap().worker_running {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let t = task.lock().unwrap();
        assert!(!t.worker_running);
        assert_eq!(t.status, STATUS_REMOVED);
        assert!(t.output_path.is_dir());
        assert!(t.error.as_ref().unwrap().contains("remove download failed"));
        drop(t);
        std::fs::remove_dir(&path).unwrap();
        for _ in 0..100 {
            if plugin.manager.lock().unwrap().revision >= before + 2 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(plugin.manager.lock().unwrap().revision >= before + 2);
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(
            plugin
                .invoke("resume", &json!({"gid": gid}))
                .unwrap_err()
                .code,
            "invalid_state"
        );
    }

    fn transfer_response(response: &'static [u8]) -> (Result<(), String>, Vec<u8>, DownloadTask) {
        transfer_response_with_etag(response, None)
    }

    fn transfer_response_with_etag(
        response: &'static [u8],
        etag: Option<&'static str>,
    ) -> (Result<(), String>, Vec<u8>, DownloadTask) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            if let Some(etag) = etag {
                assert!(String::from_utf8(request)
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains(&format!("if-range: {etag}\r\n")));
            }
            stream.write_all(response).unwrap();
        });
        let plugin = DownloadPlugin::new();
        let task = plugin.create_task(&url, None).unwrap();
        task.lock().unwrap().etag = etag.map(|value| value.parse().unwrap());
        let path = task.lock().unwrap().output_path.clone();
        std::fs::write(&path, b"keep").unwrap();
        let result = DownloadPlugin::run_download(plugin.manager.clone(), task.clone());
        let contents = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        server.join().unwrap();
        let status = task.lock().unwrap().clone();
        (result, contents, status)
    }

    #[test]
    fn changed_etags_do_not_mix_resource_versions() {
        let (result, contents, _) = transfer_response_with_etag(b"HTTP/1.1 206 Partial Content\r\nETag: \"new\"\r\nContent-Range: bytes 4-6/7\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad", Some("\"old\""));
        assert!(result.unwrap_err().contains("ETag changed"));
        assert_eq!(contents, b"keep");
        let (result, contents, task) = transfer_response_with_etag(b"HTTP/1.1 200 OK\r\nETag: \"new\"\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew", Some("\"old\""));
        result.unwrap();
        assert_eq!(contents, b"new");
        assert_eq!(task.status, STATUS_COMPLETE);
        assert_eq!(task.etag.unwrap(), "\"new\"");
        assert_eq!(task.total_length, 3);
    }

    #[test]
    fn unsatisfied_ranges_do_not_complete_a_file_with_the_wrong_size() {
        let (result, contents, task) = transfer_response(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */8\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert!(result.is_err());
        assert_eq!(contents, b"keep");
        assert_ne!(task.status, STATUS_COMPLETE);
    }

    #[test]
    fn wrong_resume_offset_preserves_the_existing_file() {
        let (result, contents, _) = transfer_response(b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-2/3\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad");
        assert!(result.unwrap_err().contains("Content-Range"));
        assert_eq!(contents, b"keep");
    }

    #[test]
    fn resumed_chunked_bodies_must_match_the_declared_range() {
        for (response, expected) in [
            (&b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 4-6/7\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nok\r\n0\r\n\r\n"[..], &b"keepok"[..]),
            (&b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 4-6/7\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nlong\r\n0\r\n\r\n"[..], &b"keep"[..]),
        ] {
            let (result, contents, status) = transfer_response(response);
            assert!(result.is_err());
            assert_ne!(status.status, STATUS_COMPLETE);
            assert_eq!(status.completed_length, contents.len() as u64);
            assert_eq!(contents, expected);
        }
        let (result, contents, status) = transfer_response(b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 4-6/7\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nend\r\n0\r\n\r\n");
        result.unwrap();
        assert_eq!(contents, b"keepend");
        assert_eq!(status.status, STATUS_COMPLETE);
        assert_eq!(status.completed_length, contents.len() as u64);
    }

    #[test]
    fn resumed_ranges_match_the_file_offset_and_response_length() {
        let mut headers = HeaderMap::new();
        assert!(resumed_response_length(&headers, 5).is_err());
        for range in [
            "bytes 0-9/10",
            "bytes 5-4/10",
            "bytes 5-9/9",
            "bytes x-9/10",
            "bytes +5-9/10",
            "bytes 5-+9/10",
            "bytes 5-9/+10",
            "items 5-9/10",
            "bytes 5-18446744073709551615/*",
        ] {
            headers.insert(CONTENT_RANGE, range.parse().unwrap());
            assert!(resumed_response_length(&headers, 5).is_err(), "{range}");
        }
        for range in ["bytes 5-9/10", "bytes 5-9/*"] {
            headers.insert(CONTENT_RANGE, range.parse().unwrap());
            headers.insert(CONTENT_LENGTH, "5".parse().unwrap());
            assert_eq!(
                resumed_response_length(&headers, 5).unwrap(),
                (10, if range.ends_with("/*") { 0 } else { 10 })
            );
            headers.insert(CONTENT_LENGTH, "6".parse().unwrap());
            assert!(resumed_response_length(&headers, 5).is_err());
        }
    }

    #[test]
    fn pause_and_remove_release_the_manager_lookup_lock() {
        let plugin = Arc::new(DownloadPlugin::new());
        let task = plugin
            .create_task("http://127.0.0.1/file.bin", None)
            .unwrap();
        let gid = task.lock().unwrap().gid.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            plugin.invoke("pause", &json!({"gid": gid})).unwrap();
            assert!(task.lock().unwrap().paused);
            plugin.invoke("remove", &json!({"gid": gid})).unwrap();
            assert!(task.lock().unwrap().removed);
            assert_eq!(plugin.manager.lock().unwrap().revision, 3);
            sender.send(()).unwrap();
        });
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("download controls must not deadlock");
        worker.join().unwrap();
    }

    fn serve_once(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write headers");
            stream.write_all(body).expect("write body");
        });
        format!("http://{addr}/file.bin")
    }

    #[test]
    fn download_plugin_completes_real_http_transfer() {
        let plugin = DownloadPlugin::new();
        let url = serve_once(b"hello from crepuscularity-lite");
        let added = plugin
            .invoke("addUri", &json!({ "url": url }))
            .expect("addUri should succeed");
        let gid = added
            .get("gid")
            .and_then(Value::as_str)
            .expect("gid")
            .to_string();

        let mut completed = false;
        for _ in 0..60 {
            let snapshot = plugin.invoke("list", &json!({})).expect("list");
            let tasks = snapshot
                .get("tasks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(task) = tasks
                .iter()
                .find(|task| task.get("gid").and_then(Value::as_str) == Some(gid.as_str()))
            {
                if task.get("status").and_then(Value::as_str) == Some(STATUS_COMPLETE)
                    && task.get("completedLength").and_then(Value::as_u64)
                        == task.get("totalLength").and_then(Value::as_u64)
                {
                    completed = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        assert!(completed, "download never completed");
    }
}
