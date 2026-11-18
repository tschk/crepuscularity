//! Native download manager used by the Motrix example.
//!
//! This is intentionally small and self-contained: it stores task state in Rust,
//! downloads with a blocking HTTP client on background threads, and exposes a
//! Motrix-shaped task list over the bridge.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use serde_json::{json, Value};

use crate::bridge::{BridgeError, Capability, NativePlugin};

const STATUS_ACTIVE: &str = "active";
const STATUS_WAITING: &str = "waiting";
const STATUS_PAUSED: &str = "paused";
const STATUS_ERROR: &str = "error";
const STATUS_COMPLETE: &str = "complete";
const STATUS_REMOVED: &str = "removed";

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
            let parts: Vec<&str> = url.split('/').filter(|part| !part.is_empty()).collect();
            parts
                .iter()
                .rfind(|part| !part.contains('?'))
                .copied()
                .unwrap_or("download.bin")
                .to_string()
        };
        raw.replace(['/', '\\', '\0'], "_")
    }

    fn create_task(
        &self,
        url: &str,
        filename: Option<&str>,
    ) -> Result<Arc<Mutex<DownloadTask>>, BridgeError> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(BridgeError::new(
                "invalid_url",
                "only http(s) URLs are supported",
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("{:016X}", nanos as u64 ^ ((nanos >> 32) as u64))
    }

    fn start_download(&self, task: Arc<Mutex<DownloadTask>>) {
        let manager = self.manager.clone();
        thread::spawn(move || {
            if let Err(err) = Self::run_download(manager.clone(), task.clone()) {
                if let Ok(mut t) = task.lock() {
                    t.status = STATUS_ERROR.to_string();
                    t.error = Some(err);
                }
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
            }
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

        let (url, output_path) = {
            let mut t = task.lock().unwrap_or_else(|e| e.into_inner());
            t.status = STATUS_ACTIVE.to_string();
            t.error = None;
            (t.url.clone(), t.output_path.clone())
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create_dir_all failed: {e}"))?;
        }

        let mut existing_len = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let mut req = client.get(&url);
        if existing_len > 0 {
            req = req.header(RANGE, format!("bytes={existing_len}-"));
        }

        let response = req.send().map_err(|e| format!("request failed: {e}"))?;

        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(format!("http status {}", response.status()));
        }

        let resume_supported = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !resume_supported {
            existing_len = 0;
        }
        let total_length = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|len| len + existing_len)
            .unwrap_or(existing_len);

        if let Ok(mut t) = task.lock() {
            if total_length > t.total_length {
                t.total_length = total_length;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(!resume_supported)
            .append(resume_supported)
            .write(true)
            .open(&output_path)
            .map_err(|e| format!("open output failed: {e}"))?;

        let completed = Self::transfer_loop(
            manager.clone(),
            task.clone(),
            response,
            file,
            existing_len,
            total_length,
            output_path,
        )?;

        if !completed {
            return Ok(());
        }

        if let Ok(mut t) = task.lock() {
            t.status = STATUS_COMPLETE.to_string();
            t.download_speed = 0;
            t.error = None;
        }
        if let Ok(mut m) = manager.lock() {
            m.bump_revision();
        }

        Ok(())
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
            if read == 0 {
                break;
            }
            file.write_all(&buf[..read])
                .map_err(|e| format!("write failed: {e}"))?;

            existing_len += read as u64;
            let elapsed = last_tick.elapsed().as_secs_f32();
            if elapsed >= 0.25 {
                let delta = existing_len.saturating_sub(last_completed);
                let speed = if elapsed > 0.0 {
                    (delta as f32 / elapsed) as u64
                } else {
                    0
                };
                if let Ok(mut t) = task.lock() {
                    t.completed_length = existing_len;
                    t.download_speed = speed;
                    if total_length == 0 {
                        t.total_length = existing_len;
                    }
                }
                if let Ok(mut m) = manager.lock() {
                    m.bump_revision();
                }
                last_tick = Instant::now();
                last_completed = existing_len;
            }
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
        let filename = payload.get("filename").and_then(Value::as_str);
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
        if let Ok(mut t) = task.lock() {
            t.paused = true;
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
        if let Ok(mut t) = task.lock() {
            t.paused = false;
            t.status = STATUS_WAITING.to_string();
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
        if let Ok(mut t) = task.lock() {
            t.removed = true;
            t.status = STATUS_REMOVED.to_string();
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

    fn serve_once(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let read = stream.read(&mut buf).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..read]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
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
