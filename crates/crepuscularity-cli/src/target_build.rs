use std::path::{Path, PathBuf};

use console::style;
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_embedded::{write_ppm, Ui};
use crepuscularity_lvgl::{
    render_component_file_to_lvgl_xml_with_options, render_nodes_to_lvgl_xml, LvglOptions, LvglRoot,
};
use crepuscularity_native::{
    render_component_file_to_ir, render_template_to_ir_with_path, to_json, to_json_pretty,
};

use crate::build_options::BuildOptions;
use crate::crepus_toml::ResolvedTarget;
use crate::error::CrepusCliError;
use crate::ui;

pub struct ManifestBuildArgs {
    pub options: BuildOptions,
    pub target_id: Option<String>,
    pub selector: Option<String>,
    pub manifest: Option<PathBuf>,
    pub all: bool,
}

pub(crate) fn has_manifest_targets(manifest: Option<PathBuf>) -> Result<bool, CrepusCliError> {
    let targets =
        crate::crepus_toml::load_manifest_targets(manifest).map_err(CrepusCliError::context)?;
    Ok(targets.is_some_and(|targets| !targets.is_empty()))
}

pub(crate) fn execute(args: ManifestBuildArgs) -> Result<(), CrepusCliError> {
    let ManifestBuildArgs {
        options,
        target_id,
        selector,
        manifest,
        all,
    } = args;
    if target_id.is_some() && selector.is_some() {
        return Err(CrepusCliError::context(
            "use either --target ID or a positional selector, not both",
        ));
    }
    if all && (target_id.is_some() || selector.is_some()) {
        return Err(CrepusCliError::context(
            "use either --all or a target selector, not both",
        ));
    }
    let targets = crate::crepus_toml::load_manifest_targets(manifest)
        .map_err(CrepusCliError::context)?
        .ok_or_else(|| CrepusCliError::context("no crepus.toml found"))?;
    if targets.is_empty() {
        return Err(CrepusCliError::context(
            "crepus.toml has no [[targets]] entries",
        ));
    }
    let picked = if all {
        targets
    } else if let Some(sel) = selector.as_deref() {
        pick_targets_by_selector(&targets, sel)?
    } else {
        crate::crepus_toml::pick_targets(&targets, target_id.as_deref())
            .map_err(CrepusCliError::context)?
    };
    validate_target_outputs(&picked)?;
    for target in picked {
        build_target(&target, options)?;
    }
    Ok(())
}

fn pick_targets_by_selector(
    targets: &[ResolvedTarget],
    selector: &str,
) -> Result<Vec<ResolvedTarget>, CrepusCliError> {
    if targets.iter().any(|target| target.id == selector) {
        return crate::crepus_toml::pick_targets(targets, Some(selector))
            .map_err(CrepusCliError::context);
    }
    let normalized = normalize_selector(selector);
    let picked: Vec<ResolvedTarget> = targets
        .iter()
        .filter(|target| normalize_selector(&target.target_type) == normalized)
        .cloned()
        .collect();
    if !picked.is_empty() {
        return Ok(picked);
    }
    let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    Err(CrepusCliError::context(format!(
        "no target id or type {selector:?} (ids: {ids:?})"
    )))
}

fn normalize_selector(selector: &str) -> &str {
    match selector {
        "extension" | "browser-extension" | "web-extension" => "webext",
        "ir" => "native",
        other => other,
    }
}

// Only known primary output files participate; webext's multi-asset output is separate.
fn output_file_path(target: &ResolvedTarget) -> Option<PathBuf> {
    let extension = match target.target_type.as_str() {
        "native" | "ir" => "json",
        "lvgl" => "xml",
        "embedded" => "ppm",
        "web" => {
            return Some(
                target
                    .out
                    .clone()
                    .unwrap_or_else(|| target.dir.join("dist"))
                    .join("index.html"),
            )
        }
        _ => return None,
    };
    Some(target.out.clone().unwrap_or_else(|| {
        target
            .dir
            .join("dist")
            .join(format!("{}.{extension}", target.id))
    }))
}

fn validate_target_outputs(targets: &[ResolvedTarget]) -> Result<(), CrepusCliError> {
    let outputs: Vec<_> = targets
        .iter()
        .filter_map(|target| {
            output_file_path(target)
                .map(|path| (format!("{} ({})", target.id, target.target_type), path))
        })
        .collect();
    crepuscularity_core::output_paths::validate_output_paths(
        outputs
            .iter()
            .map(|(label, path)| (label.as_str(), path.as_path())),
    )
    .map_err(CrepusCliError::context)
}

