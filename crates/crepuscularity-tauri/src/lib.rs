use rayon::prelude::*;
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crepuscularity_core::bundle::{parse_bundle, Bundle};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use serde_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TauriVersion {
    V1,
    V2,
}

#[derive(Debug, Clone)]
pub struct TauriProject {
    root: PathBuf,
    config: PathBuf,
    version: TauriVersion,
    frontend_dist: PathBuf,
    config_value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TauriMetadata {
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub identifier: Option<String>,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TauriWindowSpec {
    pub label: String,
    pub title: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl TauriProject {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref().canonicalize().map_err(|e| e.to_string())?;
        let config = config_paths(&root)
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| format!("no Tauri config under {}", root.display()))?;
        let value = read_config(&config)?;
        let build = value
            .get("build")
            .and_then(Value::as_object)
            .ok_or_else(|| "tauri config missing build object".to_string())?;
        let (version, dist) = if let Some(dist) = build.get("frontendDist").and_then(Value::as_str)
        {
            (TauriVersion::V2, dist)
        } else if let Some(dist) = build.get("distDir").and_then(Value::as_str) {
            (TauriVersion::V1, dist)
        } else {
            return Err(
                "tauri config missing build.frontendDist (v2) or build.distDir (v1)".into(),
            );
        };
        let frontend_dist = config.parent().unwrap_or(&root).join(dist);
        // Canonicalize to resolve `..` components — Windows UNC (\\?\) paths
        // do not resolve `..` at the OS level, so bundle reads fail without this.
        let frontend_dist = std::fs::canonicalize(&frontend_dist).unwrap_or(frontend_dist);
        Ok(Self {
            root,
            config,
            version,
            frontend_dist,
            config_value: value,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn config_path(&self) -> &Path {
        &self.config
    }
    pub fn version(&self) -> TauriVersion {
        self.version
    }
    pub fn frontend_dist(&self) -> &Path {
        &self.frontend_dist
    }

    pub fn metadata(&self) -> TauriMetadata {
        let windows = self
            .config_value
            .pointer("/app/windows")
            .or_else(|| self.config_value.pointer("/tauri/windows"))
            .and_then(Value::as_array);
        TauriMetadata {
            product_name: self
                .config_value
                .get("productName")
                .and_then(Value::as_str)
                .map(str::to_string),
            version: self
                .config_value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string),
            identifier: self
                .config_value
                .get("identifier")
                .and_then(Value::as_str)
                .map(str::to_string),
            window_title: windows
                .and_then(|windows| windows.first())
                .and_then(|window| window.get("title"))
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }

    pub fn windows(&self) -> Vec<TauriWindowSpec> {
        self.config_value
            .pointer("/app/windows")
            .or_else(|| self.config_value.pointer("/tauri/windows"))
            .and_then(Value::as_array)
            .map(|windows| {
                windows
                    .iter()
                    .enumerate()
                    .map(|(index, window)| TauriWindowSpec {
                        label: window
                            .get("label")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| {
                                if index == 0 {
                                    "main".into()
                                } else {
                                    format!("window-{index}")
                                }
                            }),
                        title: window
                            .get("title")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| "Crepuscularity".into()),
                        width: window
                            .get("width")
                            .and_then(Value::as_u64)
                            .and_then(|value| u32::try_from(value).ok()),
                        height: window
                            .get("height")
                            .and_then(Value::as_u64)
                            .and_then(|value| u32::try_from(value).ok()),
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![TauriWindowSpec {
                    label: "main".into(),
                    title: self
                        .metadata()
                        .product_name
                        .unwrap_or_else(|| "Crepuscularity".into()),
                    width: None,
                    height: None,
                }]
            })
    }

    pub fn audit(&self) -> AuditReport {
        let mut uses = config_uses(&self.config_value, &self.config);
        collect_project_uses(&self.root, &mut uses);
        uses.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then(left.api.cmp(&right.api))
        });
        uses.dedup();
        AuditReport { uses }
    }

    pub fn bundle(&self) -> Result<Bundle, String> {
        let path = self.frontend_dist.join("crepus-bundle.json");
        let json = fs::read_to_string(&path)
            .map_err(|_| format!("{} is required for native conversion", path.display()))?;
        let bundle = parse_bundle(&json).map_err(|e| e.to_string())?;
        validate_bundle(&bundle)?;
        Ok(bundle)
    }

    #[cfg(feature = "native")]
    pub fn native_ir(&self) -> Result<crepuscularity_native::ViewIr, String> {
        let bundle = self.bundle()?;
        crepuscularity_native::render_from_files(
            &bundle.files,
            &bundle.entry,
            &crepuscularity_core::context::TemplateContext::new(),
        )
        .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Coverage {
    Native,
    Backend,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUse {
    pub source: String,
    pub api: String,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub uses: Vec<ApiUse>,
}

pub type CommandHandler = Arc<dyn Fn(&App, Value) -> Result<Value, String> + Send + Sync>;
type EventHandler = Arc<dyn Fn(&Event) + Send + Sync>;

pub use crepuscularity_tauri_macros::{command, generate_handler};

pub fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime().block_on(future)
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("create command runtime"))
}

pub struct Command {
    name: String,
    handler: CommandHandler,
}

impl Command {
    pub fn new(
        name: impl Into<String>,
        handler: impl Fn(&App, Value) -> Result<Value, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            handler: Arc::new(handler),
        }
    }
}

#[macro_export]
macro_rules! generate_context {
    () => {
        ()
    };
}

#[derive(Clone)]
pub struct Builder {
    commands: HashMap<String, CommandHandler>,
    events: Arc<Mutex<EventState>>,
    state: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

#[derive(Clone)]
pub struct App {
    commands: Arc<HashMap<String, CommandHandler>>,
    events: Arc<Mutex<EventState>>,
    state: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

pub type AppHandle = App;

pub trait Manager {
    fn emit_all(&self, name: impl Into<String>, payload: Value) -> Result<(), String>;
    fn get_window(&self, label: &str) -> Option<Window>;
}

pub mod async_runtime {
    pub fn spawn<T: Send + 'static>(
        future: impl std::future::Future<Output = T> + Send + 'static,
    ) -> tokio::task::JoinHandle<T> {
        super::runtime().spawn(future)
    }
}

#[derive(Clone)]
pub struct Window {
    app: App,
    label: String,
}

pub struct State<'a, T> {
    inner: Arc<T>,
    marker: PhantomData<&'a T>,
}

impl<'a, T> std::ops::Deref for State<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub name: String,
    pub payload: Value,
}

pub struct Listener {
    events: Arc<Mutex<EventState>>,
    name: String,
    id: u64,
}

#[derive(Default)]
struct EventState {
    next_id: u64,
    listeners: HashMap<String, BTreeMap<u64, EventHandler>>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            commands: HashMap::new(),
            events: Arc::new(Mutex::new(EventState::default())),
            state: HashMap::new(),
        }
    }
}

