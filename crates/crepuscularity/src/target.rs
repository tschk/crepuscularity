use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crepuscularity_core::{parser::parse_template_with_path, TemplateContext, TemplateValue};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
pub struct Target {
    #[serde(rename = "type")]
    pub target_type: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
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
    pub out: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub id: String,
    pub target_type: String,
    pub path: Option<PathBuf>,
    pub contents: String,
}

pub fn build_manifest_file(path: impl AsRef<Path>) -> Result<Vec<Artifact>, String> {
    build_manifest_file_target(path, None)
}

pub fn write_manifest_file(path: impl AsRef<Path>) -> Result<Vec<Artifact>, String> {
    write_manifest_file_target(path, None)
}

pub fn write_manifest_file_target(
    path: impl AsRef<Path>,
    target_id: Option<&str>,
) -> Result<Vec<Artifact>, String> {
    let artifacts = build_manifest_file_target(path, target_id)?;
    write_artifacts(&artifacts)?;
    Ok(artifacts)
}

pub fn build_manifest_file_target(
    path: impl AsRef<Path>,
    target_id: Option<&str>,
) -> Result<Vec<Artifact>, String> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let manifest = Manifest::parse(&raw)?;
    let base = path
        .parent()
        .ok_or_else(|| format!("manifest has no parent: {}", path.display()))?;
    build_manifest(&manifest, base, target_id)
}

pub fn build_manifest(
    manifest: &Manifest,
    base_dir: impl AsRef<Path>,
    target_id: Option<&str>,
) -> Result<Vec<Artifact>, String> {
    let base_dir = base_dir.as_ref();
    let targets = manifest.targets.iter().enumerate().filter(|(_, target)| {
        target_id
            .map(|id| target.id.as_deref() == Some(id))
            .unwrap_or(true)
    });
    let mut out = Vec::new();
    for (idx, target) in targets {
        out.push(build_target(target, base_dir, idx)?);
    }
    if let (true, Some(target_id)) = (out.is_empty(), target_id) {
        return Err(format!("no target with id {target_id:?}"));
    }
    Ok(out)
}

impl Manifest {
    pub fn parse(src: &str) -> Result<Self, String> {
        toml::from_str(src).map_err(|e| e.to_string())
    }
}

