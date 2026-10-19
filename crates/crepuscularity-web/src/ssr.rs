//! Server-side rendering with **hydration metadata**: stable `data-crepus-id` markers,
//! a JSON manifest (`#__crepus_hydration__`, base64-wrapped for safe embedding), and
//! optional full HTML document shells.
//!
//! Enable the **`ssr`** crate feature (on by default). For minimal WASM builds use
//! `default-features = false` on the `crepuscularity-web` dependency.
//!
//! # Manifest (`#__crepus_hydration__`)
//!
//! The script tag holds base64 (standard alphabet, no wraps) of UTF-8 JSON:
//! ```json
//! { "v": 1, "ctx": { … }, "bind": { "0": { "kind": "text", "parts": … }, … } }
//! ```
//!
//! - `ctx`: scalar template variables plus simple list/object shapes (see [`serialize_ctx_for_ssr`]).
//! - `bind`: maps numeric id strings (`"0"`, `"1"`, …) to binding descriptors. Matches `data-crepus-id="c0"` → `"0"`.

use std::cell::Cell;
use std::collections::HashMap;

use crate::render_nodes_to_html;

use ammonia::Builder;
use base64::Engine;
use serde_json::Value;

pub use crate::render::BindMap;

/// Maximum include nesting depth to prevent infinite recursion / stack overflow.
/// The marker-emitting render path costs ~36 KB of stack per include level in a
/// debug build (measured), so the budget is set to stay well inside the 1 MiB
/// stack a `wasm32` instance gets by default.
const MAX_INCLUDE_DEPTH: usize = 16;

thread_local! {
    static INCLUDE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// RAII depth counter: the SSR renderer recurses through `pub` entry points, so the
/// budget is tracked out of band rather than threaded through their signatures.
pub(crate) struct IncludeDepthGuard;

impl IncludeDepthGuard {
    pub(crate) fn enter(path: &str) -> Result<Self, CrepusError> {
        INCLUDE_DEPTH.with(|depth| {
            if depth.get() >= MAX_INCLUDE_DEPTH {
                return Err(CrepusError::render(format!(
                    "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{path}'"
                )));
            }
            depth.set(depth.get() + 1);
            Ok(Self)
        })
    }
}

impl Drop for IncludeDepthGuard {
    fn drop(&mut self) {
        INCLUDE_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::parser::parse_component_file;
use crepuscularity_core::CrepusError;

/// Options for [`render_ssr_document`].
#[derive(Debug, Clone)]
pub struct SsrDocument<'a> {
    pub lang: &'a str,
    pub title: &'a str,
    /// Raw HTML inserted before `</head>` (link tags, meta, inline styles).
    /// Sanitized with [`ammonia::clean`] before injection to strip scripts and unsafe markup.
    pub head_extra: &'a str,
    pub body_class: Option<&'a str>,
}

impl Default for SsrDocument<'static> {
    fn default() -> Self {
        Self {
            lang: "en",
            title: "",
            head_extra: "",
            body_class: None,
        }
    }
}

/// Render like [`crate::render_template_to_html`]. With `markers: true`, emit hydration markers + manifest.
pub fn render_template_to_html_with_ssr(
    template: &str,
    ctx: &TemplateContext,
    markers: bool,
) -> Result<String, CrepusError> {
    if !markers {
        return crate::render_template_to_html(template, ctx);
    }
    let nodes = crepuscularity_core::ast_cache::parse_content(template)?;
    let counter = Cell::new(0u32);
    let mut bind = BindMap::new();
    let mut html = render_nodes_ssr(&nodes, ctx, &counter, &mut bind, true)?;
    append_hydration_payload(&mut html, ctx, &bind)?;
    Ok(html)
}

/// Like [`crate::render_from_files`] with optional SSR markers on the entry template.
pub fn render_from_files_with_ssr(
    files: &HashMap<String, String>,
    entry: &str,
    ctx: &TemplateContext,
    markers: bool,
) -> Result<String, CrepusError> {
    let mut ctx = ctx.clone();
    ctx.virtual_files = std::sync::Arc::new(files.clone());

    if let Some((file_part, comp_name)) = entry.split_once('#') {
        let content = files.get(file_part).ok_or_else(|| {
            CrepusError::render(format!("file not found in virtual fs: {file_part}"))
        })?;
        return render_component_file_to_html_with_ssr(content, comp_name, &ctx, markers);
    }

    let content = files
        .get(entry)
        .ok_or_else(|| CrepusError::render(format!("file not found in virtual fs: {entry}")))?;
    render_template_to_html_with_ssr(content, &ctx, markers)
}

fn render_component_file_to_html_with_ssr(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
    markers: bool,
) -> Result<String, CrepusError> {
    if !markers {
        return crate::render_component_file_to_html(content, component_name, ctx);
    }

    let file = parse_component_file(content)?;
    let component = file
        .components
        .get(component_name)
        .ok_or_else(|| CrepusError::render(format!("component not found: {component_name}")))?;

    let mut child_ctx = ctx.clone();
    for (key, expr) in &component.meta.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }

    let counter = Cell::new(0u32);
    let mut bind = BindMap::new();
    let mut html = render_nodes_ssr(&component.nodes, &child_ctx, &counter, &mut bind, true)?;
    append_hydration_payload(&mut html, &child_ctx, &bind)?;
    Ok(html)
}

