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
    if let Some(id) = target_id {
        if manifest
            .targets
            .iter()
            .enumerate()
            .filter(|(idx, target)| self::target_id(target, *idx) == id)
            .count()
            > 1
        {
            return Err(format!(
                "ambiguous target id {id:?}: multiple targets match"
            ));
        }
    }
    let targets = manifest.targets.iter().enumerate().filter(|(idx, target)| {
        target_id
            .map(|id| self::target_id(target, *idx) == id)
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
    crepuscularity_core::output_paths::validate_output_paths(artifacts.iter().filter_map(
        |artifact| {
            artifact
                .path
                .as_deref()
                .map(|path| (artifact.id.as_str(), path))
        },
    ))?;
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
    let mut artifact = artifact(target, idx, "web", contents, base_dir);
    artifact.path = artifact.path.map(|directory| directory.join("index.html"));
    Ok(artifact)
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
    let name = target
        .name
        .clone()
        .or_else(|| target.component.clone())
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
    let options = crepuscularity_lvgl::LvglOptions { name, root };
    let contents = if let Some(component) = &target.component {
        crepuscularity_lvgl::render_component_file_to_lvgl_xml_with_options(
            &template, component, &ctx, &options,
        )
    } else {
        let nodes =
            parse_template_with_path(&template, Some(&template_path)).map_err(|e| e.to_string())?;
        crepuscularity_lvgl::render_nodes_to_lvgl_xml(&nodes, &ctx, &options)
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
    fn overlapping_file_artifacts_are_rejected_before_unrelated_writes() {
        for reverse in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let sentinel = directory.path().join("sentinel.json");
            std::fs::write(&sentinel, "keep existing").unwrap();
            let parent = directory.path().join("new/result");
            let child = parent.join("view.json");
            let mut paths = [parent.clone(), child];
            if reverse {
                paths.reverse();
            }
            let artifacts: Vec<_> = std::iter::once(sentinel.clone())
                .chain(paths)
                .enumerate()
                .map(|(index, path)| Artifact {
                    id: format!("output-{index}"),
                    target_type: "native".into(),
                    path: Some(path),
                    contents: "replacement".into(),
                })
                .collect();
            let error = write_artifacts(&artifacts).unwrap_err();
            assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "keep existing");
            assert!(
                error.contains("output-1") && error.contains("output-2"),
                "{error}"
            );
            assert!(!directory.path().join("new").exists());
        }
    }

    #[test]
    fn colliding_artifacts_are_rejected_before_any_output_is_written() {
        let directory = tempfile::tempdir().unwrap();
        let shared = directory.path().join("shared.json");
        let unrelated = directory.path().join("first.json");
        std::fs::write(&shared, "keep existing").unwrap();
        let artifacts = vec![
            Artifact {
                id: "first".into(),
                target_type: "native".into(),
                path: Some(unrelated.clone()),
                contents: "first".into(),
            },
            Artifact {
                id: "one".into(),
                target_type: "native".into(),
                path: Some(shared.clone()),
                contents: "one".into(),
            },
            Artifact {
                id: "two".into(),
                target_type: "native".into(),
                path: Some(shared.clone()),
                contents: "two".into(),
            },
        ];
        let error = write_artifacts(&artifacts).unwrap_err();
        assert!(error.contains("one") && error.contains("two"), "{error}");
        assert_eq!(std::fs::read_to_string(&shared).unwrap(), "keep existing");
        assert!(!unrelated.exists());
        let mut distinct = artifacts[..2].to_vec();
        distinct.push(Artifact {
            path: None,
            ..artifacts[2].clone()
        });
        write_artifacts(&distinct).unwrap();
        assert_eq!(std::fs::read_to_string(&shared).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&unrelated).unwrap(), "first");
    }

    #[test]
    fn generated_manifest_ids_are_selectable_and_checked_for_collisions() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("one.crepus"), "span \"one\"").unwrap();
        let mut manifest = Manifest::parse(
            r#"[[targets]]
type = "native"
template = "missing.crepus"
[[targets]]
type = "native"
template = "one.crepus"
out = "one.json"
"#,
        )
        .unwrap();
        let selected = build_manifest(&manifest, directory.path(), Some("target-1")).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "target-1");
        manifest.targets[1].id = Some("target-0".into());
        let error = build_manifest(&manifest, directory.path(), Some("target-0")).unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
        assert!(!directory.path().join("one.json").exists());
    }

    #[test]
    fn named_manifest_selection_rejects_duplicate_ids_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("one.crepus"), "span \"one\"").unwrap();
        std::fs::write(directory.path().join("two.crepus"), "span \"two\"").unwrap();
        let manifest_path = directory.path().join("crepus.toml");
        std::fs::write(
            &manifest_path,
            r#"[[targets]]
type = "native"
id = "same"
template = "one.crepus"
out = "one.json"
[[targets]]
type = "native"
id = "same"
template = "two.crepus"
out = "two.json"
"#,
        )
        .unwrap();
        let error = write_manifest_file_target(&manifest_path, Some("same")).unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
        assert!(!directory.path().join("one.json").exists());
        assert!(!directory.path().join("two.json").exists());
        assert_eq!(build_manifest_file(&manifest_path).unwrap().len(), 2);
        let mut manifest =
            Manifest::parse(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.targets[1].id = Some("other".into());
        assert_eq!(
            build_manifest(&manifest, directory.path(), Some("same"))
                .unwrap()
                .len(),
            1
        );
        assert!(build_manifest(&manifest, directory.path(), Some("missing"))
            .unwrap_err()
            .contains("no target"));
    }

    #[test]
    fn web_manifest_writes_index_inside_manifest_relative_output_directory() {
        for existing in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let site = directory.path().join("site");
            std::fs::create_dir(&site).unwrap();
            std::fs::write(site.join("index.crepus"), "p \"Manifest page\"").unwrap();
            let output = directory.path().join("generated/web");
            if existing {
                std::fs::create_dir_all(&output).unwrap();
                std::fs::write(output.join("asset.txt"), "keep asset").unwrap();
                std::fs::write(output.join("index.html"), "old page").unwrap();
            }
            let manifest_path = directory.path().join("crepus.toml");
            std::fs::write(
                &manifest_path,
                r#"[[targets]]
type = "web"
id = "page"
site = "site"
out = "generated/web"
"#,
            )
            .unwrap();
            let artifacts = write_manifest_file_target(&manifest_path, Some("page")).unwrap();
            assert_eq!(artifacts.len(), 1);
            assert_eq!(artifacts[0].path, Some(output.join("index.html")));
            assert_eq!(
                std::fs::read_to_string(output.join("index.html")).unwrap(),
                artifacts[0].contents
            );
            assert!(artifacts[0].contents.contains("Manifest page"));
            assert!(site.join("index.crepus").is_file());
            if existing {
                assert_eq!(
                    std::fs::read_to_string(output.join("asset.txt")).unwrap(),
                    "keep asset"
                );
            }
            let manifest = Manifest::parse("[[targets]]\ntype = \"web\"\nsite = \"site\"").unwrap();
            assert!(
                build_manifest(&manifest, directory.path(), None).unwrap()[0]
                    .path
                    .is_none()
            );
        }
    }

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
    #[test]
    fn lvgl_manifest_uses_the_template_filename_frontend() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("view.vue"), "<script>const ignored = 1 < 2;</script><template><label>{{ title }}</label></template>").unwrap();
        let manifest = Manifest::parse(
            r#"[[targets]]
 type = "lvgl"
 id = "dash"
 template = "view.vue"
 name = "Dash"
 root = "screen"
 [targets.vars]
 title = "Vue label"
"#,
        )
        .unwrap();
        let artifacts = build_manifest(&manifest, directory.path(), Some("dash")).unwrap();
        assert!(artifacts[0].contents.contains("<screen name=\"Dash\">"));
        assert!(artifacts[0].contents.contains("text=\"Vue label\""));
        assert!(!artifacts[0].contents.contains("const ignored"));
    }
    #[test]
    fn named_lvgl_manifest_keeps_explicit_wrapper_options_and_validates_root() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("ui.crepus"),
            "--- Card\nspan \"Named content\"",
        )
        .unwrap();
        let source = r#"[[targets]]
 type = "lvgl"
 id = "dash"
 template = "ui.crepus"
 component = "Card"
 name = "CustomScreen"
 root = "screen"
"#;
        let manifest = Manifest::parse(source).unwrap();
        let artifacts = build_manifest(&manifest, directory.path(), Some("dash")).unwrap();
        assert!(
            artifacts[0]
                .contents
                .contains("<screen name=\"CustomScreen\">"),
            "{}",
            artifacts[0].contents
        );
        let manifest =
            Manifest::parse(&source.replace("root = \"screen\"", "root = \"invalid\"")).unwrap();
        assert!(build_manifest(&manifest, directory.path(), Some("dash")).is_err());
    }
}