impl Builder {
    pub fn command(
        mut self,
        name: impl Into<String>,
        handler: impl Fn(Value) -> Result<Value, String> + Send + Sync + 'static,
    ) -> Self {
        self.commands
            .insert(name.into(), Arc::new(move |_, payload| handler(payload)));
        self
    }

    pub fn manage<T: Send + Sync + 'static>(mut self, state: T) -> Self {
        self.state.insert(TypeId::of::<T>(), Arc::new(state));
        self
    }

    pub fn build(self) -> App {
        App {
            commands: Arc::new(self.commands),
            events: self.events,
            state: Arc::new(self.state),
        }
    }

    pub fn invoke_handler(mut self, commands: Vec<Command>) -> Self {
        for command in commands {
            self.commands.insert(command.name, command.handler);
        }
        self
    }

    pub fn run(self, _context: ()) -> Result<App, String> {
        Ok(self.build())
    }
}

impl App {
    pub fn invoke(&self, command: &str, payload: Value) -> Result<Value, String> {
        self.commands
            .get(command)
            .ok_or_else(|| format!("unknown command {command:?}"))?(self, payload)
    }

    pub fn state<T: Send + Sync + 'static>(&self) -> Result<State<'static, T>, String> {
        let inner = self
            .state
            .get(&TypeId::of::<T>())
            .ok_or_else(|| format!("unmanaged state {}", std::any::type_name::<T>()))?
            .clone()
            .downcast::<T>()
            .map_err(|_| format!("invalid state {}", std::any::type_name::<T>()))?;
        Ok(State {
            inner,
            marker: PhantomData,
        })
    }