/// Parse `crepus-bundle.json` and render with optional SSR markers.
pub fn render_bundle_with_ssr(bundle_json: &str, markers: bool) -> Result<String, CrepusError> {
    let root: Value = serde_json::from_str(bundle_json)
        .map_err(|e| CrepusError::render(format!("bundle JSON: {e}")))?;

    let mut root_map = match root {
        Value::Object(map) => map,
        _ => return Err(CrepusError::render("bundle must be a JSON object")),
    };

    let entry = match root_map.remove("entry") {
        Some(Value::String(s)) => s,
        _ => return Err(CrepusError::render("bundle missing string field \"entry\"")),
    };

    let files_val = root_map
        .remove("files")
        .ok_or_else(|| CrepusError::render("bundle missing \"files\" object"))?;

    let files_obj = match files_val {
        Value::Object(map) => map,
        _ => return Err(CrepusError::render("\"files\" must be a JSON object")),
    };

    let mut files = HashMap::with_capacity(files_obj.len());
    for (k, v) in files_obj {
        let s = match v {
            Value::String(s) => s,
            _ => {
                return Err(CrepusError::render(format!(
                    "files[{k:?}] must be a string"
                )))
            }
        };
        files.insert(k, s);
    }

    let ctx = TemplateContext::new();
    render_from_files_with_ssr(&files, &entry, &ctx, markers)
}

/// Helper: wrap rendered inner HTML in an HTML5 document shell.
pub fn wrap_ssr_document(inner: &str, doc: &SsrDocument<'_>) -> String {
    let body_class = doc
        .body_class
        .map(|c| format!(r#" class="{}""#, crate::escape_html(c)))
        .unwrap_or_default();
    let title_esc = crate::escape_html(doc.title);
    let head_safe = Builder::new()
        .rm_tags(&["base", "meta", "link", "style"])
        .clean(doc.head_extra);
    format!(
        r#"<!DOCTYPE html>
<html lang="{}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  {}
</head>
<body{}>
{}
</body>
</html>
"#,
        crate::escape_html(doc.lang),
        title_esc,
        head_safe,
        body_class,
        inner
    )
}

/// Full HTML5 document: runs the template through [`render_template_to_html_with_ssr`], then wraps result.
pub fn render_ssr_document(
    template: &str,
    ctx: &TemplateContext,
    doc: &SsrDocument<'_>,
    markers: bool,
) -> Result<String, CrepusError> {
    let inner = render_template_to_html_with_ssr(template, ctx, markers)?;
    Ok(wrap_ssr_document(&inner, doc))
}

/// Like [`render_ssr_document`] but takes pre-parsed AST nodes instead of a template string.
/// Use this when the caller has already parsed and cached the AST (e.g. [`SsrOptions`](crate::SsrOptions)).
/// The `counter` and `bind` are reused across calls for hydration consistency.
pub fn render_ssr_document_with_nodes(
    nodes: &[Node],
    counter: &Cell<u32>,
    bind: &mut BindMap,
    ctx: &TemplateContext,
    doc: &SsrDocument<'_>,
    markers: bool,
) -> Result<String, CrepusError> {
    let inner = if markers {
        let mut html = render_nodes_ssr(nodes, ctx, counter, bind, true)?;
        append_hydration_payload(&mut html, ctx, bind)?;
        html
    } else {
        render_nodes_to_html(nodes, ctx)?
    };
    Ok(wrap_ssr_document(&inner, doc))
}

pub(crate) fn append_hydration_payload(
    html: &mut String,
    ctx: &TemplateContext,
    bind: &BindMap,
) -> Result<(), CrepusError> {
    let ctx_val = serialize_ctx_for_ssr(ctx)?;
    let raw = crate::hydration_payload_bytes(ctx_val, Value::Object(bind.clone()))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
    let script = format!(
        r#"<script type="application/json" id="__crepus_hydration__" data-crepus-encoding="base64">{b64}</script>"#
    );
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &script);
    } else {
        html.push_str(&script);
    }
    Ok(())
}

