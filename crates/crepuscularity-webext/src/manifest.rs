//! Extension manifest parsing and generation (.crex format).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::capabilities::{Capability, CapabilitySet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserTarget {
    #[default]
    Chromium,
    Firefox,
    Safari,
}

impl BrowserTarget {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "chromium" | "chrome" | "edge" | "brave" | "opera" => Some(Self::Chromium),
            "firefox" | "gecko" => Some(Self::Firefox),
            "safari" => Some(Self::Safari),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::Firefox => "firefox",
            Self::Safari => "safari",
        }
    }

    pub fn dist_dir(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::Firefox => "firefox",
            Self::Safari => "safari",
        }
    }
}

/// Error type for manifest operations.
#[derive(Debug)]
pub enum ManifestError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl From<std::io::Error> for ManifestError {
    fn from(e: std::io::Error) -> Self {
        ManifestError::Io(e)
    }
}

impl From<toml::de::Error> for ManifestError {
    fn from(e: toml::de::Error) -> Self {
        ManifestError::Parse(e)
    }
}

impl From<toml::ser::Error> for ManifestError {
    fn from(e: toml::ser::Error) -> Self {
        ManifestError::Serialize(e)
    }
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io(e) => write!(f, "IO error: {e}"),
            ManifestError::Parse(e) => write!(f, "Parse error: {e}"),
            ManifestError::Serialize(e) => write!(f, "Serialize error: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Optional `suggested_key` for [`ExtensionManifestCommand`] (Chrome `commands` API).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtensionManifestSuggestedKey {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub windows: Option<String>,
    #[serde(default)]
    pub linux: Option<String>,
    #[serde(default)]
    pub chromeos: Option<String>,
}

/// One entry under `commands` in `manifest.json` (MV3).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtensionManifestCommand {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub suggested_key: Option<ExtensionManifestSuggestedKey>,
    /// When true, the shortcut may be active even when Chrome is not focused (platform-dependent).
    #[serde(default)]
    pub global: Option<bool>,
}

/// The extension manifest (.crex file).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub extension: ExtensionInfo,
    #[serde(default)]
    pub capabilities: CapabilitiesSection,
    #[serde(default)]
    pub content_scripts: Vec<ContentScriptEntry>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginEntry>,
    #[serde(default)]
    pub options: ManifestOptions,
    #[serde(default)]
    pub web_accessible_resources: WebAccessibleResourcesOptions,
    /// Map of Chrome command name → spec. Use `_execute_action` to open the toolbar popup from a keybinding.
    #[serde(default)]
    pub commands: BTreeMap<String, ExtensionManifestCommand>,
    /// Chrome `chrome_url_overrides` (e.g. `newtab` → `pages/new-tab.crepus`).
    #[serde(default)]
    pub chrome_url_overrides: BTreeMap<String, String>,
}

/// Basic extension information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub minimum_chrome_version: Option<String>,
}

/// Capabilities section in the manifest.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapabilitiesSection {
    #[serde(default, rename = "content-script")]
    pub content_script: bool,
    #[serde(default, rename = "background-script")]
    pub background_script: bool,
    #[serde(default)]
    pub storage: bool,
    #[serde(default)]
    pub messaging: bool,
    #[serde(default)]
    pub clipboard: bool,
    #[serde(default)]
    pub notifications: bool,
    #[serde(default)]
    pub tabs: bool,
    #[serde(default)]
    pub history: bool,
    #[serde(default)]
    pub bookmarks: bool,
    #[serde(default)]
    pub downloads: bool,
    #[serde(default, rename = "web-request")]
    pub web_request: bool,
    #[serde(default)]
    pub cookies: bool,
    #[serde(default)]
    pub geolocation: bool,
    #[serde(default)]
    pub identity: bool,
    #[serde(default, rename = "active-tab")]
    pub active_tab: bool,
    #[serde(default)]
    pub scripting: bool,
    #[serde(default)]
    pub alarms: bool,
    #[serde(default, rename = "native-messaging")]
    pub native_messaging: bool,
    #[serde(default, rename = "context-menus")]
    pub context_menus: bool,
    #[serde(default)]
    pub sessions: bool,
    #[serde(default, rename = "web-navigation")]
    pub web_navigation: bool,
    #[serde(default)]
    pub search: bool,
    #[serde(default)]
    pub favicon: bool,
    #[serde(default, rename = "host-permissions")]
    pub host_permissions: Vec<String>,
}

