//! `crepus inspect` — dump AST / HTML / View IR / context for a `.crepus` file.
//!
//! Replaces the old standalone `crepus-dev` binary modes.

use std::path::{Path, PathBuf};

use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::parser::parse_template_with_path;
use crepuscularity_web::render_nodes_to_html;

use crate::cli::InspectMode;
use crate::ui;

pub fn execute(
    file: PathBuf,
    mode: InspectMode,
    vars: Vec<String>,
    bools: Vec<String>,
    ints: Vec<String>,
    width: f32,
    height: f32,
) {
    if !file.exists() {
        ui::error(&format!("template not found: {}", file.display()));
    }
    let source = std::fs::read_to_string(&file).unwrap_or_else(|e| {
        ui::error(&format!("failed to read {}: {e}", file.display()));
    });

    let mut ctx_vars: Vec<(String, TemplateValue)> = Vec::new();
    for raw in vars {
        let Some((k, v)) = raw.split_once('=') else {
            ui::error(&format!("--var expects key=value, got: {raw}"));
        };
        ctx_vars.push((k.to_string(), TemplateValue::Str(v.to_string())));
    }
    for raw in bools {
        let Some((k, v)) = raw.split_once('=') else {
            ui::error(&format!("--bool expects key=true|false, got: {raw}"));
        };
        let val = v.parse::<bool>().unwrap_or(false);
        ctx_vars.push((k.to_string(), TemplateValue::Bool(val)));
    }
    for raw in ints {
        let Some((k, v)) = raw.split_once('=') else {
            ui::error(&format!("--int expects key=N, got: {raw}"));
        };
        let val = v.parse::<i64>().unwrap_or(0);
        ctx_vars.push((k.to_string(), TemplateValue::Int(val)));
    }

    match mode {
        InspectMode::Ast => dump_ast(&source, &file),
        InspectMode::Render => dump_html(&source, &file, &ctx_vars),
        InspectMode::Ir => dump_ir(&source, &file, &ctx_vars),
        InspectMode::Ctx => dump_ctx(&file),
        InspectMode::Preview => {
            #[cfg(feature = "desktop")]
            {
                launch_viewer(file, width, height, ctx_vars);
            }
            #[cfg(not(feature = "desktop"))]
            {
                let _ = (file, width, height, ctx_vars);
                ui::error(
                    "GPUI preview not compiled in. Rebuild crepus with default features (full).",
                );
            }
        }
    }
}

fn dump_ast(source: &str, path: &Path) {
    match parse_template_with_path(source, Some(path)) {
        Ok(ast) => println!("{ast:#?}"),
        Err(e) => ui::error(&format!("Parse error: {e}")),
    }
}

fn dump_html(source: &str, path: &Path, vars: &[(String, TemplateValue)]) {
    let mut ctx = TemplateContext::new();
    for (k, v) in vars {
        ctx.set(k, v.clone());
    }
    match parse_template_with_path(source, Some(path)) {
        Ok(nodes) => match render_nodes_to_html(&nodes, &ctx) {
            Ok(html) => print!("{html}"),
            Err(e) => ui::error(&format!("Render error: {e}")),
        },
        Err(e) => ui::error(&format!("Render error: {e}")),
    }
}

fn dump_ir(source: &str, path: &Path, vars: &[(String, TemplateValue)]) {
    let mut ctx = TemplateContext::new();
    for (k, v) in vars {
        ctx.set(k, v.clone());
    }
    let component = path.file_stem().and_then(|s| s.to_str()).unwrap_or("root");
    match crepuscularity_native::render_component_file_to_ir(source, component, &ctx) {
        Ok(ir) => match crepuscularity_native::to_json_pretty(&ir) {
            Ok(json) => print!("{json}"),
            Err(e) => ui::error(&format!("IR serialization error: {e}")),
        },
        Err(e) => ui::error(&format!("IR error: {e}")),
    }
}

fn dump_ctx(template_path: &Path) {
    let dir = template_path.parent().unwrap_or(Path::new("."));
    let ctx_path = dir.join("context.toml");
    if !ctx_path.exists() {
        eprintln!("No context.toml found in {}", dir.display());
        eprintln!("Create one with key=value pairs:");
        eprintln!("  name = \"Alice\"");
        eprintln!("  count = 42");
        eprintln!("  show_header = true");
        return;
    }
    let content = std::fs::read_to_string(&ctx_path).unwrap_or_else(|e| {
        ui::error(&format!("Error reading {}: {e}", ctx_path.display()));
    });
    eprintln!("=== {} ===", ctx_path.display());
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let raw_val = line[eq + 1..].trim();
            let typed = if raw_val == "true" || raw_val == "false" {
                format!("bool: {raw_val}")
            } else if raw_val.parse::<i64>().is_ok() {
                format!("int: {raw_val}")
            } else if raw_val.parse::<f64>().is_ok() {
                format!("float: {raw_val}")
            } else {
                format!("string: {raw_val}")
            };
            eprintln!("  {key} = {typed}");
        }
    }
}

#[cfg(feature = "desktop")]
fn launch_viewer(
    template_path: PathBuf,
    width: f32,
    height: f32,
    vars: Vec<(String, TemplateValue)>,
) {
    use crepuscularity_runtime::{HotReloadState, HotReloadView};
    use gpui::{bounds, point, prelude::*, size, WindowOptions};

    let mut ctx = TemplateContext::new();
    for (k, v) in vars {
        ctx.set(k, v);
    }

    let display_name = template_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("template")
        .to_string();

    eprintln!("crepus inspect: watching {:?}", template_path);
    eprintln!("crepus inspect: window {}x{}", width as u32, height as u32);
    eprintln!("crepus inspect: edit and save to hot-reload");

    gpui_platform::application().run(move |cx: &mut gpui::App| {
        let window_options = WindowOptions {
            app_id: Some(format!("crepuscularity.inspect.{}", display_name)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(format!("Crepus Inspect - {}", display_name).into()),
                ..Default::default()
            }),
            window_bounds: Some(gpui::WindowBounds::Windowed(bounds(
                point(gpui::px(100.), gpui::px(100.)),
                size(gpui::px(width), gpui::px(height)),
            ))),
            ..Default::default()
        };

        let path = template_path.clone();
        let ctx = ctx.clone();

        if let Err(e) = cx.open_window(window_options, move |_window, cx| {
            let state = cx.new(|cx| HotReloadState::new(path.clone(), ctx.clone(), cx));
            cx.new(|_| HotReloadView::new(state))
        }) {
            eprintln!("Failed to open window: {e:?}");
        }
    });
}