/// Serialize [`TemplateContext::vars`] for the hydration manifest (scalars, lists of flat objects, nested maps of scalars).
pub fn serialize_ctx_for_ssr(ctx: &TemplateContext) -> Result<Value, CrepusError> {
    let mut m = BindMap::new();
    for (k, v) in &ctx.vars {
        m.insert(k.clone(), template_value_to_json(v)?);
    }
    Ok(Value::Object(m))
}

fn template_value_to_json(v: &TemplateValue) -> Result<Value, CrepusError> {
    Ok(match v {
        TemplateValue::Str(s) => Value::String(s.clone()),
        TemplateValue::Int(n) => Value::Number((*n).into()),
        TemplateValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        TemplateValue::Bool(b) => Value::Bool(*b),
        TemplateValue::Null => Value::Null,
        TemplateValue::List(items) => {
            let mut arr = Vec::with_capacity(items.len());
            for item in items {
                arr.push(flat_context_object(item)?);
            }
            Value::Array(arr)
        }
        TemplateValue::Scope(ctx) => flat_context_object(ctx)?,
    })
}

fn flat_context_object(ctx: &TemplateContext) -> Result<Value, CrepusError> {
    let mut m = BindMap::new();
    for (k, v) in &ctx.vars {
        m.insert(k.clone(), template_value_to_json(v)?);
    }
    Ok(Value::Object(m))
}

/// Walk `nodes` emitting hydration markers, allocating `data-crepus-id` bindings into `bind`.
///
/// `root_element_pending` marks the first dynamic element as the hydration root.
pub fn render_nodes_ssr(
    nodes: &[Node],
    ctx: &TemplateContext,
    counter: &Cell<u32>,
    bind: &mut BindMap,
    root_element_pending: bool,
) -> Result<String, CrepusError> {
    crate::render::Walker::ssr(counter, bind, root_element_pending).nodes(nodes, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_core::context::TemplateContext;
    use crepuscularity_core::context::TemplateValue;

    #[test]
    fn test_serialize_ctx_for_ssr_empty() {
        let ctx = TemplateContext::new();
        let result = serialize_ctx_for_ssr(&ctx).unwrap();
        assert_eq!(result, serde_json::Value::Object(serde_json::Map::new()));
    }

    #[test]
    fn test_serialize_ctx_for_ssr_with_vars() {
        let mut ctx = TemplateContext::new();
        ctx.vars
            .insert("str".to_string(), TemplateValue::Str("hello".to_string()));
        ctx.vars
            .insert("num".to_string(), TemplateValue::Float(42.0));
        ctx.vars
            .insert("bool".to_string(), TemplateValue::Bool(true));

        let result = serialize_ctx_for_ssr(&ctx).unwrap();
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("str").unwrap().as_str().unwrap(), "hello");
        assert_eq!(obj.get("num").unwrap().as_f64().unwrap(), 42.0);
        assert!(obj.get("bool").unwrap().as_bool().unwrap());
    }
}