    pub fn emit(&self, name: impl Into<String>, payload: Value) {
        let event = Event {
            name: name.into(),
            payload,
        };
        let listeners = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .listeners
            .get(&event.name)
            .map(|listeners| listeners.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for listener in listeners {
            listener(&event);
        }
    }

    pub fn emit_all(&self, name: impl Into<String>, payload: Value) -> Result<(), String> {
        self.emit(name, payload);
        Ok(())
    }

    pub fn handle(&self) -> AppHandle {
        self.clone()
    }

    pub fn get_window(&self, label: &str) -> Option<Window> {
        (label == "main").then(|| Window {
            app: self.clone(),
            label: label.to_string(),
        })
    }

    pub fn listen(
        &self,
        name: impl Into<String>,
        handler: impl Fn(&Event) + Send + Sync + 'static,
    ) -> Listener {
        let name = name.into();
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = events.next_id;
        events.next_id += 1;
        events
            .listeners
            .entry(name.clone())
            .or_default()
            .insert(id, Arc::new(handler));
        Listener {
            events: self.events.clone(),
            name,
            id,
        }
    }
}

impl Window {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn emit(&self, name: impl Into<String>, payload: Value) -> Result<(), String> {
        self.app.emit(name, payload);
        Ok(())
    }

    pub fn listen(
        &self,
        name: impl Into<String>,
        handler: impl Fn(&Event) + Send + Sync + 'static,
    ) -> Listener {
        self.app.listen(name, handler)
    }
}

impl Manager for App {
    fn emit_all(&self, name: impl Into<String>, payload: Value) -> Result<(), String> {
        self.emit_all(name, payload)
    }

    fn get_window(&self, label: &str) -> Option<Window> {
        self.get_window(label)
    }
}

impl Manager for Window {
    fn emit_all(&self, name: impl Into<String>, payload: Value) -> Result<(), String> {
        self.emit(name, payload)
    }

    fn get_window(&self, label: &str) -> Option<Window> {
        self.app.get_window(label)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(listeners) = events.listeners.get_mut(&self.name) {
            listeners.remove(&self.id);
            if listeners.is_empty() {
                events.listeners.remove(&self.name);
            }
        }
    }
}

#[cfg(feature = "native")]
pub fn plugin_request(
    plugin: &str,
    method: impl Into<String>,
    payload: Value,
) -> Result<crepuscularity_native::NativeRequest, String> {
    use crepuscularity_native::{NativeCapability, NativePluginRequest, NativeRequest};

    let capability = match plugin {
        "clipboard-manager" => NativeCapability::Clipboard,
        "dialog" => NativeCapability::DocumentPicker,
        "opener" => NativeCapability::Browser,
        "haptics" => NativeCapability::Haptics,
        "share" => NativeCapability::Share,
        _ => return Err(format!("unsupported Tauri plugin {plugin:?}")),
    };
    Ok(NativeRequest::Plugin(NativePluginRequest {
        capability,
        method: method.into(),
        payload,
    }))
}

impl AuditReport {
    pub fn native_ready(&self) -> Result<(), String> {
        let blocked = self
            .uses
            .iter()
            .filter(|use_| use_.coverage != Coverage::Native)
            .map(|use_| format!("{} ({})", use_.api, use_.source))
            .collect::<Vec<_>>();
        if blocked.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "native conversion requires adapters for: {}",
                blocked.join(", ")
            ))
        }
    }
}