fn build_target(target: &ResolvedTarget, options: BuildOptions) -> Result<(), CrepusCliError> {
    match target.target_type.as_str() {
        "web" => {
            crate::web::build_site_wasm(&crate::web::WebBuildArgs {
                site_dir: Some(target.dir.clone()),
                out_dir: target.out.clone(),
                entry: target.entry.clone(),
                target_id: None,
                manifest: None,
                meta: Some(target.web.clone()),
                options,
            });
            Ok(())
        }
        "webext" => {
            if let Some(manifest) = &target.webext {
                crate::webext::build_app_target(
                    &target.dir,
                    manifest,
                    &target.webext_config,
                    options,
                );
            } else {
                crate::webext::build_app_path(&target.dir, options);
            }
            Ok(())
        }
        "lvgl" => build_lvgl_target(target, options),
        "native" | "ir" => build_native_target(target, options),
        "embedded" => build_embedded_target(target, options),
        other => Err(CrepusCliError::context(format!(
            "unsupported target type {other:?}"
        ))),
    }
}

fn build_lvgl_target(
    target: &ResolvedTarget,
    _options: BuildOptions,
) -> Result<(), CrepusCliError> {
    let template_path = resolve_template_path(target);
    let template = std::fs::read_to_string(&template_path)
        .map_err(|e| CrepusCliError::io(e, template_path.clone()))?;
    let ctx = target_context(target, template_path.parent().map(PathBuf::from));
    let name = target
        .name
        .clone()
        .or_else(|| target.component.clone())
        .unwrap_or_else(|| target.id.clone());
    let root = match target.root.as_deref().unwrap_or("component") {
        "screen" => LvglRoot::Screen,
        "component" => LvglRoot::Component,
        other => {
            return Err(CrepusCliError::context(format!(
                "lvgl root must be component or screen, got {other:?}"
            )))
        }
    };
    let options = LvglOptions { name, root };
    let xml = if let Some(component) = &target.component {
        render_component_file_to_lvgl_xml_with_options(&template, component, &ctx, &options)
    } else {
        let nodes = crepuscularity_core::parse_template_with_path(&template, Some(&template_path))
            .map_err(|e| {
                CrepusCliError::context(format!("parse lvgl target {:?}: {e}", target.id))
            })?;
        render_nodes_to_lvgl_xml(&nodes, &ctx, &options)
    }
    .map_err(|e| CrepusCliError::context(format!("render lvgl target {:?}: {e}", target.id)))?;
    let out = output_file_path(target).expect("render target has an output file");
    write_output(&out, xml.as_bytes());
    eprintln!(
        "{} {} {}",
        ui::ok(),
        style(&target.id).cyan().bold(),
        style(out.display().to_string()).dim()
    );
    Ok(())
}

fn build_native_target(
    target: &ResolvedTarget,
    _options: BuildOptions,
) -> Result<(), CrepusCliError> {
    let template_path = resolve_template_path(target);
    let template = std::fs::read_to_string(&template_path)
        .map_err(|e| CrepusCliError::io(e, template_path.clone()))?;
    let ctx = target_context(target, template_path.parent().map(PathBuf::from));
    let ir = if let Some(component) = &target.component {
        render_component_file_to_ir(&template, component, &ctx)
    } else {
        render_template_to_ir_with_path(&template, &ctx, Some(&template_path))
    }
    .map_err(|e| CrepusCliError::context(format!("render native target {:?}: {e}", target.id)))?;
    let json = if target.root.as_deref() == Some("pretty") {
        to_json_pretty(&ir)
    } else {
        to_json(&ir)
    }
    .map_err(|e| {
        CrepusCliError::context(format!("serialize native target {:?}: {e}", target.id))
    })?;
    let out = output_file_path(target).expect("render target has an output file");
    write_output(&out, json.as_bytes());
    eprintln!(
        "{} {} {}",
        ui::ok(),
        style(&target.id).cyan().bold(),
        style(out.display().to_string()).dim()
    );
    Ok(())
}

