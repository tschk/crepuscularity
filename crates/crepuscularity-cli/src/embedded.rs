//! `crepus embedded` — validate templates and optional PPM snapshots for debugging.

use std::fs;
use std::path::{Path, PathBuf};

use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::parser::{parse_component_file, parse_template_with_path};
use crepuscularity_embedded::{write_ppm, Ui};
use serde_json::Value;

use crate::cli::EmbeddedCommands;

pub fn execute(cmd: EmbeddedCommands) {
    match cmd {
        EmbeddedCommands::Check { file, component } => match run_check_file(&file, component) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        EmbeddedCommands::Snapshot {
            file,
            width,
            height,
            out,
            ctx,
            vars,
            component,
        } => match run_snapshot_parsed(SnapshotArgs {
            path: file,
            width,
            height,
            out,
            ctx_file: ctx,
            vars: parse_var_pairs(&vars),
            component,
        }) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
    }
}

fn parse_var_pairs(var_strings: &[String]) -> Vec<(String, String)> {
    var_strings
        .iter()
        .map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .unwrap_or_else(|| {
                    eprintln!("--var expects key=value, got: {kv}");
                    std::process::exit(1);
                })
        })
        .collect()
}

fn run_check_file(path: &Path, component: Option<String>) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if let Some(name) = component {
        let file = parse_component_file(&content).map_err(|e| e.to_string())?;
        file.components
            .get(&name)
            .ok_or_else(|| format!("component not found: {name}"))?;
    } else {
        parse_template_with_path(&content, Some(path)).map_err(|e| e.to_string())?;
    }
    println!("ok: {}", path.display());
    Ok(())
}

fn run_snapshot_parsed(parsed: SnapshotArgs) -> Result<(), String> {
    let mut ctx = TemplateContext::new();
    if let Some(path) = &parsed.ctx_file {
        load_json_ctx(path, &mut ctx)?;
    }
    for (key, raw) in parsed.vars {
        ctx.set(key, parse_var_value(&raw));
    }

    let content = fs::read_to_string(&parsed.path)
        .map_err(|e| format!("read {}: {e}", parsed.path.display()))?;
    let mut ui = Ui::new(parsed.width, parsed.height, &content);
    if let Some(name) = parsed.component {
        ui.set_component(name);
    }
    for (k, v) in ctx.vars {
        ui.set(k, v);
    }

    let screen = ui.screen();
    let mut ppm =
        crepuscularity_embedded::Rgb888Buffer::new(screen, crepuscularity_embedded::DEFAULT_BG);
    ui.render_into(&mut ppm)?;
    write_ppm(&parsed.out, &ppm)?;
    Ok(())
}

struct SnapshotArgs {
    path: PathBuf,
    width: u16,
    height: u16,
    out: PathBuf,
    ctx_file: Option<PathBuf>,
    vars: Vec<(String, String)>,
    component: Option<String>,
}

fn load_json_ctx(path: &Path, ctx: &mut TemplateContext) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|e| format!("context JSON {}: {e}", path.display()))?;
    merge_json_ctx(&value, ctx)
}

fn merge_json_ctx(value: &Value, ctx: &mut TemplateContext) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err("context must be a JSON object".to_string());
    };
    for (key, value) in obj {
        ctx.set(key.clone(), json_to_template_value(value)?);
    }
    Ok(())
}

fn json_to_template_value(value: &Value) -> Result<TemplateValue, String> {
    match value {
        Value::Null => Ok(TemplateValue::Null),
        Value::Bool(v) => Ok(TemplateValue::Bool(*v)),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                Ok(TemplateValue::Int(n))
            } else if let Some(n) = v.as_f64() {
                Ok(TemplateValue::Float(n))
            } else {
                Err(format!("unsupported number: {v}"))
            }
        }
        Value::String(v) => Ok(TemplateValue::Str(v.clone())),
        Value::Array(values) => {
            let mut items = Vec::new();
            for item in values {
                let Some(obj) = item.as_object() else {
                    return Err("context arrays must contain objects".to_string());
                };
                let mut child = TemplateContext::new();
                for (key, value) in obj {
                    child.set(key.clone(), json_to_template_scalar(value)?);
                }
                items.push(child);
            }
            Ok(TemplateValue::List(items))
        }
        Value::Object(_) => {
            Err("context object values are only supported inside arrays".to_string())
        }
    }
}

fn json_to_template_scalar(value: &Value) -> Result<TemplateValue, String> {
    match value {
        Value::Array(_) | Value::Object(_) => {
            Err("loop item fields must be scalar JSON values".to_string())
        }
        _ => json_to_template_value(value),
    }
}

fn parse_var_value(raw: &str) -> TemplateValue {
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
