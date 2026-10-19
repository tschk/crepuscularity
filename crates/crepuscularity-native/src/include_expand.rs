//! Expand `include` directives into parsed child nodes + context (no cycles with IR lowering).

use std::cell::Cell;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use crepuscularity_core::ast::{IncludeNode, Node};
use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::include_paths::{include_frontend_path, resolve_include_path};
use crepuscularity_core::parser::{parse_component_file, parse_template_with_path};
use crepuscularity_core::virtual_files::lookup_virtual_file;
use crepuscularity_core::CrepusError;

/// Maximum include nesting depth to prevent infinite recursion / stack overflow.
const MAX_INCLUDE_DEPTH: usize = 64;

thread_local! {
    static INCLUDE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct IncludeGuard(PhantomData<Rc<()>>);

impl Drop for IncludeGuard {
    fn drop(&mut self) {
        INCLUDE_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

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

/// Parsed nodes, child context, and a guard to keep alive while rendering them.
pub(crate) fn expand_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
) -> Result<(Vec<Node>, TemplateContext, IncludeGuard), CrepusError> {
    let depth = INCLUDE_DEPTH.with(Cell::get);
    let (nodes, child_ctx) = expand_include_with_depth(inc, ctx, depth)?;
    INCLUDE_DEPTH.with(|current| current.set(depth + 1));
    Ok((nodes, child_ctx, IncludeGuard(PhantomData)))
}

/// Depth-aware version of [`expand_include`]. Callers that recurse on the returned
/// nodes keep the guard from `expand_include` alive so depth includes nested
/// elements and slot rendering, even when their template context changes.
fn expand_include_with_depth(
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
    let frontend_path = include_frontend_path(&file_path, &inc.path);
    let nodes = parse_template_with_path(&content, Some(&frontend_path))
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
