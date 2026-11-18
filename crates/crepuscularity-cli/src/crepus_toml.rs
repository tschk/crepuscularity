//! Root **`crepus.toml`** — shared manifest for **`[ios]`** and one or more **`[[targets]]`** (`type = "web"`).
//!
//! Paths under each web target are relative to the directory containing **`crepus.toml`**.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Deserialize)]
pub struct CrepusManifest {
    #[serde(default)]
    pub ios: Option<IosTomlSection>,
    #[serde(default)]
    pub android: Option<AndroidTomlSection>,
    #[serde(default)]
    pub apple: Option<AppleTomlSection>,
    #[serde(default)]
    pub targets: Vec<ManifestTarget>,
    /// Single-site shorthand when you do not use `[[targets]]`.
    #[serde(default)]
    pub web: Option<WebShorthand>,
}

#[derive(Debug, Deserialize)]
pub struct IosTomlSection {
    #[serde(default = "default_ios_scheme")]
    pub scheme: String,
    #[serde(default = "default_ios_destination")]
    pub destination: String,
    #[serde(default)]
    pub bundle_id: Option<String>,
    #[serde(default)]
    pub development_team: Option<String>,
    #[serde(default)]
    pub code_sign_style: Option<String>,
    #[serde(default)]
    pub allow_provisioning_updates: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleTomlSection {
    pub name: String,
    pub platform: String,
    pub bundle_id: String,
    pub deployment_target: String,
    #[serde(default = "default_native_shell")]
    pub package_path: String,
    #[serde(default = "default_native_shell")]
    pub package_product: String,
    #[serde(default)]
    pub destination: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AndroidTomlSection {
    #[serde(default)]
    pub application_id: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestTarget {
    #[serde(rename = "type")]
    pub target_type: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub out: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub ctx: Option<String>,
    #[serde(default)]
    pub vars: HashMap<String, toml::Value>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub width: Option<u16>,
    #[serde(default)]
    pub height: Option<u16>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub head_html: Option<String>,
    #[serde(default)]
    pub google_fonts: Vec<String>,
    #[serde(default)]
    pub seo: Option<SeoConfig>,
    #[serde(default)]
    pub docs: Option<DocsHookConfig>,
    #[serde(default)]
    pub llms: Option<LlmsConfig>,
    #[serde(default)]
    pub extension: Option<crepuscularity_webext::ExtensionInfo>,
    #[serde(default)]
    pub capabilities: Option<crepuscularity_webext::CapabilitiesSection>,
    #[serde(default)]
    pub content_scripts: Vec<crepuscularity_webext::ContentScriptEntry>,
    #[serde(default)]
    pub plugins: HashMap<String, crepuscularity_webext::PluginEntry>,
    #[serde(default)]
    pub options: Option<crepuscularity_webext::ManifestOptions>,
    #[serde(default)]
    pub web_accessible_resources: Option<crepuscularity_webext::WebAccessibleResourcesOptions>,
    #[serde(default)]
    pub commands: BTreeMap<String, crepuscularity_webext::ExtensionManifestCommand>,
    #[serde(default)]
    pub chrome_url_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub browsers: Vec<String>,
    #[serde(default)]
    pub safari: Option<SafariTomlSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SafariTomlSection {
    pub bundle_identifier: String,
    #[serde(default)]
    pub project_location: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WebextTargetConfig {
    pub browsers: Vec<String>,
    pub safari: Option<SafariTomlSection>,
}

#[derive(Debug, Deserialize)]
pub struct WebShorthand {
    /// Site root (relative to manifest dir). Default `"."`.
    #[serde(default = "default_dot")]
    pub site: String,
    #[serde(default)]
    pub out: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
}

fn default_dot() -> String {
    ".".into()
}

fn default_native_shell() -> String {
    "NativeShell".into()
}

pub(crate) fn default_ios_scheme() -> String {
    "CrepusMobileApp".into()
}

pub(crate) fn default_ios_destination() -> String {
    "platform=iOS Simulator,name=iPhone 16,OS=latest".into()
}

/// Resolved web site for `crepus web build` / `dev`.
#[derive(Debug, Clone)]
pub struct ResolvedWebTarget {
    pub id: String,
    pub site_dir: PathBuf,
    pub out_dir: PathBuf,
    pub entry: String,
    pub meta: WebTargetMeta,
}

#[derive(Debug, Clone, Default)]
pub struct WebTargetMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub head_html: Option<String>,
    pub google_fonts: Vec<String>,
    pub seo: Option<SeoConfig>,
    pub docs: Option<DocsHookConfig>,
    pub llms: Option<LlmsConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocsHookConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub src: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sources: Vec<LlmsSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmsSource {
    pub path: String,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeoConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub canonical: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_alt: Option<String>,
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(rename = "type", default)]
    pub og_type: Option<String>,
    #[serde(default)]
    pub twitter_card: Option<String>,
    #[serde(default)]
    pub twitter_site: Option<String>,
    #[serde(default)]
    pub twitter_creator: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub robots: Option<String>,
    #[serde(default)]
    pub theme_color: Option<String>,
    #[serde(default)]
    pub application_name: Option<String>,
    #[serde(default)]
    pub generator: Option<String>,
    #[serde(default)]
    pub alternates: Vec<SeoAlternate>,
    #[serde(default)]
    pub json_ld: Vec<String>,
    #[serde(default)]
    pub robots_txt: Option<RobotsTxtConfig>,
    #[serde(default)]
    pub sitemap: Option<SitemapConfig>,
    /// Path or URL to favicon (e.g. "/favicon.svg", "/favicon.ico").
    /// Generates `<link rel="icon">` tags automatically.
    #[serde(default)]
    pub favicon: Option<String>,
    /// Path or URL to apple-touch-icon (e.g. "/apple-touch-icon.png").
    #[serde(default)]
    pub apple_touch_icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeoAlternate {
    pub href: String,
    #[serde(default)]
    pub hreflang: Option<String>,
    #[serde(default)]
    pub media: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "type", default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotsTxtConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub disallow: Vec<String>,
    #[serde(default)]
    pub sitemap: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SitemapConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub changefreq: Option<String>,
    #[serde(default)]
    pub priority: Option<f32>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub id: String,
    pub target_type: String,
    pub dir: PathBuf,
    pub out: Option<PathBuf>,
    pub entry: Option<String>,
    pub template: Option<PathBuf>,
    pub component: Option<String>,
    pub ctx: Option<PathBuf>,
    pub vars: HashMap<String, toml::Value>,
    pub name: Option<String>,
    pub root: Option<String>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub web: WebTargetMeta,
    pub webext: Option<crepuscularity_webext::ExtensionManifest>,
    pub webext_config: WebextTargetConfig,
}

type WebTargetRow = (Option<String>, String, Option<String>, Option<String>);

impl CrepusManifest {
    pub fn parse(src: &str) -> Result<Self, String> {
        toml::from_str(src).map_err(|e| e.to_string())
    }

    /// All `type = "web"` entries, plus `[web]` when there are no `[[targets]]` web rows.
    pub fn web_targets(&self, manifest_dir: &Path) -> Result<Vec<ResolvedWebTarget>, String> {
        let mut raw: Vec<(WebTargetRow, WebTargetMeta)> = Vec::new();

        for t in &self.targets {
            if !t.target_type.eq_ignore_ascii_case("web") {
                continue;
            }
            let site = t
                .site
                .clone()
                .or_else(|| t.path.clone())
                .or_else(|| t.app.clone())
                .unwrap_or_else(|| ".".into());
            raw.push((
                (t.id.clone(), site, t.out.clone(), t.entry.clone()),
                WebTargetMeta {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    head_html: t.head_html.clone(),
                    google_fonts: t.google_fonts.clone(),
                    seo: t.seo.clone(),
                    docs: t.docs.clone(),
                    llms: t.llms.clone(),
                },
            ));
        }

        if raw.is_empty() {
            if let Some(w) = &self.web {
                raw.push((
                    (None, w.site.clone(), w.out.clone(), w.entry.clone()),
                    WebTargetMeta::default(),
                ));
            }
        }

        if raw.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::with_capacity(raw.len());
        for ((id, site, od, ent), meta) in raw {
            let site_dir = manifest_dir.join(&site);
            let site_dir = std::fs::canonicalize(&site_dir).unwrap_or(site_dir);
            let default_out = site_dir.join("dist");
            let out_dir = od
                .map(|r| {
                    let p = manifest_dir.join(r);
                    std::fs::canonicalize(&p).unwrap_or(p)
                })
                .unwrap_or(default_out);
            let entry = ent
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "index.crepus".into());
            let id = id.unwrap_or_else(|| {
                site_dir
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "default".into())
            });
            out.push(ResolvedWebTarget {
                id,
                site_dir,
                out_dir,
                entry,
                meta,
            });
        }

        Ok(out)
    }

    pub fn resolved_targets(&self, manifest_dir: &Path) -> Vec<ResolvedTarget> {
        self.targets
            .iter()
            .enumerate()
            .map(|(idx, target)| {
                let raw_dir = target
                    .site
                    .as_ref()
                    .or(target.path.as_ref())
                    .or(target.app.as_ref())
                    .map(String::as_str)
                    .unwrap_or(".");
                let dir = absolutize(manifest_dir, raw_dir);
                let id = target.id.clone().unwrap_or_else(|| {
                    dir.file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("target-{idx}"))
                });
                ResolvedTarget {
                    id,
                    target_type: target.target_type.to_ascii_lowercase(),
                    dir,
                    out: target.out.as_deref().map(|p| absolutize(manifest_dir, p)),
                    entry: target.entry.clone(),
                    template: target
                        .template
                        .as_deref()
                        .map(|p| absolutize(manifest_dir, p)),
                    component: target.component.clone(),
                    ctx: target.ctx.as_deref().map(|p| absolutize(manifest_dir, p)),
                    vars: target.vars.clone(),
                    name: target.name.clone(),
                    root: target.root.clone(),
                    width: target.width,
                    height: target.height,
                    web: WebTargetMeta {
                        name: target.name.clone(),
                        description: target.description.clone(),
                        head_html: target.head_html.clone(),
                        google_fonts: target.google_fonts.clone(),
                        seo: target.seo.clone(),
                        docs: target.docs.clone(),
                        llms: target.llms.clone(),
                    },
                    webext: target.extension.clone().map(|extension| {
                        crepuscularity_webext::ExtensionManifest {
                            extension,
                            capabilities: target.capabilities.clone().unwrap_or_default(),
                            content_scripts: target.content_scripts.clone(),
                            plugins: target.plugins.clone(),
                            options: target.options.clone().unwrap_or_default(),
                            web_accessible_resources: target
                                .web_accessible_resources
                                .clone()
                                .unwrap_or_default(),
                            commands: target.commands.clone(),
                            chrome_url_overrides: target.chrome_url_overrides.clone(),
                        }
                    }),
                    webext_config: WebextTargetConfig {
                        browsers: target.browsers.clone(),
                        safari: target.safari.clone(),
                    },
                }
            })
            .collect()
    }
}

fn absolutize(base: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

/// Resolve the directory containing `crepus.toml` against `cwd` when the manifest path is relative.
fn manifest_dir(manifest_path: &Path, cwd: &Path) -> Option<PathBuf> {
    let parent = manifest_path.parent()?;
    let parent = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        cwd.join(parent)
    };
    Some(std::fs::canonicalize(&parent).unwrap_or(parent))
}

pub fn find_manifest_upward(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let p = dir.join("crepus.toml");
        if p.is_file() {
            return Some(p);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn try_load_ios(manifest_path: &Path) -> Option<IosTomlSection> {
    let raw = std::fs::read_to_string(manifest_path).ok()?;
    let m: CrepusManifest = CrepusManifest::parse(&raw).ok()?;
    m.ios
}

pub fn try_load_apple(manifest_path: &Path) -> Option<AppleTomlSection> {
    let raw = std::fs::read_to_string(manifest_path).ok()?;
    CrepusManifest::parse(&raw).ok()?.apple
}

/// Load web targets from `crepus.toml`, walking up from cwd when `manifest` is `None`.
pub fn load_web_targets(manifest: Option<PathBuf>) -> Option<Vec<ResolvedWebTarget>> {
    let cwd = std::env::current_dir().ok()?;
    let mpath = manifest.or_else(|| find_manifest_upward(&cwd))?;
    let md = manifest_dir(&mpath, &cwd)?;
    let raw = std::fs::read_to_string(&mpath).ok()?;
    let man = CrepusManifest::parse(&raw).ok()?;
    let targets = man.web_targets(&md).ok()?;
    if targets.is_empty() {
        None
    } else {
        Some(targets)
    }
}

pub fn load_manifest_targets(
    manifest: Option<PathBuf>,
) -> Result<Option<Vec<ResolvedTarget>>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    let Some(mpath) = manifest.or_else(|| find_manifest_upward(&cwd)) else {
        return Ok(None);
    };
    let md = manifest_dir(&mpath, &cwd)
        .ok_or_else(|| format!("manifest has no parent: {}", mpath.display()))?;
    let raw =
        std::fs::read_to_string(&mpath).map_err(|e| format!("read {}: {e}", mpath.display()))?;
    let man = CrepusManifest::parse(&raw)?;
    Ok(Some(man.resolved_targets(&md)))
}

pub fn pick_targets(
    targets: &[ResolvedTarget],
    id: Option<&str>,
) -> Result<Vec<ResolvedTarget>, String> {
    if let Some(id) = id {
        let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
        return targets
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .map(|t| vec![t])
            .ok_or_else(|| format!("no target with id {id:?} (available: {ids:?})"));
    }
    Ok(targets.to_vec())
}

pub fn resolve_pick(
    targets: &[ResolvedWebTarget],
    id: Option<&str>,
) -> Result<ResolvedWebTarget, String> {
    let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    match (id, targets.len()) {
        (Some(id), _) => targets
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| format!("no web target with id {id:?} (available: {ids:?})")),
        (None, 1) => Ok(targets[0].clone()),
        (None, n) => Err(format!(
            "crepus.toml defines {n} web targets {ids:?}; pass --target ID"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_web_target(id: &str) -> ResolvedWebTarget {
        ResolvedWebTarget {
            id: id.to_string(),
            site_dir: std::path::PathBuf::from(format!("/tmp/{}", id)),
            out_dir: std::path::PathBuf::from(format!("/tmp/{}/out", id)),
            entry: "src/main.rs".to_string(),
            meta: WebTargetMeta::default(),
        }
    }

    #[test]
    fn test_resolve_pick() {
        let t1 = dummy_web_target("foo");
        let t2 = dummy_web_target("bar");

        // 0 targets, no id
        let err = resolve_pick(&[], None).unwrap_err();
        assert_eq!(
            err,
            "crepus.toml defines 0 web targets []; pass --target ID"
        );

        // 1 target, no id
        let res = resolve_pick(std::slice::from_ref(&t1), None).unwrap();
        assert_eq!(res.id, "foo");

        // 2 targets, no id
        let err = resolve_pick(&[t1.clone(), t2.clone()], None).unwrap_err();
        assert_eq!(
            err,
            r#"crepus.toml defines 2 web targets ["foo", "bar"]; pass --target ID"#
        );

        // id matches
        let res = resolve_pick(&[t1.clone(), t2.clone()], Some("bar")).unwrap();
        assert_eq!(res.id, "bar");

        // id doesn't match
        let err = resolve_pick(&[t1.clone(), t2.clone()], Some("baz")).unwrap_err();
        assert_eq!(
            err,
            r#"no web target with id "baz" (available: ["foo", "bar"])"#
        );
    }

    #[test]
    fn resolves_webext_browsers_and_safari_packaging() {
        let manifest = CrepusManifest::parse(
            r#"
[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["chromium", "firefox", "safari"]

[targets.extension]
name = "Example"
version = "1.0.0"

[targets.safari]
bundle_identifier = "com.example.extension"
project_location = "build/safari"
app_name = "Example Safari"
platforms = ["macos", "ios"]
"#,
        )
        .expect("parse manifest");
        let targets = manifest.resolved_targets(Path::new("/tmp/example"));
        let target = targets.first().expect("target");
        assert_eq!(
            target.webext_config.browsers,
            ["chromium", "firefox", "safari"]
        );
        let safari = target.webext_config.safari.as_ref().expect("safari config");
        assert_eq!(safari.bundle_identifier, "com.example.extension");
        assert_eq!(safari.project_location.as_deref(), Some("build/safari"));
    }
}