pub fn write_artifacts(artifacts: &[Artifact]) -> Result<(), String> {
    for artifact in artifacts {
        let Some(path) = &artifact.path else {
            continue;
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, &artifact.contents)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn build_target(target: &Target, base_dir: &Path, idx: usize) -> Result<Artifact, String> {
    let target_type = target.target_type.to_ascii_lowercase();
    match target_type.as_str() {
        "web" => build_web(target, base_dir, idx),
        "lvgl" => build_lvgl(target, base_dir, idx),
        "native" | "ir" => build_native(target, base_dir, idx),
        other => Err(format!("unsupported API target type {other:?}")),
    }
}

fn build_web(target: &Target, base_dir: &Path, idx: usize) -> Result<Artifact, String> {
    let root = target_dir(target, base_dir);
    let template_path = target
        .entry
        .as_ref()
        .map(|entry| root.join(entry))
        .unwrap_or_else(|| root.join("index.crepus"));
    let template = std::fs::read_to_string(&template_path)
        .map_err(|e| format!("read {}: {e}", template_path.display()))?;
    let ctx = target_context(
        target,
        base_dir,
        template_path.parent().map(Path::to_path_buf),
    )?;
    let contents = if let Some(component) = &target.component {
        crepuscularity_web::render_component_file_to_html(&template, component, &ctx)
    } else {
        let nodes =
            parse_template_with_path(&template, Some(&template_path)).map_err(|e| e.to_string())?;
        crepuscularity_web::render_nodes_to_html(&nodes, &ctx)
    }
    .map_err(|e| e.to_string())?;
    Ok(artifact(target, idx, "web", contents, base_dir))
}

fn build_lvgl(target: &Target, base_dir: &Path, idx: usize) -> Result<Artifact, String> {
    let template_path = template_path(target, base_dir, "ui.crepus");
    let template = std::fs::read_to_string(&template_path)
        .map_err(|e| format!("read {}: {e}", template_path.display()))?;
    let ctx = target_context(
        target,
        base_dir,
        template_path.parent().map(Path::to_path_buf),
    )?;
    let contents = if let Some(component) = &target.component {
        crepuscularity_lvgl::render_component_file_to_lvgl_xml(&template, component, &ctx)
    } else {
        let name = target
            .name
            .clone()
            .unwrap_or_else(|| target_id(target, idx));
        let root = match target.root.as_deref().unwrap_or("component") {
            "screen" => crepuscularity_lvgl::LvglRoot::Screen,
            "component" => crepuscularity_lvgl::LvglRoot::Component,
            other => {
                return Err(format!(
                    "lvgl root must be component or screen, got {other:?}"
                ))
            }
        };
        crepuscularity_lvgl::render_template_to_lvgl_xml_with_options(
            &template,
            &ctx,
            &crepuscularity_lvgl::LvglOptions { name, root },
        )
    }
    .map_err(|e| e.to_string())?;
    Ok(artifact(target, idx, "lvgl", contents, base_dir))
}

fn build_native(target: &Target, base_dir: &Path, idx: usize) -> Result<Artifact, String> {
    let template_path = template_path(target, base_dir, "ui.crepus");
    let template = std::fs::read_to_string(&template_path)
        .map_err(|e| format!("read {}: {e}", template_path.display()))?;
    let ctx = target_context(
        target,
        base_dir,
        template_path.parent().map(Path::to_path_buf),
    )?;
    let ir = if let Some(component) = &target.component {
        crepuscularity_native::render_component_file_to_ir(&template, component, &ctx)
    } else {
        let nodes =
            parse_template_with_path(&template, Some(&template_path)).map_err(|e| e.to_string())?;
        crepuscularity_native::render_nodes_to_ir(&nodes, &ctx)
    }
    .map_err(|e| e.to_string())?;
    let contents = if target.root.as_deref() == Some("pretty") {
        crepuscularity_native::to_json_pretty(&ir).map_err(|e| e.to_string())?
    } else {
        crepuscularity_native::to_json(&ir).map_err(|e| e.to_string())?
    };
    Ok(artifact(target, idx, "native", contents, base_dir))
}

fn target_dir(target: &Target, base_dir: &Path) -> PathBuf {
    target
        .site
        .as_ref()
        .or(target.path.as_ref())
        .or(target.app.as_ref())
        .map(|path| absolutize(base_dir, path))
        .unwrap_or_else(|| base_dir.to_path_buf())
}

fn template_path(target: &Target, base_dir: &Path, default: &str) -> PathBuf {
    target
        .template
        .as_ref()
        .map(|path| absolutize(base_dir, path))
        .or_else(|| {
            target
                .entry
                .as_ref()
                .map(|entry| target_dir(target, base_dir).join(entry))
        })
        .unwrap_or_else(|| target_dir(target, base_dir).join(default))
}

fn target_context(
    target: &Target,
    manifest_dir: &Path,
    base_dir: Option<PathBuf>,
) -> Result<TemplateContext, String> {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = base_dir;
    if let Some(path) = &target.ctx {
        let ctx_path = absolutize(manifest_dir, path);
        let raw = std::fs::read_to_string(&ctx_path)
            .map_err(|e| format!("read {}: {e}", ctx_path.display()))?;
        let table = raw
            .parse::<toml::Table>()
            .map_err(|e| format!("parse {}: {e}", ctx_path.display()))?;
        for (key, value) in table {
            ctx.set(key, toml_to_template_value(&value));
        }
    }
    for (key, value) in &target.vars {
        ctx.set(key, toml_to_template_value(value));
    }
    Ok(ctx)
}

fn artifact(
    target: &Target,
    idx: usize,
    target_type: &str,
    contents: String,
    base_dir: &Path,
) -> Artifact {
    Artifact {
        id: target_id(target, idx),
        target_type: target_type.into(),
        path: target.out.as_ref().map(|out| absolutize(base_dir, out)),
        contents,
    }
}

fn target_id(target: &Target, idx: usize) -> String {
    target.id.clone().unwrap_or_else(|| format!("target-{idx}"))
}

fn absolutize(base: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn toml_to_template_value(value: &toml::Value) -> TemplateValue {
    match value {
        toml::Value::String(s) => TemplateValue::Str(s.clone()),
        toml::Value::Integer(n) => TemplateValue::Int(*n),
        toml::Value::Float(n) => TemplateValue::Float(*n),
        toml::Value::Boolean(b) => TemplateValue::Bool(*b),
        toml::Value::Array(items) => TemplateValue::List(
            items
                .iter()
                .map(|item| {
                    let mut row = TemplateContext::new();
                    row.set("value", toml_to_template_value(item));
                    row
                })
                .collect(),
        ),
        toml::Value::Table(table) => {
            let mut ctx = TemplateContext::new();
            for (key, value) in table {
                ctx.set(key, toml_to_template_value(value));
            }
            TemplateValue::Scope(ctx)
        }
        toml::Value::Datetime(dt) => TemplateValue::Str(dt.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_builds_lvgl_artifact() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("ui.crepus"),
            "div #panel\n  h1 text-white\n    \"{device}\"",
        )
        .expect("write template");
        let manifest = Manifest::parse(
            r#"
[[targets]]
type = "lvgl"
id = "dash"
template = "ui.crepus"
out = "dist/dash.xml"
name = "Dash"
root = "screen"

[targets.vars]
device = "STM32F411"
"#,
        )
        .expect("parse manifest");
        let artifacts = build_manifest(&manifest, dir.path(), Some("dash")).expect("build");
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].path, Some(dir.path().join("dist/dash.xml")));
        assert!(artifacts[0].contents.contains(r#"<screen name="Dash">"#));
        assert!(artifacts[0].contents.contains("STM32F411"));
    }
}