fn build_embedded_target(
    target: &ResolvedTarget,
    _options: BuildOptions,
) -> Result<(), CrepusCliError> {
    let width = target
        .width
        .ok_or_else(|| CrepusCliError::context("embedded target needs width"))?;
    let height = target
        .height
        .ok_or_else(|| CrepusCliError::context("embedded target needs height"))?;
    let template_path = resolve_template_path(target);
    let ctx = target_context(target, template_path.parent().map(PathBuf::from));
    let mut ui_view = Ui::from_path(width, height, &template_path).map_err(|e| {
        CrepusCliError::context(format!("load embedded target {:?}: {e}", target.id))
    })?;
    if let Some(component) = &target.component {
        ui_view.set_component(component.clone());
    }
    for (key, value) in &ctx.vars {
        ui_view.set(key, value.clone());
    }
    let screen = ui_view.screen();
    let mut ppm =
        crepuscularity_embedded::Rgb888Buffer::new(screen, crepuscularity_embedded::DEFAULT_BG);
    ui_view.render_into(&mut ppm).map_err(|e| {
        CrepusCliError::context(format!("render embedded target {:?}: {e}", target.id))
    })?;
    let out = output_file_path(target).expect("render target has an output file");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CrepusCliError::io(e, parent.to_path_buf()))?;
    }
    write_ppm(&out, &ppm)
        .map_err(|e| CrepusCliError::context(format!("write {}: {e}", out.display())))?;
    eprintln!(
        "{} {} {}",
        ui::ok(),
        style(&target.id).cyan().bold(),
        style(out.display().to_string()).dim()
    );
    Ok(())
}

fn resolve_template_path(target: &ResolvedTarget) -> PathBuf {
    target
        .template
        .clone()
        .or_else(|| target.entry.as_ref().map(|entry| target.dir.join(entry)))
        .unwrap_or_else(|| target.dir.join("ui.crepus"))
}

fn target_context(target: &ResolvedTarget, base_dir: Option<PathBuf>) -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = base_dir;
    if let Some(path) = &target.ctx {
        load_ctx_file(path, &mut ctx);
    }
    for (key, value) in &target.vars {
        ctx.set(key, toml_to_template_value(value));
    }
    ctx
}

fn load_ctx_file(path: &PathBuf, ctx: &mut TemplateContext) {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| ui::error(&format!("read {}: {e}", path.display())));
    let table = raw
        .parse::<toml::Table>()
        .unwrap_or_else(|e| ui::error(&format!("parse {}: {e}", path.display())));
    for (key, value) in table {
        ctx.set(key, toml_to_template_value(&value));
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

fn write_output(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| ui::error(&format!("create {}: {e}", parent.display())));
    }
    std::fs::write(path, bytes)
        .unwrap_or_else(|e| ui::error(&format!("write {}: {e}", path.display())));
}

#[cfg(test)]
mod selector_tests {
    use super::*;

    #[test]
    fn matching_duplicate_id_does_not_fall_back_to_type_selection() {
        let manifest = crate::crepus_toml::CrepusManifest::parse(
            r#"[[targets]]
type = "web"
id = "web"
site = "one"
[[targets]]
type = "web"
id = "web"
site = "two"
"#,
        )
        .unwrap();
        let mut targets = manifest.resolved_targets(Path::new("/manifest"));
        let error = pick_targets_by_selector(&targets, "web").unwrap_err();
        assert!(error.to_string().contains("ambiguous"), "{error}");
        targets[0].id = "one".into();
        targets[1].id = "two".into();
        assert_eq!(pick_targets_by_selector(&targets, "web").unwrap().len(), 2);
        assert_eq!(pick_targets_by_selector(&targets, "one").unwrap().len(), 1);
    }
    fn targets(source: &str, root: &Path) -> Vec<ResolvedTarget> {
        crate::crepus_toml::CrepusManifest::parse(source)
            .unwrap()
            .resolved_targets(root)
    }

    #[test]
    fn output_preflight_rejects_duplicate_explicit_files_before_any_build() {
        let root = tempfile::tempdir().unwrap();
        let selected = targets(
            r#"[[targets]]
type = "native"
id = "one"
out = "shared/result"
[[targets]]
type = "lvgl"
id = "two"
out = "shared/result"
"#,
            root.path(),
        );
        assert!(validate_target_outputs(&selected).is_err());
        assert!(!root.path().join("shared").exists());
    }

    #[test]
    fn output_preflight_rejects_native_ir_default_alias_collisions() {
        let root = tempfile::tempdir().unwrap();
        let selected = targets(
            r#"[[targets]]
type = "native"
id = "same"
[[targets]]
type = "ir"
id = "same"
"#,
            root.path(),
        );
        assert!(validate_target_outputs(&selected).is_err());
    }

    #[test]
    fn output_preflight_keeps_distinct_files_in_a_shared_directory() {
        let root = tempfile::tempdir().unwrap();
        let selected = targets(
            r#"[[targets]]
type = "native"
id = "same"
[[targets]]
type = "lvgl"
id = "same"
[[targets]]
type = "embedded"
id = "same"
[[targets]]
type = "web"
id = "web"
"#,
            root.path(),
        );
        validate_target_outputs(&selected).unwrap();
        assert!(output_file_path(&selected[3])
            .unwrap()
            .ends_with("dist/index.html"));
    }
}