/// Content script entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentScriptEntry {
    pub matches: Vec<String>,
    #[serde(default)]
    pub js: Vec<String>,
    #[serde(default)]
    pub css: Vec<String>,
    #[serde(default)]
    pub run_at: Option<String>,
    #[serde(default)]
    pub all_frames: Option<bool>,
    #[serde(default)]
    pub match_about_blank: Option<bool>,
}

/// Plugin entry for custom functionality.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
}

/// Browser-ready MV3 manifest options that app authors can configure.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ManifestOptions {
    /// Map of icon size → resource path (e.g., "48" → "icons/48.png")
    #[serde(default)]
    pub icons: BTreeMap<String, String>,
    /// Content script CSS resource paths
    #[serde(default)]
    pub content_css: Vec<String>,
    /// Additional web_accessible_resources patterns
    #[serde(default)]
    pub extra_resources: Vec<String>,
    /// Custom popup HTML path (default: "src/popup.html")
    #[serde(default)]
    pub popup_html: Option<String>,
    #[serde(default)]
    pub options_page: Option<String>,
    #[serde(default)]
    pub options_ui: Option<OptionsUiSpec>,
    #[serde(default)]
    pub action_popup: Option<String>,
    #[serde(default)]
    pub action_icons: BTreeMap<String, String>,
    /// Custom background script path (default: "src/background.js")
    #[serde(default)]
    pub background_script: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OptionsUiSpec {
    pub page: String,
    #[serde(default)]
    pub browser_style: Option<bool>,
    #[serde(default)]
    pub open_in_tab: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WebAccessibleResourcesOptions {
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub matches: Vec<String>,
}

// ---------------------------------------------------------------------------
// Browser MV3 manifest generation
// ---------------------------------------------------------------------------

/// Full browser Manifest V3 document.
#[derive(Clone, Debug, Serialize)]
pub struct ManifestV3 {
    pub manifest_version: u8,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_chrome_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub icons: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub host_permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_security_policy: Option<ContentSecurityPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_specific_settings: Option<BrowserSpecificSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options_page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options_ui: Option<OptionsUiJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content_scripts: Vec<ContentScriptSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub web_accessible_resources: Vec<WebAccessibleResources>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub commands: BTreeMap<String, ManifestCommandJson>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub chrome_url_overrides: BTreeMap<String, String>,
}

/// Serialized form of a `commands` entry for `manifest.json`.
#[derive(Clone, Debug, Serialize)]
pub struct ManifestCommandJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_key: Option<ManifestSuggestedKeyJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManifestSuggestedKeyJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chromeos: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OptionsUiJson {
    pub page: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_style: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_in_tab: Option<bool>,
}

/// Content Security Policy for extension pages.
#[derive(Clone, Debug, Serialize)]
pub struct ContentSecurityPolicy {
    /// CSP for extension pages (popup, options). Must include 'wasm-unsafe-eval' to run WASM.
    pub extension_pages: String,
}

/// Background service worker specification.
#[derive(Clone, Debug, Serialize)]
pub struct BackgroundSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_worker: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserSpecificSettings {
    pub gecko: GeckoSettings,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeckoSettings {
    pub id: String,
}

/// Browser action (toolbar button) specification.
#[derive(Clone, Debug, Serialize)]
pub struct ActionSpec {
    pub default_popup: String,
    pub default_title: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub default_icon: BTreeMap<String, String>,
}

/// Content script specification for MV3.
#[derive(Clone, Debug, Serialize)]
pub struct ContentScriptSpec {
    pub matches: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub js: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
    pub run_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_frames: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_about_blank: Option<bool>,
}

/// Web accessible resources specification.
#[derive(Clone, Debug, Serialize)]
pub struct WebAccessibleResources {
    pub resources: Vec<String>,
    pub matches: Vec<String>,
}

impl ManifestV3 {
    /// Create a ManifestV3 from an ExtensionManifest.
    pub fn from_manifest(manifest: &ExtensionManifest) -> Self {
        Self::from_manifest_for_browser(manifest, BrowserTarget::Chromium)
    }

    pub fn from_manifest_for_browser(manifest: &ExtensionManifest, browser: BrowserTarget) -> Self {
        let opts = &manifest.options;

        let permissions = Self::build_permissions(manifest);
        let host_permissions = manifest.capabilities.host_permissions.clone();
        let content_scripts = Self::build_content_scripts(manifest, opts);
        let web_accessible_resources = Self::build_web_accessible_resources(
            manifest,
            opts,
            &host_permissions,
            &content_scripts,
        );
        let background = Self::build_background(manifest, opts, browser);
        let action = Self::build_action(manifest, opts);
        let chrome_url_overrides = Self::build_chrome_url_overrides(manifest);
        let commands = Self::build_commands(manifest);

        ManifestV3 {
            manifest_version: 3,
            name: manifest.extension.name.clone(),
            version: manifest.extension.version.clone(),
            minimum_chrome_version: manifest.extension.minimum_chrome_version.clone(),
            description: manifest.extension.description.clone(),
            icons: opts.icons.clone(),
            permissions,
            host_permissions,
            content_security_policy: (browser != BrowserTarget::Safari).then(|| {
                ContentSecurityPolicy {
                    extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'none';"
                        .to_string(),
                }
            }),
            background,
            browser_specific_settings: match browser {
                BrowserTarget::Chromium | BrowserTarget::Safari => None,
                BrowserTarget::Firefox => Some(BrowserSpecificSettings {
                    gecko: GeckoSettings {
                        id: format!(
                            "{}@crepuscularity.dev",
                            gecko_slug(&manifest.extension.name)
                        ),
                    },
                }),
            },
            action,
            options_page: if opts.options_ui.is_some() {
                None
            } else {
                opts.options_page.clone().map(Self::resolve_page)
            },
            options_ui: opts.options_ui.as_ref().map(|options_ui| OptionsUiJson {
                page: Self::resolve_page(options_ui.page.clone()),
                browser_style: options_ui.browser_style,
                open_in_tab: options_ui.open_in_tab,
            }),
            content_scripts,
            web_accessible_resources,
            commands,
            chrome_url_overrides,
        }
    }

    fn build_permissions(manifest: &ExtensionManifest) -> Vec<String> {
        let caps = manifest.to_capability_set();
        let mut permissions: Vec<String> = caps.to_permissions();
        permissions.sort();
        permissions.dedup();
        permissions
    }

    fn build_content_scripts(
        manifest: &ExtensionManifest,
        opts: &ManifestOptions,
    ) -> Vec<ContentScriptSpec> {
        if !manifest.content_scripts.is_empty() {
            manifest
                .content_scripts
                .iter()
                .map(|cs| {
                    let mut css = cs.css.clone();
                    css.extend(
                        opts.content_css
                            .iter()
                            .filter(|&e| !cs.css.contains(e))
                            .cloned(),
                    );
                    ContentScriptSpec {
                        matches: cs.matches.clone(),
                        js: cs.js.clone(),
                        css,
                        run_at: cs
                            .run_at
                            .clone()
                            .unwrap_or_else(|| "document_idle".to_string()),
                        all_frames: cs.all_frames,
                        match_about_blank: cs.match_about_blank,
                    }
                })
                .collect()
        } else if manifest.capabilities.content_script
            && !manifest.capabilities.host_permissions.is_empty()
        {
            let mut css = opts.content_css.clone();
            if css.is_empty() {
                css.push("src/content.css".to_string());
            }
            vec![ContentScriptSpec {
                matches: manifest.capabilities.host_permissions.clone(),
                js: vec!["src/content.js".to_string()],
                css,
                run_at: "document_idle".to_string(),
                all_frames: None,
                match_about_blank: None,
            }]
        } else {
            vec![]
        }
    }

    fn build_web_accessible_resources(
        manifest: &ExtensionManifest,
        opts: &ManifestOptions,
        host_permissions: &[String],
        content_scripts: &[ContentScriptSpec],
    ) -> Vec<WebAccessibleResources> {
        let mut resources = vec![
            "vendor/*".to_string(),
            "src/*".to_string(),
            "views/*".to_string(),
        ];
        resources.extend(opts.extra_resources.iter().cloned());
        resources.extend(manifest.web_accessible_resources.resources.iter().cloned());
        resources.sort();
        resources.dedup();

        let mut resource_matches = manifest.web_accessible_resources.matches.clone();
        if resource_matches.is_empty() {
            resource_matches = host_permissions.to_vec();
            let mut seen: std::collections::HashSet<&str> =
                host_permissions.iter().map(|s| s.as_str()).collect();
            for content_script in content_scripts {
                for pattern in &content_script.matches {
                    if seen.insert(pattern.as_str()) {
                        resource_matches.push(pattern.clone());
                    }
                }
            }
        }

        if !resources.is_empty() && !resource_matches.is_empty() {
            vec![WebAccessibleResources {
                resources,
                matches: resource_matches,
            }]
        } else {
            vec![]
        }
    }

    fn build_background(
        manifest: &ExtensionManifest,
        opts: &ManifestOptions,
        browser: BrowserTarget,
    ) -> Option<BackgroundSpec> {
        let background_script = opts
            .background_script
            .clone()
            .unwrap_or_else(|| "src/background.js".to_string());
        if manifest.capabilities.background_script {
            match browser {
                BrowserTarget::Chromium | BrowserTarget::Safari => Some(BackgroundSpec {
                    service_worker: Some(background_script),
                    scripts: Vec::new(),
                    kind: Some("module".to_string()),
                }),
                BrowserTarget::Firefox => Some(BackgroundSpec {
                    service_worker: None,
                    scripts: vec![background_script],
                    kind: None,
                }),
            }
        } else {
            None
        }
    }

    fn build_action(manifest: &ExtensionManifest, opts: &ManifestOptions) -> Option<ActionSpec> {
        Some(ActionSpec {
            default_popup: opts
                .action_popup
                .clone()
                .or_else(|| opts.popup_html.clone())
                .map(Self::resolve_page)
                .unwrap_or_else(|| "src/popup.html".to_string()),
            default_title: manifest.extension.name.clone(),
            default_icon: if opts.action_icons.is_empty() {
                opts.icons.clone()
            } else {
                opts.action_icons.clone()
            },
        })
    }

    fn resolve_page(p: String) -> String {
        if p.ends_with(".crepus") {
            p.trim_end_matches(".crepus").to_string() + ".html"
        } else {
            p
        }
    }

    fn build_chrome_url_overrides(manifest: &ExtensionManifest) -> BTreeMap<String, String> {
        manifest
            .chrome_url_overrides
            .iter()
            .map(|(key, page)| (key.clone(), Self::resolve_page(page.clone())))
            .collect()
    }

    fn build_commands(manifest: &ExtensionManifest) -> BTreeMap<String, ManifestCommandJson> {
        manifest
            .commands
            .iter()
            .map(|(name, cmd)| {
                let suggested_key = cmd
                    .suggested_key
                    .as_ref()
                    .map(|k| ManifestSuggestedKeyJson {
                        default: k.default.clone(),
                        mac: k.mac.clone(),
                        windows: k.windows.clone(),
                        linux: k.linux.clone(),
                        chromeos: k.chromeos.clone(),
                    });
                (
                    name.clone(),
                    ManifestCommandJson {
                        description: cmd.description.clone(),
                        suggested_key,
                        global: cmd.global,
                    },
                )
            })
            .collect()
    }

    /// Serialize to pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ManifestV3 serialization failed")
    }
}

fn gecko_slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "extension".to_string()
    } else {
        out
    }
}