fn config_paths(root: &Path) -> Vec<PathBuf> {
    [root.join("src-tauri"), root.to_path_buf()]
        .into_iter()
        .flat_map(|dir| {
            ["tauri.conf.json", "tauri.conf.json5", "Tauri.toml"]
                .into_iter()
                .map(move |name| dir.join(name))
        })
        .collect()
}

fn read_config(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
        let value: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        serde_json::to_value(value).map_err(|e| e.to_string())
    } else {
        json5::from_str(&text).map_err(|e| e.to_string())
    }
}

fn validate_bundle(bundle: &Bundle) -> Result<(), String> {
    if !bundle.files.contains_key(&bundle.entry) {
        return Err(format!("bundle entry {:?} is missing", bundle.entry));
    }
    for path in bundle.files.keys() {
        if Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(format!("bundle contains unsafe path {path:?}"));
        }
    }
    Ok(())
}

fn config_uses(config: &Value, config_path: &Path) -> Vec<ApiUse> {
    let source = config_path.display().to_string();
    let mut uses = Vec::new();
    if config.pointer("/app/trayIcon").is_some() || config.pointer("/tauri/systemTray").is_some() {
        uses.push(api_use(&source, "tray", Coverage::Unsupported));
    }
    if config
        .pointer("/app/windows")
        .and_then(Value::as_array)
        .is_some_and(|windows| windows.len() > 1)
    {
        uses.push(api_use(&source, "windows.multiple", Coverage::Native));
    }
    if config.pointer("/tauri/allowlist").is_some()
        || config.pointer("/app/security/capabilities").is_some()
    {
        uses.push(api_use(&source, "permissions", Coverage::Unsupported));
    }
    if let Some(plugins) = config.get("plugins").and_then(Value::as_object) {
        for name in plugins.keys() {
            uses.push(api_use(
                &source,
                &format!("plugin.{name}"),
                plugin_coverage(name),
            ));
        }
    }
    uses
}

