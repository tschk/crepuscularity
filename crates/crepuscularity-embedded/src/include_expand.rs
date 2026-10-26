//! Expand `include` directives into parsed child nodes + context.

use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crepuscularity_core::ast::{IncludeNode, Node};
use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::parser::parse_component_file;
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

#[derive(Default)]
pub(crate) struct IncludeCache {
    bundles: Vec<BundleCache>,
}

struct BundleCache {
    virtual_files: Arc<HashMap<String, String>>,
    files: HashMap<PathBuf, CachedFile>,
}

struct CachedFile {
    source: String,
    nodes: HashMap<PathBuf, Arc<Vec<Node>>>,
    components: Option<HashMap<String, CachedComponent>>,
}

struct CachedComponent {
    nodes: Arc<Vec<Node>>,
    defaults: HashMap<String, String>,
}

impl IncludeCache {
    fn file(&mut self, ctx: &TemplateContext, path: &Path) -> Result<&mut CachedFile, CrepusError> {
        let index = self
            .bundles
            .iter()
            .position(|bundle| Arc::ptr_eq(&bundle.virtual_files, &ctx.virtual_files))
            .unwrap_or_else(|| {
                self.bundles.push(BundleCache {
                    virtual_files: Arc::clone(&ctx.virtual_files),
                    files: HashMap::new(),
                });
                self.bundles.len() - 1
            });
        let files = &mut self.bundles[index].files;
        if !files.contains_key(path) {
            files.insert(
                path.to_path_buf(),
                CachedFile {
                    source: read_file(ctx, path)?,
                    nodes: HashMap::new(),
                    components: None,
                },
            );
        }
        Ok(files.get_mut(path).expect("included file cached"))
    }
}

pub(crate) fn read_file(ctx: &TemplateContext, path: &Path) -> Result<String, CrepusError> {
    if let Some(content) = lookup_virtual_file(ctx, path) {
        return Ok(content);
    }
    std::fs::read_to_string(path)
        .map_err(|e| CrepusError::render(format!("include error: {:?}: {}", path, e)))
}

fn inherit_target_context(child: &mut TemplateContext, parent: &TemplateContext) {
    // Target and display values belong to the rendering environment, not component props.
    for key in [
        "crepus_target",
        "is_embedded",
        "is_tui",
        "is_web",
        "is_gui",
        "screen_width",
        "screen_height",
    ] {
        if let Some(value) = parent.get(key) {
            child.vars.insert(key.into(), value.clone());
        }
    }
}

pub(crate) fn expand_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    cache: &mut IncludeCache,
) -> Result<(Arc<Vec<Node>>, TemplateContext, IncludeGuard), CrepusError> {
    let depth = INCLUDE_DEPTH.with(Cell::get);
    let (nodes, child_ctx) = expand_include_with_depth(inc, ctx, depth, cache)?;
    INCLUDE_DEPTH.with(|current| current.set(depth + 1));
    Ok((nodes, child_ctx, IncludeGuard(PhantomData)))
}

/// Depth-aware version of [`expand_include`]. Callers that recurse on the returned
/// nodes should pass `depth + 1` so that circular includes are detected before
/// stack overflow.
pub(crate) fn expand_include_with_depth(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    depth: usize,
    cache: &mut IncludeCache,
) -> Result<(Arc<Vec<Node>>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{}'",
            inc.path
        )));
    }

    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return expand_named_component(inc, ctx, file_part, comp_name, cache);
    }

    let file_path = resolve_include_path(ctx.base_dir.as_deref(), &inc.path)?;
    let file = cache.file(ctx, &file_path)?;
    let frontend_path =
        crepuscularity_core::include_paths::include_frontend_path(&file_path, &inc.path);
    let nodes = if let Some(nodes) = file.nodes.get(&frontend_path) {
        Arc::clone(nodes)
    } else {
        let nodes = Arc::new(
            crepuscularity_core::parse_template_with_path(&file.source, Some(&frontend_path))
                .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?,
        );
        file.nodes.insert(frontend_path, Arc::clone(&nodes));
        nodes
    };

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }

    inherit_target_context(&mut child_ctx, ctx);
    Ok((nodes, child_ctx))
}

fn expand_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
    cache: &mut IncludeCache,
) -> Result<(Arc<Vec<Node>>, TemplateContext), CrepusError> {
    let file_path = resolve_include_path(ctx.base_dir.as_deref(), file_part)?;
    let file = cache.file(ctx, &file_path)?;
    if file.components.is_none() {
        let parsed = parse_component_file(&file.source)
            .map_err(|e| CrepusError::render(format!("component file parse error: {e}")))?;
        file.components = Some(
            parsed
                .components
                .into_iter()
                .map(|(name, component)| {
                    (
                        name,
                        CachedComponent {
                            nodes: Arc::new(component.nodes),
                            defaults: component.meta.defaults,
                        },
                    )
                })
                .collect(),
        );
    }
    let comp = file
        .components
        .as_ref()
        .expect("included components cached")
        .get(comp_name)
        .ok_or_else(|| {
            CrepusError::render(format!("component '{comp_name}' not found in {file_part}"))
        })?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &comp.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }

    inherit_target_context(&mut child_ctx, ctx);
    Ok((Arc::clone(&comp.nodes), child_ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn include(source: &str) -> IncludeNode {
        match crepuscularity_core::parse_template(source)
            .unwrap()
            .remove(0)
        {
            Node::Include(include) => include,
            _ => panic!("expected include"),
        }
    }

    #[test]
    fn repeated_includes_share_nodes_but_evaluate_each_prop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("child.crepus");
        std::fs::write(&path, "span \"{title}\"").unwrap();
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(dir.path().to_path_buf());
        let mut cache = IncludeCache::default();
        let first = include("include child.crepus title=\"First\"");
        let second = include("include child.crepus title=\"Second\"");
        let (one, one_ctx) = expand_include_with_depth(&first, &ctx, 0, &mut cache).unwrap();
        std::fs::remove_file(&path).unwrap();
        let (two, two_ctx) = expand_include_with_depth(&second, &ctx, 0, &mut cache).unwrap();
        assert!(Arc::ptr_eq(&one, &two));
        assert_eq!(one_ctx.get_str("title"), "First");
        assert_eq!(two_ctx.get_str("title"), "Second");
        assert!(expand_include_with_depth(&second, &ctx, 64, &mut cache).is_err());
        assert!(expand_include_with_depth(&second, &ctx, 0, &mut IncludeCache::default()).is_err());
    }

    #[test]
    fn named_components_share_nodes_but_keep_distinct_props() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files)
            .insert("ui.crepus".into(), "--- Card\nspan \"{title}\"".into());
        let mut cache = IncludeCache::default();
        let first = include("include ui.crepus#Card title=\"First\"");
        let second = include("include ui.crepus#Card title=\"Second\"");
        let (one, one_ctx) = expand_include_with_depth(&first, &ctx, 0, &mut cache).unwrap();
        let (two, two_ctx) = expand_include_with_depth(&second, &ctx, 0, &mut cache).unwrap();
        assert!(Arc::ptr_eq(&one, &two));
        assert_eq!(one_ctx.get_str("title"), "First");
        assert_eq!(two_ctx.get_str("title"), "Second");
        assert!(expand_include_with_depth(&second, &ctx, 64, &mut cache).is_err());
    }
}