impl ExtensionManifest {
    /// Load manifest from a .crex file.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path)?;
        let manifest: ExtensionManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Save manifest to a .crex file.
    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Convert capabilities section to a CapabilitySet.
    pub fn to_capability_set(&self) -> CapabilitySet {
        let mut set = CapabilitySet::new();

        if self.capabilities.content_script {
            set.add(Capability::ContentScript);
        }
        if self.capabilities.background_script {
            set.add(Capability::BackgroundScript);
        }
        if self.capabilities.storage {
            set.add(Capability::Storage);
        }
        if self.capabilities.messaging {
            set.add(Capability::Messaging);
        }
        if self.capabilities.clipboard {
            set.add(Capability::Clipboard);
        }
        if self.capabilities.notifications {
            set.add(Capability::Notifications);
        }
        if self.capabilities.tabs {
            set.add(Capability::Tabs);
        }
        if self.capabilities.history {
            set.add(Capability::History);
        }
        if self.capabilities.bookmarks {
            set.add(Capability::Bookmarks);
        }
        if self.capabilities.downloads {
            set.add(Capability::Downloads);
        }
        if self.capabilities.web_request {
            set.add(Capability::WebRequest);
        }
        if self.capabilities.cookies {
            set.add(Capability::Cookies);
        }
        if self.capabilities.geolocation {
            set.add(Capability::Geolocation);
        }
        if self.capabilities.identity {
            set.add(Capability::Identity);
        }
        if self.capabilities.active_tab {
            set.add(Capability::ActiveTab);
        }
        if self.capabilities.scripting {
            set.add(Capability::Scripting);
        }
        if self.capabilities.alarms {
            set.add(Capability::Alarms);
        }
        if self.capabilities.native_messaging {
            set.add(Capability::NativeMessaging);
        }
        if self.capabilities.context_menus {
            set.add(Capability::ContextMenus);
        }
        if self.capabilities.sessions {
            set.add(Capability::Sessions);
        }
        if self.capabilities.web_navigation {
            set.add(Capability::WebNavigation);
        }
        if self.capabilities.search {
            set.add(Capability::Search);
        }
        if self.capabilities.favicon {
            set.add(Capability::Favicon);
        }

        for pattern in &self.capabilities.host_permissions {
            set.add(Capability::HostPermission(pattern.clone()));
        }

        set
    }

