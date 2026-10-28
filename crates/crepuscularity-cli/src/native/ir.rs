use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::{DriverCache, Fingerprint};
use crepuscularity_native::{
    generate_native_source, render_component_file_to_ir, render_from_files, render_template_to_ir,
    render_template_to_ir_with_path, to_json, to_json_pretty, NativeCodegenTarget,
};
use serde::Deserialize;
use serde_json::Value;

use super::{check_template_size, prepend_kotlin_package};
use crate::error::CrepusCliError;
use crate::ui;

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum CodegenPlatform {
    #[value(name = "swiftui", alias = "swift", alias = "ios")]
    SwiftUi,
    #[value(name = "compose", alias = "kotlin", alias = "android")]
    Compose,
}

impl From<CodegenPlatform> for NativeCodegenTarget {
    fn from(p: CodegenPlatform) -> Self {
        match p {
            CodegenPlatform::SwiftUi => NativeCodegenTarget::SwiftUi,
            CodegenPlatform::Compose => NativeCodegenTarget::Compose,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrEnvelope {
    entry: Option<String>,
    files: Option<HashMap<String, String>>,
    template: Option<String>,
    filename: Option<PathBuf>,
    context: Option<Value>,
    component: Option<String>,
    pretty: Option<bool>,
    base_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct IrArgs {
    pub file: Option<PathBuf>,
    #[arg(long)]
    pub component: Option<String>,
    #[arg(long)]
    pub ctx: Option<PathBuf>,
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,
    #[arg(long)]
    pub pretty: bool,
    #[arg(long)]
    pub stdin: bool,
    #[arg(long = "stdin-json")]
    pub stdin_json: bool,
    #[arg(long)]
    pub base_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct SyncArgs {
    pub template: PathBuf,
    #[arg(long, default_value = ".")]
    pub dir: PathBuf,
    #[arg(long)]
    pub out: Vec<PathBuf>,
    #[arg(long = "no-defaults")]
    pub no_defaults: bool,
    #[arg(long)]
    pub component: Option<String>,
    #[arg(long)]
    pub ctx: Option<PathBuf>,
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CodegenArgs {
    pub template: Option<PathBuf>,
    #[arg(long)]
    pub platform: Option<CodegenPlatform>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub view_name: Option<String>,
    #[arg(long)]
    pub component: Option<String>,
    #[arg(long)]
    pub ctx: Option<PathBuf>,
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,
}

pub fn parse_kv_vars(vars: &[String]) -> Vec<(String, String)> {
    vars.iter()
        .map(|raw| {
            raw.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .ok_or_else(|| format!("--var expects key=value, got: {raw}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| ui::error(&e))
}

pub fn run_ir_parsed(parsed: IrArgs) -> Result<String, CrepusCliError> {
    if parsed.stdin && parsed.stdin_json {
        return Err(CrepusCliError::context(
            "--stdin and --stdin-json are mutually exclusive",
        ));
    }

    let mut ctx = TemplateContext::new();
    if let Some(path) = &parsed.ctx {
        load_json_ctx(path, &mut ctx)?;
    }
    for (key, raw) in parse_kv_vars(&parsed.vars) {
        ctx.set(key, parse_var_value(&raw));
    }

    if parsed.stdin_json {
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(|e| CrepusCliError::context(format!("read stdin: {e}")))?;
        check_template_size(raw.len()).map_err(CrepusCliError::context)?;
        let env: IrEnvelope = serde_json::from_str(&raw)
            .map_err(|e| CrepusCliError::context(format!("stdin JSON: {e}")))?;
        if let Some(value) = env.context {
            merge_json_ctx(&value, &mut ctx)?;
        }
        if let Some(base_dir) = env.base_dir {
            ctx.base_dir = Some(base_dir);
        }
        let pretty = env.pretty.unwrap_or(parsed.pretty);
        let ir = if let (Some(files), Some(entry)) = (env.files, env.entry) {
            render_from_files(&files, &entry, &ctx)
                .map_err(|e| CrepusCliError::context(e.to_string()))?
        } else if let Some(template) = env.template {
            if let Some(component) = env.component {
                render_component_file_to_ir(&template, &component, &ctx)
                    .map_err(|e| CrepusCliError::context(e.to_string()))?
            } else {
                render_template_to_ir_with_path(&template, &ctx, env.filename.as_deref())
                    .map_err(|e| CrepusCliError::context(e.to_string()))?
            }
        } else {
            return Err(CrepusCliError::context(
                "stdin JSON must include files+entry or template",
            ));
        };
        return stringify_ir(&ir, pretty);
    }

    if parsed.stdin {
        let mut template = String::new();
        std::io::stdin()
            .read_to_string(&mut template)
            .map_err(|e| CrepusCliError::context(format!("read stdin: {e}")))?;
        check_template_size(template.len()).map_err(CrepusCliError::context)?;
        ctx.base_dir = parsed.base_dir;
        let ir = if let Some(component) = parsed.component {
            render_component_file_to_ir(&template, &component, &ctx)
                .map_err(|e| CrepusCliError::context(e.to_string()))?
        } else {
            render_template_to_ir(&template, &ctx)
                .map_err(|e| CrepusCliError::context(e.to_string()))?
        };
        return stringify_ir(&ir, parsed.pretty);
    }

    let path = parsed.file.ok_or_else(|| {
        CrepusCliError::context("Usage: crepus native ir <file.crepus> [--component Name] [--ctx FILE] [--var k=v] [--pretty]")
    })?;
    let content = fs::read_to_string(&path).map_err(|e| CrepusCliError::io(e, path.clone()))?;
    check_template_size(content.len()).map_err(CrepusCliError::context)?;
    ctx.base_dir = path.parent().map(Path::to_path_buf);
    let ir = if let Some(component) = parsed.component {
        render_component_file_to_ir(&content, &component, &ctx)
            .map_err(|e| CrepusCliError::context(e.to_string()))?
    } else {
        render_template_to_ir(&content, &ctx).map_err(|e| CrepusCliError::context(e.to_string()))?
    };
    stringify_ir(&ir, parsed.pretty)
}

pub fn load_json_ctx(path: &Path, ctx: &mut TemplateContext) -> Result<(), CrepusCliError> {
    let raw = fs::read_to_string(path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|e| format!("context JSON {}: {e}", path.display()))?;
    merge_json_ctx(&value, ctx)
}

pub fn merge_json_ctx(value: &Value, ctx: &mut TemplateContext) -> Result<(), CrepusCliError> {
    let Some(obj) = value.as_object() else {
        return Err(CrepusCliError::context("context must be a JSON object"));
    };
    for (key, value) in obj {
        ctx.set(key.clone(), json_to_template_value(value)?);
    }
    Ok(())
}

pub fn json_to_template_value(value: &Value) -> Result<TemplateValue, CrepusCliError> {
    match value {
        Value::Null => Ok(TemplateValue::Null),
        Value::Bool(v) => Ok(TemplateValue::Bool(*v)),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                Ok(TemplateValue::Int(n))
            } else if let Some(n) = v.as_f64() {
                Ok(TemplateValue::Float(n))
            } else {
                Err(CrepusCliError::context(format!("unsupported number: {v}")))
            }
        }
        Value::String(v) => Ok(TemplateValue::Str(v.clone())),
        Value::Array(values) => {
            let mut items = Vec::new();
            for item in values {
                match item {
                    Value::Object(obj) => {
                        // Each object becomes a child context
                        let mut child = TemplateContext::new();
                        for (key, value) in obj {
                            child.set(key.clone(), json_to_template_scalar(value)?);
                        }
                        items.push(child);
                    }
                    Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
                        // Scalar values become item contexts with "value" key
                        let mut child = TemplateContext::new();
                        child.set("value", json_to_template_scalar(item)?);
                        items.push(child);
                    }
                    _ => return Err(CrepusCliError::context("unsupported array item type")),
                }
            }
            Ok(TemplateValue::List(items))
        }
        Value::Object(_) => Err(CrepusCliError::context(
            "context object values are only supported inside arrays",
        )),
    }
}

pub fn json_to_template_scalar(value: &Value) -> Result<TemplateValue, CrepusCliError> {
    match value {
        Value::Array(_) | Value::Object(_) => Err(CrepusCliError::context(
            "loop item fields must be scalar JSON values",
        )),
        _ => json_to_template_value(value),
    }
}

pub fn parse_var_value(raw: &str) -> TemplateValue {
    match raw {
        "true" => TemplateValue::Bool(true),
        "false" => TemplateValue::Bool(false),
        "null" => TemplateValue::Null,
        _ => raw
            .parse::<i64>()
            .map(TemplateValue::Int)
            .or_else(|_| raw.parse::<f64>().map(TemplateValue::Float))
            .unwrap_or_else(|_| TemplateValue::Str(raw.to_string())),
    }
}

pub fn stringify_ir(
    ir: &crepuscularity_native::ViewIr,
    pretty: bool,
) -> Result<String, CrepusCliError> {
    if pretty {
        to_json_pretty(ir).map_err(|e| CrepusCliError::context(format!("serialize IR: {e}")))
    } else {
        to_json(ir).map_err(|e| CrepusCliError::context(format!("serialize IR: {e}")))
    }
}

pub fn sync_native_fixture_inner(parsed: SyncArgs) -> Result<(), CrepusCliError> {
    if !parsed.dir.exists() {
        return Err(CrepusCliError::context(format!(
            "native scaffold dir not found: {}",
            parsed.dir.display()
        )));
    }
    let root = fs::canonicalize(&parsed.dir).unwrap_or(parsed.dir);
    let template_path = resolve_template_path(&root, &parsed.template);
    let template = fs::read_to_string(&template_path)
        .map_err(|e| CrepusCliError::io(e, template_path.clone()))?;

    let mut ctx = TemplateContext::new();
    if let Some(path) = &parsed.ctx {
        load_json_ctx(path, &mut ctx)?;
    }
    for (key, raw) in parse_kv_vars(&parsed.vars) {
        ctx.set(key, parse_var_value(&raw));
    }
    ctx.base_dir = template_path.parent().map(Path::to_path_buf);

    let component_ref = parsed.component.clone();
    let ir = if let Some(component) = &parsed.component {
        render_component_file_to_ir(&template, component, &ctx)
            .map_err(|e| CrepusCliError::context(e.to_string()))?
    } else {
        render_template_to_ir(&template, &ctx)
            .map_err(|e| CrepusCliError::context(e.to_string()))?
    };
    let mut json = stringify_ir(&ir, parsed.pretty)?;
    if !json.ends_with('\n') {
        json.push('\n');
    }

    // Cache skip: avoid rewriting unchanged fixtures (prevents git dirty
    // markers on `crepus native sync` re-runs).
    let cache = DriverCache::open(&root);
    let fp = Fingerprint::new(&template, component_ref.as_deref(), "native-ir");

    let mut written = 0;
    if !parsed.no_defaults {
        for path in default_fixture_output_paths(&root) {
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    if path.is_file() && cache.is_up_to_date(&fp, &json) {
                        written += 1;
                        continue;
                    }
                    fs::write(&path, &json).map_err(|e| CrepusCliError::io(e, path.clone()))?;
                    written += 1;
                }
            }
        }
    }
    for path in explicit_fixture_output_paths(&root, &parsed.out) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| CrepusCliError::io(e, parent.to_path_buf()))?;
        }
        if path.is_file() && cache.is_up_to_date(&fp, &json) {
            written += 1;
            continue;
        }
        fs::write(&path, &json).map_err(|e| CrepusCliError::io(e, path.clone()))?;
        written += 1;
    }
    cache.record(&fp, &json);
    if written == 0 {
        return Err(CrepusCliError::context(format!(
            "no native fixture directories found under {}",
            root.display()
        )));
    }
    ui::success(&format!(
        "synced View IR fixture from {}",
        template_path.display()
    ));
    Ok(())
}

pub fn codegen_native_source_inner(parsed: CodegenArgs) -> Result<PathBuf, CrepusCliError> {
    let template_path = parsed.template.ok_or_else(|| {
        CrepusCliError::context("Usage: crepus native codegen <file.crepus> --platform swiftui|compose --out DIR --view-name NAME")
    })?;
    let platform = parsed
        .platform
        .ok_or_else(|| CrepusCliError::context("--platform swiftui|compose is required"))?;
    let out_dir = parsed
        .out
        .ok_or_else(|| CrepusCliError::context("--out DIR is required"))?;
    let view_name = parsed
        .view_name
        .ok_or_else(|| CrepusCliError::context("--view-name NAME is required"))?;

    let template = fs::read_to_string(&template_path)
        .map_err(|e| CrepusCliError::io(e, template_path.clone()))?;

    let mut ctx = TemplateContext::new();
    if let Some(path) = &parsed.ctx {
        load_json_ctx(path, &mut ctx)?;
    }
    for (key, raw) in parse_kv_vars(&parsed.vars) {
        ctx.set(key, parse_var_value(&raw));
    }
    ctx.base_dir = template_path.parent().map(Path::to_path_buf);

    let ir = if let Some(component) = &parsed.component {
        render_component_file_to_ir(&template, component, &ctx)
            .map_err(|e| CrepusCliError::context(e.to_string()))?
    } else {
        render_template_to_ir(&template, &ctx)
            .map_err(|e| CrepusCliError::context(e.to_string()))?
    };
    let mut source = generate_native_source(&ir, platform.into(), &view_name);
    if !source.ends_with('\n') {
        source.push('\n');
    }
    fs::create_dir_all(&out_dir).map_err(|e| CrepusCliError::io(e, out_dir.clone()))?;
    let ext = match platform {
        CodegenPlatform::SwiftUi => "swift",
        CodegenPlatform::Compose => "kt",
    };
    let path = out_dir.join(format!("{}.{}", view_name, ext));
    fs::write(&path, source).map_err(|e| CrepusCliError::io(e, path.clone()))?;
    if platform == CodegenPlatform::Compose && is_native_shell_android_generated_dir(&out_dir) {
        prepend_kotlin_package(&path);
    }
    ui::success(&format!("generated native source at {}", path.display()));
    Ok(path)
}

pub fn is_native_shell_android_generated_dir(path: &Path) -> bool {
    path.ends_with("android/app/src/main/java/dev/crepuscularity/nativeshell/generated")
}

pub fn resolve_template_path(root: &Path, template: &Path) -> PathBuf {
    if template.is_absolute() {
        return template.to_path_buf();
    }
    let rooted = root.join(template);
    if rooted.exists() {
        rooted
    } else {
        template.to_path_buf()
    }
}

pub fn default_fixture_output_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("fixture.json"),
        root.join("ios/Sources/NativeShell/fixture.json"),
        root.join("NativeShell/Sources/NativeShell/fixture.json"),
        root.join("android/app/src/main/assets/fixture.json"),
    ]
}

pub fn explicit_fixture_output_paths(root: &Path, explicit: &[PathBuf]) -> Vec<PathBuf> {
    explicit
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_native::{ViewIr, ViewNode};

    #[test]
    fn test_stringify_ir_pretty() {
        let ir = ViewIr {
            version: 5,
            root: vec![ViewNode::Text {
                content: "hello".into(),
                bind: None,
                style: None,
            }],
        };

        let json = stringify_ir(&ir, true).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("kind"));
        assert!(json.contains("text"));
        assert!(json.contains("\n"));
    }

    #[test]
    fn test_stringify_ir_compact() {
        let ir = ViewIr {
            version: 5,
            root: vec![ViewNode::Text {
                content: "hello".into(),
                bind: None,
                style: None,
            }],
        };

        let json = stringify_ir(&ir, false).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("kind"));
        assert!(json.contains("text"));
        assert!(!json.contains("\n"));
    }
}
