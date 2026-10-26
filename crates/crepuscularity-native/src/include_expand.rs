//! Expand `include` directives into parsed child nodes + context (no cycles with IR lowering).

use std::path::Path;
use std::sync::Arc;

use crepuscularity_core::ast::{IncludeNode, Node};
use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::parser::{parse_component_file, parse_template_with_path};
use crepuscularity_core::virtual_files::lookup_virtual_file;
use crepuscularity_core::CrepusError;

/// Maximum include nesting depth to prevent infinite recursion / stack overflow.
const MAX_INCLUDE_DEPTH: usize = 64;

pub(crate) fn read_file(ctx: &TemplateContext, path: &Path) -> Result<String, CrepusError> {
    if let Some(content) = lookup_virtual_file(ctx, path) {
        return Ok(content);
    }
    if cfg!(not(target_arch = "wasm32")) {
        std::fs::read_to_string(path)
            .map_err(|e| CrepusError::render(format!("include error: {:?}: {}", path, e)))
    } else {
        Err(CrepusError::render(format!(
            "include error: file not in virtual bundle: {}",
            path.to_string_lossy()
        )))
    }
}

/// Parsed nodes to render and the child context to use (base_dir, virtual_files, vars, slot).
pub(crate) fn expand_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
) -> Result<(Vec<Node>, TemplateContext), CrepusError> {
    expand_include_with_depth(inc, ctx, 0)
}

/// Depth-aware version of [`expand_include`]. Callers that recurse on the returned
/// nodes should pass `depth + 1` so that circular includes are detected before
/// stack overflow.
pub(crate) fn expand_include_with_depth(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    depth: usize,
) -> Result<(Vec<Node>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{}'",
            inc.path
        )));
    }

    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return expand_named_component(inc, ctx, file_part, comp_name);
    }

    let file_path = resolve_include_path(ctx.base_dir.as_deref(), &inc.path)?;
    let content = read_file(ctx, &file_path)?;
    let nodes = parse_template_with_path(&content, Some(&file_path))
        .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }
    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    Ok((nodes, child_ctx))
}

fn expand_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
) -> Result<(Vec<Node>, TemplateContext), CrepusError> {
    let file_path = resolve_include_path(ctx.base_dir.as_deref(), file_part)?;
    let content = read_file(ctx, &file_path)?;
    let comp_file = parse_component_file(&content)
        .map_err(|e| CrepusError::render(format!("component file parse error: {e}")))?;
    let comp = comp_file.components.get(comp_name).ok_or_else(|| {
        CrepusError::render(format!("component '{comp_name}' not found in {file_part}"))
    })?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &comp.meta.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }
    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    Ok((comp.nodes.clone(), child_ctx))
}