    /// Generate a Chrome/Firefox manifest.json from this manifest (simple format).
    pub fn to_browser_manifest(&self) -> serde_json::Value {
        let caps = self.to_capability_set();

        let mut manifest = serde_json::json!({
            "manifest_version": 3,
            "name": self.extension.name,
            "version": self.extension.version,
        });

        if let Some(desc) = &self.extension.description {
            manifest["description"] = serde_json::json!(desc);
        }

        // Permissions
        let permissions = caps.to_permissions();
        if !permissions.is_empty() {
            manifest["permissions"] = serde_json::json!(permissions);
        }

        // Host permissions
        let host_permissions = caps.to_host_permissions();
        if !host_permissions.is_empty() {
            manifest["host_permissions"] = serde_json::json!(host_permissions);
        }

        // Content scripts
        if !self.content_scripts.is_empty() {
            let scripts: Vec<serde_json::Value> = self
                .content_scripts
                .iter()
                .map(|cs| {
                    let mut entry = serde_json::json!({
                        "matches": cs.matches,
                    });
                    if !cs.js.is_empty() {
                        entry["js"] = serde_json::json!(cs.js);
                    }
                    if !cs.css.is_empty() {
                        entry["css"] = serde_json::json!(cs.css);
                    }
                    if let Some(run_at) = &cs.run_at {
                        entry["run_at"] = serde_json::json!(run_at);
                    }
                    if let Some(all_frames) = cs.all_frames {
                        entry["all_frames"] = serde_json::json!(all_frames);
                    }
                    if let Some(match_about_blank) = cs.match_about_blank {
                        entry["match_about_blank"] = serde_json::json!(match_about_blank);
                    }
                    entry
                })
                .collect();
            manifest["content_scripts"] = serde_json::json!(scripts);
        }

        manifest
    }