fn collect_project_uses(root: &Path, uses: &mut Vec<ApiUse>) {
    let mut files_to_read = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("target" | "node_modules" | "dist" | ".git")
                ) {
                    pending.push(path);
                }
                continue;
            }
            files_to_read.push(path);
        }
    }

    let local_uses: Vec<ApiUse> = files_to_read
        .into_par_iter()
        .flat_map(|path| {
            let mut file_uses = Vec::new();
            let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
                return file_uses;
            };
            let source = path.display().to_string();
            match extension {
                "rs" => {
                    if let Ok(text) = fs::read_to_string(&path) {
                        if text.contains("#[tauri::command]") || text.contains("#[command]") {
                            file_uses.push(api_use(&source, "command", Coverage::Backend));
                        }
                        if text.contains(".emit(") || text.contains(".listen(") {
                            file_uses.push(api_use(&source, "event", Coverage::Backend));
                        }
                    }
                }
                "js" | "jsx" | "ts" | "tsx" => {
                    if let Ok(text) = fs::read_to_string(&path) {
                        for api in frontend_apis(&text) {
                            file_uses.push(api_use(&source, api, frontend_coverage(api)));
                        }
                    }
                }
                "toml" if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") => {
                    if let Ok(text) = fs::read_to_string(&path) {
                        for line in text.lines() {
                            let name = line.split('=').next().unwrap_or("").trim();
                            if let Some(plugin) = name.strip_prefix("tauri-plugin-") {
                                file_uses.push(api_use(
                                    &source,
                                    &format!("plugin.{plugin}"),
                                    plugin_coverage(plugin),
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            file_uses
        })
        .collect();

    uses.extend(local_uses);
}

fn frontend_apis(text: &str) -> Vec<&'static str> {
    [
        ("@tauri-apps/api/core", "invoke"),
        ("@tauri-apps/api/event", "event"),
        ("@tauri-apps/api/window", "window"),
        ("@tauri-apps/api/webview", "webview"),
        ("@tauri-apps/api/menu", "menu"),
        ("@tauri-apps/api/tray", "tray"),
        ("@tauri-apps/plugin-", "plugin"),
    ]
    .into_iter()
    .filter_map(move |(needle, api)| text.contains(needle).then_some(api))
    .collect()
}

fn frontend_coverage(api: &str) -> Coverage {
    match api {
        "plugin" => Coverage::Backend,
        "invoke" | "event" => Coverage::Backend,
        _ => Coverage::Unsupported,
    }
}

fn plugin_coverage(plugin: &str) -> Coverage {
    match plugin {
        "clipboard-manager" | "dialog" | "opener" | "haptics" | "share" => Coverage::Native,
        "fs" | "http" | "store" => Coverage::Backend,
        _ => Coverage::Unsupported,
    }
}

fn api_use(source: &str, api: &str, coverage: Coverage) -> ApiUse {
    ApiUse {
        source: source.to_string(),
        api: api.to_string(),
        coverage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(config_name: &str, config: &str, dist: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src-tauri")).unwrap();
        fs::create_dir_all(root.path().join(dist)).unwrap();
        fs::write(root.path().join("src-tauri").join(config_name), config).unwrap();
        fs::write(
            root.path().join(dist).join("crepus-bundle.json"),
            r#"{"entry":"main.crepus","files":{"main.crepus":"div\n  \"Hello\""}}"#,
        )
        .unwrap();
        root
    }

    #[test]
    fn reads_v1_dist_dir() {
        let root = project(
            "tauri.conf.json",
            r#"{"build":{"distDir":"../dist"}}"#,
            "dist",
        );
        let project = TauriProject::open(root.path()).unwrap();
        assert_eq!(project.version(), TauriVersion::V1);
        assert_eq!(project.bundle().unwrap().entry, "main.crepus");
    }

    #[test]
    fn rejects_missing_config() {
        let root = tempfile::tempdir().unwrap();
        let err = TauriProject::open(root.path()).unwrap_err();
        assert!(err.contains("no Tauri config under"));
    }

    #[test]
    fn rejects_malformed_config() {
        let root = project("tauri.conf.json", "{ malformed json", "dist");
        assert!(TauriProject::open(root.path()).is_err());
    }

    #[test]
    fn rejects_missing_build_object() {
        let root = project("tauri.conf.json", "{}", "dist");
        let err = TauriProject::open(root.path()).unwrap_err();
        assert_eq!(err, "tauri config missing build object");
    }

    #[test]
    fn rejects_missing_dist_dir() {
        let root = project("tauri.conf.json", r#"{"build":{}}"#, "dist");
        let err = TauriProject::open(root.path()).unwrap_err();
        assert_eq!(
            err,
            "tauri config missing build.frontendDist (v2) or build.distDir (v1)"
        );
    }

    #[test]
    fn reads_v2_frontend_dist() {
        let root = project(
            "tauri.conf.json",
            r#"{"build":{"frontendDist":"../dist"}}"#,
            "dist",
        );
        let project = TauriProject::open(root.path()).unwrap();
        assert_eq!(project.version(), TauriVersion::V2);
        assert_eq!(
            project.bundle().unwrap().files["main.crepus"],
            "div\n  \"Hello\""
        );
    }

    #[cfg(feature = "native")]
    #[test]
    fn renders_native_ir_from_static_bundle() {
        let root = project(
            "tauri.conf.json",
            r#"{"build":{"frontendDist":"../dist"}}"#,
            "dist",
        );
        let ir = TauriProject::open(root.path())
            .unwrap()
            .native_ir()
            .unwrap();
        assert_eq!(ir.root.len(), 1);
    }

    #[test]
    fn reads_json5_and_toml_configs() {
        let json5 = project(
            "tauri.conf.json5",
            "{ build: { frontendDist: '../dist' } }",
            "dist",
        );
        assert_eq!(
            TauriProject::open(json5.path()).unwrap().version(),
            TauriVersion::V2
        );
        let toml = project("Tauri.toml", "[build]\ndistDir = '../dist'\n", "dist");
        assert_eq!(
            TauriProject::open(toml.path()).unwrap().version(),
            TauriVersion::V1
        );
    }

    #[test]
    fn reads_application_metadata() {
        let root = project(
            "tauri.conf.json",
            r#"{"productName":"Desk","version":"2.3.4","identifier":"dev.example.desk","build":{"frontendDist":"../dist"},"app":{"windows":[{"title":"Desk Window"}]}}"#,
            "dist",
        );
        assert_eq!(
            TauriProject::open(root.path()).unwrap().metadata(),
            TauriMetadata {
                product_name: Some("Desk".into()),
                version: Some("2.3.4".into()),
                identifier: Some("dev.example.desk".into()),
                window_title: Some("Desk Window".into()),
            }
        );
    }

    #[test]
    fn rejects_unsafe_bundle_paths() {
        let root = project(
            "tauri.conf.json",
            r#"{"build":{"frontendDist":"../dist"}}"#,
            "dist",
        );
        fs::write(
            root.path().join("dist/crepus-bundle.json"),
            r#"{"entry":"../index.crepus","files":{"../index.crepus":"div"}}"#,
        )
        .unwrap();
        assert!(TauriProject::open(root.path()).unwrap().bundle().is_err());
    }

    #[test]
    fn audit_classifies_plugins_commands_and_windows() {
        let root = project(
            "tauri.conf.json",
            r#"{"build":{"frontendDist":"../dist"},"app":{"windows":[{},{}]}}"#,
            "dist",
        );
        fs::write(
            root.path().join("src-tauri/Cargo.toml"),
            "tauri-plugin-dialog = \"2\"\ntauri-plugin-updater = \"2\"\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src-tauri/lib.rs"),
            "#[tauri::command]\nfn greet() {}\n",
        )
        .unwrap();
        let report = TauriProject::open(root.path()).unwrap().audit();
        assert!(report
            .uses
            .iter()
            .any(|use_| use_.api == "plugin.dialog" && use_.coverage == Coverage::Native));
        assert!(report
            .uses
            .iter()
            .any(|use_| use_.api == "plugin.updater" && use_.coverage == Coverage::Unsupported));
        assert!(report
            .uses
            .iter()
            .any(|use_| use_.api == "command" && use_.coverage == Coverage::Backend));
        assert!(report.native_ready().is_err());
    }

    #[test]
    fn commands_events_and_plugin_requests_use_native_contracts() {
        let app = Builder::default()
            .command("greet", |payload| {
                Ok(serde_json::json!({ "name": payload["name"] }))
            })
            .build();
        assert_eq!(
            app.invoke("greet", serde_json::json!({ "name": "Ada" }))
                .unwrap(),
            serde_json::json!({ "name": "Ada" })
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let observed = seen.clone();
        let listener = app.listen("ready", move |event| {
            observed.lock().unwrap().push(event.payload.clone())
        });
        app.emit("ready", serde_json::json!(true));
        assert_eq!(*seen.lock().unwrap(), vec![serde_json::json!(true)]);
        drop(listener);
        app.emit("ready", serde_json::json!(false));
        assert_eq!(*seen.lock().unwrap(), vec![serde_json::json!(true)]);
        #[cfg(feature = "native")]
        assert_eq!(
            serde_json::to_value(plugin_request("dialog", "open", Value::Null).unwrap()).unwrap()
                ["capability"],
            "documentPicker"
        );
    }
}