    /// Generate a full MV3 manifest.
    pub fn to_manifest_v3(&self) -> ManifestV3 {
        ManifestV3::from_manifest(self)
    }

    pub fn to_manifest_v3_for_browser(&self, browser: BrowserTarget) -> ManifestV3 {
        ManifestV3::from_manifest_for_browser(self, browser)
    }

    /// Generate a full MV3 manifest as JSON string.
    pub fn to_manifest_v3_json(&self) -> String {
        self.to_manifest_v3().to_json()
    }

    pub fn to_manifest_v3_json_for_browser(&self, browser: BrowserTarget) -> String {
        self.to_manifest_v3_for_browser(browser).to_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml = r#"
[extension]
name = "test-ext"
version = "1.0.0"

[capabilities]
storage = true
messaging = true
host-permissions = ["https://example.com/*"]
"#;

        let manifest: ExtensionManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.extension.name, "test-ext");
        assert!(manifest.capabilities.storage);
        assert!(manifest.capabilities.messaging);
        assert_eq!(manifest.capabilities.host_permissions.len(), 1);
    }

    #[test]
    fn test_to_browser_manifest() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: Some("A test extension".to_string()),
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                storage: true,
                tabs: true,
                host_permissions: vec!["https://*.example.com/*".to_string()],
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let browser = manifest.to_browser_manifest();
        assert_eq!(browser["manifest_version"], 3);
        assert_eq!(browser["name"], "Test");
    }

    #[test]
    fn content_script_and_background_not_in_permissions() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                content_script: true,
                background_script: true,
                storage: true,
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let mv3 = manifest.to_manifest_v3();
        assert!(
            !mv3.permissions.contains(&"content_scripts".to_string()),
            "content_scripts must not appear in permissions"
        );
        assert!(
            !mv3.permissions.contains(&"background".to_string()),
            "background must not appear in permissions"
        );
        assert!(mv3.permissions.contains(&"storage".to_string()));
        assert!(mv3.background.is_some());
        assert!(mv3.host_permissions.is_empty());
        assert!(mv3.content_scripts.is_empty());
        assert!(mv3.web_accessible_resources.is_empty());
        assert!(!mv3.to_json().contains("<all_urls>"));
    }

    #[test]
    fn content_script_default_uses_declared_hosts_only() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                content_script: true,
                host_permissions: vec!["https://example.com/*".to_string()],
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let mv3 = manifest.to_manifest_v3();
        assert_eq!(mv3.host_permissions, vec!["https://example.com/*"]);
        assert_eq!(mv3.content_scripts.len(), 1);
        assert_eq!(
            mv3.content_scripts[0].matches,
            vec!["https://example.com/*"]
        );
        assert_eq!(mv3.content_scripts[0].js, vec!["src/content.js"]);
        assert_eq!(mv3.content_scripts[0].css, vec!["src/content.css"]);
        assert_eq!(mv3.web_accessible_resources.len(), 1);
        assert_eq!(
            mv3.web_accessible_resources[0].matches,
            vec!["https://example.com/*"]
        );
        assert!(!mv3.to_json().contains("<all_urls>"));
    }

    #[test]
    fn content_script_default_fallback_match_about_blank() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                content_script: true,
                host_permissions: vec!["https://example.com/*".to_string()],
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let opts = ManifestOptions::default();
        let content_scripts = ManifestV3::build_content_scripts(&manifest, &opts);

        assert_eq!(content_scripts.len(), 1);
        assert_eq!(content_scripts[0].matches, vec!["https://example.com/*"]);
        assert_eq!(content_scripts[0].js, vec!["src/content.js"]);
        assert_eq!(content_scripts[0].css, vec!["src/content.css"]);
        assert_eq!(content_scripts[0].run_at, "document_idle".to_string());
        assert_eq!(content_scripts[0].all_frames, None);
        assert_eq!(content_scripts[0].match_about_blank, None);
    }

    #[test]
    fn test_manifest_v3_generation() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Full Test".to_string(),
                version: "2.0.0".to_string(),
                description: Some("A full test extension".to_string()),
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                storage: true,
                background_script: true,
                content_script: true,
                host_permissions: vec!["https://*.example.com/*".to_string()],
                ..Default::default()
            },
            content_scripts: vec![ContentScriptEntry {
                matches: vec!["https://*.example.com/*".to_string()],
                js: vec!["src/content.js".to_string()],
                css: vec!["src/content.css".to_string()],
                run_at: Some("document_idle".to_string()),
                all_frames: None,
                match_about_blank: None,
            }],
            plugins: HashMap::new(),
            options: ManifestOptions {
                icons: BTreeMap::from([
                    ("48".to_string(), "icons/48.png".to_string()),
                    ("128".to_string(), "icons/128.png".to_string()),
                ]),
                ..Default::default()
            },
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let mv3 = manifest.to_manifest_v3();
        assert_eq!(mv3.manifest_version, 3);
        assert_eq!(mv3.name, "Full Test");
        assert!(mv3.background.is_some());
        assert!(!mv3.content_scripts.is_empty());
        assert!(!mv3.icons.is_empty());

        let json = mv3.to_json();
        assert!(json.contains("\"manifest_version\": 3"));
        assert!(json.contains("wasm-unsafe-eval"));
        assert!(json.contains("object-src 'none'"));
    }

    #[test]
    fn chrome_url_overrides_resolve_crepus_pages() {
        let toml = r#"
[extension]
name = "nt"
version = "1.0.0"

[chrome_url_overrides]
newtab = "pages/new-tab.crepus"
"#;
        let manifest: ExtensionManifest = toml::from_str(toml).unwrap();
        let mv3 = manifest.to_manifest_v3();
        assert_eq!(
            mv3.chrome_url_overrides.get("newtab").map(String::as_str),
            Some("pages/new-tab.html")
        );
    }

    #[test]
    fn test_commands_in_manifest_json() {
        let toml = r#"
[extension]
name = "cmd-ext"
version = "1.0.0"

[capabilities]
storage = true

[commands._execute_action]
description = "Open popup"

[commands._execute_action.suggested_key]
default = "Alt+Shift+W"
mac = "Alt+Shift+W"
"#;
        let manifest: ExtensionManifest = toml::from_str(toml).unwrap();
        let json = manifest.to_manifest_v3_json();
        assert!(json.contains("\"commands\""));
        assert!(json.contains("_execute_action"));
        assert!(json.contains("Alt+Shift+W"));
    }

    #[test]
    fn native_messaging_in_permissions() {
        let toml = r#"
[extension]
name = "nm-ext"
version = "1.0.0"

[capabilities]
storage = true
native-messaging = true
alarms = true
"#;
        let manifest: ExtensionManifest = toml::from_str(toml).unwrap();
        assert!(manifest.capabilities.native_messaging);
        let mv3 = manifest.to_manifest_v3();
        assert!(mv3.permissions.contains(&"nativeMessaging".to_string()));
        assert!(mv3.permissions.contains(&"alarms".to_string()));
    }

    #[test]
    fn firefox_manifest_uses_scripts_background_and_gecko_id() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test Extension".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                background_script: true,
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        let json = manifest.to_manifest_v3_json_for_browser(BrowserTarget::Firefox);
        assert!(json.contains("\"scripts\": ["));
        assert!(json.contains("\"src/background.js\""));
        assert!(json.contains("\"browser_specific_settings\""));
        assert!(json.contains("\"test-extension@crepuscularity.dev\""));
        assert!(!json.contains("\"service_worker\""));
    }

    #[test]
    fn safari_manifest_uses_service_worker_without_gecko_settings() {
        let manifest = ExtensionManifest {
            extension: ExtensionInfo {
                name: "Test Extension".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                homepage: None,
                minimum_chrome_version: None,
            },
            capabilities: CapabilitiesSection {
                background_script: true,
                ..Default::default()
            },
            content_scripts: vec![],
            plugins: HashMap::new(),
            options: ManifestOptions::default(),
            web_accessible_resources: WebAccessibleResourcesOptions::default(),
            commands: BTreeMap::new(),
            chrome_url_overrides: BTreeMap::new(),
        };

        assert_eq!(BrowserTarget::parse("safari"), Some(BrowserTarget::Safari));
        assert_eq!(BrowserTarget::Safari.dist_dir(), "safari");
        let json = manifest.to_manifest_v3_json_for_browser(BrowserTarget::Safari);
        assert!(json.contains("\"service_worker\""));
        assert!(json.contains("\"type\": \"module\""));
        assert!(!json.contains("\"browser_specific_settings\""));
    }

    #[test]
    fn vimium_grade_manifest_fields_round_trip_to_mv3() {
        let toml = r#"
[extension]
name = "vimium-crepus"
version = "0.2.0"
minimum_chrome_version = "117.0"

[capabilities]
tabs = true
storage = true
sessions = true
bookmarks = true
history = true
notifications = true
scripting = true
web-navigation = true
search = true
favicon = true
background-script = true
content-script = true
clipboard = true
host-permissions = ["<all_urls>"]

[[content_scripts]]
matches = ["<all_urls>"]
js = ["src/content.js"]
css = ["src/content.css"]
run_at = "document_idle"
all_frames = true
match_about_blank = true

[options]
content_css = ["src/content.css"]
options_page = "pages/options.html"
action_popup = "pages/action.html"
action_icons = { "16" = "icons/action_disabled_16.png", "32" = "icons/action_disabled_32.png" }

[options.options_ui]
page = "pages/options.html"
browser_style = false
open_in_tab = true

[web_accessible_resources]
resources = ["pages/vomnibar.html", "resources/tlds.txt"]
matches = ["<all_urls>"]
"#;

        let manifest: ExtensionManifest = toml::from_str(toml).unwrap();
        let mv3 = manifest.to_manifest_v3();

        assert!(mv3.permissions.contains(&"sessions".to_string()));
        assert!(mv3.permissions.contains(&"webNavigation".to_string()));
        assert!(mv3.permissions.contains(&"search".to_string()));
        assert!(mv3.permissions.contains(&"favicon".to_string()));
        assert!(mv3.permissions.contains(&"clipboardRead".to_string()));
        assert!(mv3.permissions.contains(&"clipboardWrite".to_string()));
        assert_eq!(mv3.minimum_chrome_version.as_deref(), Some("117.0"));
        assert!(mv3.options_page.is_none());
        assert_eq!(
            mv3.options_ui.as_ref().map(|options| options.page.as_str()),
            Some("pages/options.html")
        );
        assert_eq!(
            mv3.options_ui
                .as_ref()
                .and_then(|options| options.open_in_tab),
            Some(true)
        );
        assert_eq!(
            mv3.action
                .as_ref()
                .map(|action| action.default_popup.as_str()),
            Some("pages/action.html")
        );
        assert_eq!(
            mv3.action
                .as_ref()
                .and_then(|action| action.default_icon.get("16"))
                .map(String::as_str),
            Some("icons/action_disabled_16.png")
        );
        assert_eq!(mv3.content_scripts.len(), 1);
        assert_eq!(mv3.content_scripts[0].all_frames, Some(true));
        assert_eq!(mv3.content_scripts[0].match_about_blank, Some(true));
        assert_eq!(mv3.web_accessible_resources.len(), 1);
        assert!(mv3.web_accessible_resources[0]
            .resources
            .contains(&"pages/vomnibar.html".to_string()));
        assert_eq!(mv3.web_accessible_resources[0].matches, vec!["<all_urls>"]);
    }

    #[test]
    fn test_browser_target_parse() {
        assert_eq!(
            BrowserTarget::parse("chromium"),
            Some(BrowserTarget::Chromium)
        );
        assert_eq!(
            BrowserTarget::parse("chrome"),
            Some(BrowserTarget::Chromium)
        );
        assert_eq!(BrowserTarget::parse("edge"), Some(BrowserTarget::Chromium));
        assert_eq!(BrowserTarget::parse("brave"), Some(BrowserTarget::Chromium));
        assert_eq!(BrowserTarget::parse("opera"), Some(BrowserTarget::Chromium));
        assert_eq!(
            BrowserTarget::parse("firefox"),
            Some(BrowserTarget::Firefox)
        );
        assert_eq!(BrowserTarget::parse("gecko"), Some(BrowserTarget::Firefox));
        assert_eq!(BrowserTarget::parse("safari"), Some(BrowserTarget::Safari));

        assert_eq!(BrowserTarget::parse("CHROMIUM"), None);
        assert_eq!(BrowserTarget::parse("unknown"), None);
        assert_eq!(BrowserTarget::parse(""), None);
        assert_eq!(BrowserTarget::parse("Chrome"), None);
    }
}
