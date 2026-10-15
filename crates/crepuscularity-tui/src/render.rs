//! AST + context → ratatui `Frame` rendering.
//!
//! The renderer works in two phases:
//!
//! 1. **Build** — walks the `.crepus` AST (evaluating control-flow and expressions)
//!    and produces a [`WidgetNode`] tree that mirrors the DOM hierarchy.
//!
//! 2. **Paint** — recursively splits the terminal `Rect` using ratatui `Layout` and
//!    renders each [`WidgetNode`] into the resulting chunks.
//!
//! The split mirrors CSS flexbox: `flex-col` (default) stacks children vertically;
//! `flex-row` stacks them horizontally.  Size classes (`w-[20]`, `h-[3]`, `flex-1`,
//! `w-1/3`, …) become ratatui `Constraint`s so the layout is fully driven by the
//! template, not imperative Rust code.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget, Wrap};
use ratatui::Frame;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::parser::{parse_component_file, parse_template, ComponentDef};
use crepuscularity_core::preprocess::slot_rotate_child_phrases;
use crepuscularity_core::virtual_files::lookup_virtual_file;
use crepuscularity_core::CrepusError;

use crate::style::{parse_classes, SizeHint, StyleHints};

fn eval_value(expr: &str, ctx: &TemplateContext) -> TemplateValue {
    eval_expr(expr, ctx).unwrap_or(TemplateValue::Null)
}

fn eval_condition(ctx: &TemplateContext, expr: &str) -> bool {
    ctx.eval_condition(expr).unwrap_or(false)
}

fn interpolate_str(ctx: &TemplateContext, s: &str) -> String {
    ctx.interpolate(s).unwrap_or_else(|_| s.to_string())
}

fn read_file(ctx: &TemplateContext, path: &std::path::Path) -> Result<String, CrepusError> {
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

// ─── Intermediate widget tree ─────────────────────────────────────────────────

/// An intermediate representation of the layout, independent of screen size.
/// Built from the AST, then painted into a ratatui `Frame`.
pub enum WidgetNode {
    /// A flex container that splits its area into sub-areas for its children.
    Container {
        direction: Direction,
        gap: u16,
        children: Vec<WidgetChild>,
        style: Style,
        block: Option<BlockSpec>,
        scroll_offset: usize,
        scroll_bottom: bool,
        scrollable: bool,
    },
    /// A leaf node rendered as a ratatui `Paragraph`.
    Content {
        lines: Vec<Line<'static>>,
        style: Style,
        block: Option<BlockSpec>,
        alignment: Alignment,
    },
    /// Occupies no space (collapsed if-branch, empty for-loop, etc.).
    Empty,
}

pub struct WidgetChild {
    pub node: WidgetNode,
    /// How much space this child requests from its parent in the layout axis.
    pub constraint: Constraint,
}

/// Parameters needed to build a `Block` widget (borders + optional title + padding).
#[derive(Clone)]
pub struct BlockSpec {
    borders: Borders,
    border_type: BorderType,
    border_style: Style,
    title: Option<String>,
    title_alignment: Alignment,
    padding: Padding,
}

impl BlockSpec {
    fn to_block(&self) -> Block<'static> {
        let mut b = Block::default()
            .borders(self.borders)
            .border_type(self.border_type)
            .border_style(self.border_style)
            .padding(self.padding);
        if let Some(ref t) = self.title {
            b = b.title(t.clone()).title_alignment(self.title_alignment);
        }
        b
    }
}

// ─── Public render entry points ───────────────────────────────────────────────

/// Parse `template` and paint it into `frame` within `area`.
///
/// `area` is typically `frame.area()` for a full-screen root element, or a
/// sub-`Rect` when embedding a crepus view inside a larger ratatui application.
pub fn render_template(
    template: &str,
    ctx: &TemplateContext,
    frame: &mut Frame,
    area: Rect,
) -> Result<(), CrepusError> {
    // Frame-based entry point — parse once, delegate to the shared Buffer
    // painting pipeline through a transient BufferRenderer.
    let nodes = parse_template(template)?;
    let mut renderer = BufferRenderer::new();
    renderer.render_nodes(&nodes, ctx, frame.buffer_mut(), area)
}

/// Walk a template source and collect the IDs of elements marked `focusable`,
/// in document (tab) order.
pub fn collect_focusable_ids(template: &str) -> Result<Vec<String>, CrepusError> {
    let nodes = parse_template(template)?;
    Ok(collect_focusable_ids_from_nodes(&nodes))
}

/// Walk pre-parsed `nodes` and collect focusable element IDs in document order.
pub fn collect_focusable_ids_from_nodes(nodes: &[Node]) -> Vec<String> {
    let mut ids = Vec::new();
    collect_focusable_ids_recursive(nodes, &mut ids);
    ids
}

/// Collect focusable IDs from the currently rendered branches and loop items.
pub(crate) fn collect_focusable_ids_with_context(
    template: &str,
    ctx: &TemplateContext,
) -> Result<Vec<String>, CrepusError> {
    let nodes = parse_template(template)?;
    let mut ids = Vec::new();
    collect_focusable_ids_in_context(&nodes, ctx, &mut ids);
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    Ok(ids)
}

fn collect_focusable_ids_in_context(nodes: &[Node], ctx: &TemplateContext, out: &mut Vec<String>) {
    let mut ctx = ctx.clone();
    for node in nodes {
        match node {
            Node::LetDecl(decl) => {
                if !decl.is_default || !ctx.vars.contains_key(&decl.name) {
                    let value = eval_expr(&decl.expr, &ctx).unwrap_or(TemplateValue::Null);
                    ctx.vars.insert(decl.name.clone(), value);
                }
            }
            Node::Element(el) => {
                let static_focusable = el
                    .classes
                    .iter()
                    .any(|class| class == "focusable" || class.contains("focusable"));
                let dynamic_focusable = el.conditional_classes.iter().any(|class| {
                    class.class == "focusable" && eval_condition(&ctx, &class.condition)
                });
                if static_focusable || dynamic_focusable {
                    if let Some(id) = &el.id {
                        out.push(id.clone());
                    }
                }
                collect_focusable_ids_in_context(&el.children, &ctx, out);
            }
            Node::If(block) => {
                let branch = if eval_condition(&ctx, &block.condition) {
                    Some(&block.then_children)
                } else {
                    block.else_children.as_ref()
                };
                if let Some(branch) = branch {
                    collect_focusable_ids_in_context(branch, &ctx, out);
                }
            }
            Node::For(block) => {
                for item_ctx in ctx.get_list_ref(&block.iterator) {
                    let mut child_ctx = ctx.clone();
                    child_ctx.vars.extend(item_ctx.vars.clone());
                    if !block.pattern.trim().is_empty() {
                        let item_str = item_ctx.get_str("value");
                        if !item_str.is_empty() {
                            child_ctx.vars.insert(
                                block.pattern.trim().to_string(),
                                TemplateValue::Str(item_str),
                            );
                        }
                    }
                    collect_focusable_ids_in_context(&block.body, &child_ctx, out);
                }
            }
            Node::Match(block) => {
                let value =
                    value_to_str(&eval_expr(&block.expr, &ctx).unwrap_or(TemplateValue::Null));
                let arm = block.arms.iter().find(|arm| {
                    let pattern = arm.pattern.trim();
                    let quoted = pattern
                        .strip_prefix('"')
                        .and_then(|value| value.strip_suffix('"'))
                        == Some(value.as_str());
                    pattern == "_" || quoted || pattern == value
                });
                if let Some(arm) = arm {
                    collect_focusable_ids_in_context(&arm.body, &ctx, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_focusable_ids_recursive(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::Element(el) => {
                if el.id.is_some()
                    && el
                        .classes
                        .iter()
                        .any(|c| c == "focusable" || c.contains("focusable"))
                {
                    if let Some(ref id) = el.id {
                        out.push(id.clone());
                    }
                }
                collect_focusable_ids_recursive(&el.children, out);
            }
            Node::If(block) => {
                collect_focusable_ids_recursive(&block.then_children, out);
                if let Some(ref else_children) = block.else_children {
                    collect_focusable_ids_recursive(else_children, out);
                }
            }
            Node::For(block) => {
                collect_focusable_ids_recursive(&block.body, out);
            }
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_focusable_ids_recursive(&arm.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Paint pre-parsed `nodes` into `frame` within `area`.
pub fn render_nodes(
    nodes: &[Node],
    ctx: &TemplateContext,
    frame: &mut Frame,
    area: Rect,
) -> Result<(), CrepusError> {
    // Frame-based entry point — delegate to the same Buffer painting pipeline
    // used by `BufferRenderer::render_nodes` through a transient renderer.
    let mut renderer = BufferRenderer::new();
    renderer.render_nodes(nodes, ctx, frame.buffer_mut(), area)
}

pub fn render_component(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
    frame: &mut Frame,
    area: Rect,
) -> Result<(), CrepusError> {
    // Frame-based entry point — parse once, delegate to the shared Buffer
    // painting pipeline through a transient BufferRenderer.
    let mut file = parse_component_file(content)?;
    let component = file
        .components
        .remove(component_name)
        .ok_or_else(|| CrepusError::render(format!("component not found: {component_name}")))?;
    let mut renderer = BufferRenderer::new();
    renderer.render_component(&component, ctx, frame.buffer_mut(), area)
}

fn with_tui_target(ctx: &TemplateContext) -> TemplateContext {
    let mut ctx = ctx.clone();
    if !ctx.vars.contains_key("crepus_target") {
        ctx.vars.insert(
            "crepus_target".to_string(),
            TemplateValue::Str("tui".to_string()),
        );
    }
    if !ctx.vars.contains_key("is_tui") {
        ctx.vars
            .insert("is_tui".to_string(), TemplateValue::Bool(true));
    }
    if !ctx.vars.contains_key("is_gui") {
        ctx.vars
            .insert("is_gui".to_string(), TemplateValue::Bool(false));
    }
    if !ctx.vars.contains_key("is_web") {
        ctx.vars
            .insert("is_web".to_string(), TemplateValue::Bool(false));
    }
    if !ctx.vars.contains_key("crepus_platform") {
        ctx.vars.insert(
            "crepus_platform".to_string(),
            TemplateValue::Str(std::env::consts::OS.to_string()),
        );
    }
    if !ctx.vars.contains_key("is_macos") {
        ctx.vars.insert(
            "is_macos".to_string(),
            TemplateValue::Bool(cfg!(target_os = "macos")),
        );
    }
    if !ctx.vars.contains_key("is_windows") {
        ctx.vars.insert(
            "is_windows".to_string(),
            TemplateValue::Bool(cfg!(target_os = "windows")),
        );
    }
    if !ctx.vars.contains_key("is_linux") {
        ctx.vars.insert(
            "is_linux".to_string(),
            TemplateValue::Bool(cfg!(target_os = "linux")),
        );
    }
    ctx
}

// ─── Public Buffer rendering API ──────────────────────────────────────────────
//
// The Frame-based entry points above are source-compatible and continue to be
// the primary API for terminal applications driving a `ratatui::Frame`.
//
// The Buffer-based API below is the additive public surface for callers that
// own a `ratatui::buffer::Buffer` directly (custom drawing pipelines, embedded
// use inside larger terminal apps, and unit tests that want to assert on cell
// bytes without going through a Frame). The two entry points share the same
// internal build/paint implementations; the Frame API delegates here.

/// Statistics reported by a [`BufferRenderer`] about include-cache behaviour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderCacheStats {
    /// Number of times a cached include was reused without re-parsing.
    pub hits: u64,
    /// Number of times an include was parsed for the first time and cached.
    pub misses: u64,
    /// Number of times a cached include's source changed and was re-parsed.
    pub invalidations: u64,
}

/// Cache key for an included template.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IncludeKey {
    path: PathBuf,
    component: Option<String>,
}

/// A successful cached include parse.
#[derive(Debug)]
struct CachedInclude {
    source: Arc<str>,
    nodes: Arc<[Node]>,
}

/// Public renderer that exposes the same Buffer-based painting pipeline as the
/// Frame-based [`render_template`] / [`render_nodes`] / [`render_component`]
/// API, but accepts and mutates a caller-owned `ratatui::buffer::Buffer`
/// directly. Includes are cached by resolved path plus optional component
/// name so transitive includes share one cache across renders.
#[derive(Debug, Default)]
pub struct BufferRenderer {
    include_cache: HashMap<IncludeKey, CachedInclude>,
    stats: RenderCacheStats,
}

impl BufferRenderer {
    /// Create a new renderer with an empty include cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint `nodes` into `buf` within `area` using the shared Buffer pipeline.
    pub fn render_nodes(
        &mut self,
        nodes: &[Node],
        ctx: &TemplateContext,
        buf: &mut Buffer,
        area: Rect,
    ) -> Result<(), CrepusError> {
        let render_ctx = with_tui_target(ctx);
        let children = build_children(
            nodes,
            &render_ctx,
            Direction::Vertical,
            Style::default(),
            self,
        );
        paint_children_buf(&children, buf, area, Direction::Vertical, 0);
        Ok(())
    }

    /// Paint a pre-parsed [`ComponentDef`] into `buf` within `area`. The
    /// caller supplies the parsed `ComponentDef` so the multi-component file's
    /// frontmatter and siblings need not be re-parsed for every render.
    pub fn render_component(
        &mut self,
        component: &ComponentDef,
        ctx: &TemplateContext,
        buf: &mut Buffer,
        area: Rect,
    ) -> Result<(), CrepusError> {
        let mut child_ctx = with_tui_target(ctx);
        for (key, expr) in &component.meta.defaults {
            child_ctx
                .vars
                .entry(key.clone())
                .or_insert_with(|| eval_value(expr, &TemplateContext::default()));
        }
        self.render_nodes(&component.nodes, &child_ctx, buf, area)
    }

    /// Drop every cached include. Cache statistics are reset to zero.
    pub fn clear_cache(&mut self) {
        self.include_cache.clear();
        self.stats = RenderCacheStats::default();
    }

    /// Snapshot the renderer's cache statistics.
    pub fn cache_stats(&self) -> RenderCacheStats {
        self.stats
    }

    /// Resolve the parsed nodes for an include, consulting and updating the
    /// include cache. The returned `Arc<[Node]>` is owned by the cache and is
    /// safe to keep across subsequent renders because new nodes replace the
    /// cache entry wholesale.
    fn load_include(
        &mut self,
        full_path: &Path,
        component_name: Option<&str>,
        content: &str,
    ) -> Result<Arc<[Node]>, Vec<WidgetChild>> {
        let key = IncludeKey {
            path: full_path.to_path_buf(),
            component: component_name.map(str::to_owned),
        };
        if let Some(entry) = self.include_cache.get(&key) {
            if entry.source.as_ref() == content {
                // Exact source equality: cache hit.
                self.stats.hits += 1;
                return Ok(entry.nodes.clone());
            }
            // Source changed: re-parse and bump `invalidations`. We deliberately
            // do NOT count this as a miss — the include was already cached.
            match parse_included_source(component_name, content, &full_path.to_string_lossy()) {
                Ok(nodes) => {
                    self.stats.invalidations += 1;
                    let arc_nodes: Arc<[Node]> = Arc::from(nodes.into_boxed_slice());
                    self.include_cache.insert(
                        key,
                        CachedInclude {
                            source: Arc::from(content),
                            nodes: arc_nodes.clone(),
                        },
                    );
                    Ok(arc_nodes)
                }
                Err(error_nodes) => {
                    self.include_cache.remove(&key);
                    Err(error_nodes)
                }
            }
        } else {
            // First successful parse: cache miss. Failures are NOT cached.
            match parse_included_source(component_name, content, &full_path.to_string_lossy()) {
                Ok(nodes) => {
                    self.stats.misses += 1;
                    let arc_nodes: Arc<[Node]> = Arc::from(nodes.into_boxed_slice());
                    self.include_cache.insert(
                        key,
                        CachedInclude {
                            source: Arc::from(content),
                            nodes: arc_nodes.clone(),
                        },
                    );
                    Ok(arc_nodes)
                }
                Err(error_nodes) => Err(error_nodes),
            }
        }
    }
}

#[cfg(test)]
static PARSE_INCLUSION_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub fn reset_parse_counter() {
    PARSE_INCLUSION_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
pub fn parse_inclusion_calls() -> u64 {
    PARSE_INCLUSION_CALLS.load(std::sync::atomic::Ordering::SeqCst)
}
fn parse_included_source(
    component_name: Option<&str>,
    content: &str,
    file_path: &str,
) -> Result<Vec<Node>, Vec<WidgetChild>> {
    match component_name {
        Some(name) => {
            #[cfg(test)]
            {
                PARSE_INCLUSION_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            match parse_component_file(content) {
                Ok(file) => file
                    .components
                    .get(name)
                    .map(|c| {
                        let mut nodes = c.nodes.clone();
                        for (k, v) in &c.meta.defaults {
                            nodes.insert(
                                0,
                                Node::LetDecl(LetDecl {
                                    name: k.clone(),
                                    expr: v.clone(),
                                    is_default: true,
                                }),
                            );
                        }
                        nodes
                    })
                    .ok_or_else(|| error_children(&format!("component not found: {name}"))),
                Err(e) => Err(error_children(&format!(
                    "parse error in '{file_path}': {}",
                    e
                ))),
            }
        }
        None => match parse_template(content) {
            Ok(n) => Ok(n),
            Err(e) => Err(error_children(&format!(
                "parse error in '{file_path}': {}",
                e
            ))),
        },
    }
}

/// Parse `template` and paint it into `buf` within `area`. Equivalent to
/// [`render_template`] but operates on a raw `Buffer` and uses a fresh
/// short-lived [`BufferRenderer`] (no cross-render cache reuse).
pub fn render_template_to_buffer(
    template: &str,
    ctx: &TemplateContext,
    buf: &mut Buffer,
    area: Rect,
) -> Result<(), CrepusError> {
    let nodes = parse_template(template)?;
    let mut renderer = BufferRenderer::new();
    renderer.render_nodes(&nodes, ctx, buf, area)
}

/// Parse a named component from a multi-component `.crepus` file and paint it
/// into `buf` within `area`. Equivalent to [`render_component`] but operates
/// on a raw `Buffer` and uses a fresh short-lived [`BufferRenderer`].
pub fn render_component_to_buffer(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
    buf: &mut Buffer,
    area: Rect,
) -> Result<(), CrepusError> {
    let mut file = parse_component_file(content)?;
    let component = file
        .components
        .remove(component_name)
        .ok_or_else(|| CrepusError::render(format!("component not found: {component_name}")))?;
    let mut renderer = BufferRenderer::new();
    renderer.render_component(&component, ctx, buf, area)
}
// ─── Phase 1: Build ───────────────────────────────────────────────────────────

/// Convert a slice of nodes into a list of `WidgetChild`s.
///
/// `parent_dir` controls which `SizeHint` dimension (width vs. height) is used
/// to derive each child's `Constraint`.
fn build_children(
    nodes: &[Node],
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> Vec<WidgetChild> {
    // Borrowed until a `let` declaration actually mutates the scope: cloning a
    // context whose vars hold the loop's item list is O(list length), so an
    // unconditional clone per node list makes nested loops quadratic.
    let mut ctx: std::borrow::Cow<'_, TemplateContext> = std::borrow::Cow::Borrowed(ctx);
    let mut out: Vec<WidgetChild> = Vec::new();
    // Consecutive text/expression nodes are buffered and emitted as one Content.
    let mut text_buf: Vec<Line<'static>> = Vec::new();

    for node in nodes {
        match node {
            // Variable declarations mutate context for subsequent siblings.
            Node::LetDecl(decl) => {
                let val = eval_value(&decl.expr, &ctx);
                if decl.is_default {
                    if !ctx.vars.contains_key(&decl.name) {
                        ctx.to_mut().vars.insert(decl.name.clone(), val);
                    }
                } else {
                    let ctx = ctx.to_mut();
                    if let Some(v) = ctx.vars.get_mut(&decl.name) {
                        *v = val;
                    } else {
                        ctx.vars.insert(decl.name.clone(), val);
                    }
                }
            }
            Node::Text(parts) => {
                text_buf.push(build_line(parts, &ctx));
            }
            Node::RawText(expr) => {
                text_buf.push(Line::raw(value_to_str(&eval_value(expr, &ctx))));
            }
            Node::RawHtml(expr) => {
                text_buf.push(Line::raw(value_to_str(&eval_value(expr, &ctx))));
            }
            // Structural nodes: flush buffered text first, then emit.
            Node::Element(el) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
                out.push(build_element(el, &ctx, parent_dir, inherited, renderer));
            }
            Node::If(block) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
                out.extend(build_if(block, &ctx, parent_dir, inherited, renderer));
            }
            Node::For(block) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
                out.extend(build_for(block, &ctx, parent_dir, inherited, renderer));
            }
            Node::Match(block) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
                out.extend(build_match(block, &ctx, parent_dir, inherited, renderer));
            }
            Node::Include(inc) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
                out.extend(build_include(inc, &ctx, parent_dir, inherited, renderer));
            }
            Node::Embed(_) => {
                flush_text(&mut text_buf, &mut out, parent_dir, inherited);
            }
        }
    }

    flush_text(&mut text_buf, &mut out, parent_dir, inherited);
    out
}

/// Emit accumulated text lines as a single `Content` child and clear the buffer.
fn flush_text(
    buf: &mut Vec<Line<'static>>,
    out: &mut Vec<WidgetChild>,
    parent_dir: Direction,
    inherited: Style,
) {
    if buf.is_empty() {
        return;
    }
    let lines = std::mem::take(buf);
    let height = lines.len() as u16;
    let constraint = match parent_dir {
        // In a vertical stack, text takes exactly as many rows as it has lines.
        Direction::Vertical => Constraint::Length(height),
        // In a horizontal stack, text shares space equally with siblings.
        Direction::Horizontal => Constraint::Fill(1),
    };
    out.push(WidgetChild {
        node: WidgetNode::Content {
            lines,
            style: inherited,
            block: None,
            alignment: Alignment::Left,
        },
        constraint,
    });
}

fn build_element(
    el: &Element,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> WidgetChild {
    // ── Special pseudo-elements ───────────────────────────────────────────────
    match el.tag.as_str() {
        "slot" => return build_slot(el, ctx, parent_dir, inherited, renderer),
        "slot-rotate" => return build_slot_rotate(el, ctx, parent_dir, inherited),
        _ => {}
    }

    // ── Evaluate conditional classes ──────────────────────────────────────────
    let mut classes = active_classes(el, ctx);
    if el.tag == "button"
        && !classes.iter().any(|class| {
            matches!(
                class.as_str(),
                "tui-button" | "button" | "tui-button-active" | "button-active"
            )
        })
    {
        classes.push("tui-button".to_string());
    }
    let mut hints = parse_classes(&classes);

    if let Some(ref el_id) = el.id {
        hints.id = Some(el_id.clone());
    }

    let focused_id = ctx.get_str("focused_id");
    if !focused_id.is_empty()
        && hints.id.as_deref() == Some(focused_id.as_str())
        && hints.focus_border_fg.is_some()
    {
        hints.border_fg = hints.focus_border_fg;
        if hints.borders == Borders::NONE {
            hints.borders = Borders::ALL;
        }
    }

    // Allow a `title={expr}` binding to set the block title.
    if let Some(title_val) = el
        .bindings
        .iter()
        .find(|b| b.prop == "title")
        .map(|b| value_to_str(&eval_value(&b.value, ctx)))
    {
        hints.title = Some(title_val);
    }

    let constraint = adjusted_constraint(&hints, parent_dir);
    let style = inherited.patch(hints.to_style());

    // ── Detect pure-text vs. mixed/structural children ────────────────────────
    let is_pure_text = !el.children.is_empty()
        && el.children.iter().all(|n| {
            matches!(
                n,
                Node::Text(_) | Node::RawText(_) | Node::RawHtml(_) | Node::LetDecl(_)
            )
        });

    if is_pure_text {
        build_content_element(el, &hints, ctx, constraint, style)
    } else if el.children.is_empty() {
        build_empty_box(&hints, constraint, style)
    } else {
        build_container_element(
            el,
            &hints,
            ctx,
            parent_dir,
            ContainerProps {
                constraint,
                style,
                classes: &classes,
            },
            renderer,
        )
    }
}

fn adjusted_constraint(hints: &StyleHints, parent_dir: Direction) -> Constraint {
    match hints.constraint_for(parent_dir) {
        Constraint::Length(n) => Constraint::Length(match parent_dir {
            Direction::Horizontal => n.max(min_width(hints)),
            Direction::Vertical => n.max(min_height(hints)),
        }),
        constraint => constraint,
    }
}

fn min_width(hints: &StyleHints) -> u16 {
    let borders = u16::from(hints.borders.intersects(Borders::LEFT))
        + u16::from(hints.borders.intersects(Borders::RIGHT));
    borders + hints.padding.left + hints.padding.right + 1
}

fn min_height(hints: &StyleHints) -> u16 {
    let borders = u16::from(hints.borders.intersects(Borders::TOP))
        + u16::from(hints.borders.intersects(Borders::BOTTOM));
    borders + hints.padding.top + hints.padding.bottom + 1
}

/// Element whose children are all text → render as a `Paragraph`.
fn build_content_element(
    el: &Element,
    hints: &StyleHints,
    ctx: &TemplateContext,
    constraint: Constraint,
    style: Style,
) -> WidgetChild {
    let mut child_ctx: std::borrow::Cow<'_, TemplateContext> = std::borrow::Cow::Borrowed(ctx);

    let lines: Vec<Line<'static>> = el
        .children
        .iter()
        .filter_map(|child| match child {
            Node::Text(parts) => Some(build_line(parts, &child_ctx)),
            Node::RawText(expr) => Some(Line::raw(value_to_str(&eval_value(expr, &child_ctx)))),
            Node::LetDecl(decl) => {
                let val = eval_value(&decl.expr, &child_ctx);
                if decl.is_default {
                    if !child_ctx.vars.contains_key(&decl.name) {
                        child_ctx.to_mut().vars.insert(decl.name.clone(), val);
                    }
                } else {
                    let child_ctx = child_ctx.to_mut();
                    if let Some(v) = child_ctx.vars.get_mut(&decl.name) {
                        *v = val;
                    } else {
                        child_ctx.vars.insert(decl.name.clone(), val);
                    }
                }
                None
            }
            _ => None,
        })
        .collect();

    // Resolve w-fit for text elements: width = longest line length.
    let constraint = if matches!(hints.width, SizeHint::Fit) {
        let max_width = lines.iter().map(|l| l.width() as u16).max().unwrap_or(0);
        let overhead = border_padding_overhead(hints, Direction::Horizontal);
        Constraint::Length(max_width.saturating_add(overhead))
    } else {
        constraint
    };

    WidgetChild {
        node: WidgetNode::Content {
            lines,
            style,
            block: hints_to_block(hints),
            alignment: hints.alignment,
        },
        constraint,
    }
}

/// Element with no children (spacer, background panel, etc.).
fn build_empty_box(hints: &StyleHints, constraint: Constraint, style: Style) -> WidgetChild {
    WidgetChild {
        node: WidgetNode::Container {
            direction: hints.direction,
            gap: hints.gap,
            children: vec![],
            style,
            block: hints_to_block(hints),
            scroll_offset: 0,
            scroll_bottom: false,
            scrollable: false,
        },
        constraint,
    }
}

pub struct ContainerProps<'a> {
    pub constraint: Constraint,
    pub style: Style,
    pub classes: &'a [String],
}

/// Element with structural children → split its area for each child.
fn build_container_element(
    el: &Element,
    hints: &StyleHints,
    ctx: &TemplateContext,
    _parent_dir: Direction,
    props: ContainerProps<'_>,
    renderer: &mut BufferRenderer,
) -> WidgetChild {
    let child_dir = hints.direction;
    let children = build_children(&el.children, ctx, child_dir, props.style, renderer);
    let scroll_cfg = scroll_config(el, ctx, props.classes);

    // Resolve Fit constraints: measure the natural content size and
    // convert to a Fixed constraint so the element doesn't fill.
    let constraint =
        resolve_fit_constraint(props.constraint, &children, child_dir, hints.gap, hints);

    WidgetChild {
        node: WidgetNode::Container {
            direction: child_dir,
            gap: hints.gap,
            children,
            style: props.style,
            block: hints_to_block(hints),
            scroll_offset: scroll_cfg.offset,
            scroll_bottom: scroll_cfg.scroll_to_bottom,
            scrollable: scroll_cfg.scrollable,
        },
        constraint,
    }
}

/// When a container has a `Fit` size hint (via `h-fit`/`w-fit`), measure the
/// natural content height/width from the built children and return a `Length`
/// constraint so the element takes exactly its content size.
fn resolve_fit_constraint(
    constraint: Constraint,
    children: &[WidgetChild],
    child_dir: Direction,
    gap: u16,
    hints: &StyleHints,
) -> Constraint {
    // Determine which axis needs fitting and compute content size for it.
    // h-fit → measure height; w-fit → measure width.
    let fit_height = matches!(hints.height, SizeHint::Fit);
    let fit_width = matches!(hints.width, SizeHint::Fit);

    if !fit_height && !fit_width {
        return constraint;
    }

    let n = children.len();
    if n == 0 {
        return Constraint::Length(0);
    }

    let total_gap = gap * (n as u16).saturating_sub(1);

    // Content size along the container's main axis (child_dir).
    let main_size: u16 = match child_dir {
        Direction::Vertical => {
            let sum: u16 = children
                .iter()
                .map(|c| constraint_min_height(&c.constraint))
                .sum();
            sum + total_gap
        }
        Direction::Horizontal => {
            let sum: u16 = children
                .iter()
                .map(|c| constraint_min_width(&c.constraint))
                .sum();
            sum + total_gap
        }
    };

    // Content size along the cross axis (perpendicular to child_dir).
    let cross_size: u16 = match child_dir {
        Direction::Vertical => children
            .iter()
            .map(|c| constraint_min_width(&c.constraint))
            .max()
            .unwrap_or(0),
        Direction::Horizontal => children
            .iter()
            .map(|c| constraint_min_height(&c.constraint))
            .max()
            .unwrap_or(0),
    };

    // Pick the right size based on which axis has Fit.
    let (content_size, overhead_axis) = if fit_height {
        // Height fit: if container is flex-col, main axis = height;
        // if flex-row, cross axis = height.
        let size = match child_dir {
            Direction::Vertical => main_size,
            Direction::Horizontal => cross_size,
        };
        (size, Direction::Vertical)
    } else {
        // Width fit: if container is flex-row, main axis = width;
        // if flex-col, cross axis = width.
        let size = match child_dir {
            Direction::Horizontal => main_size,
            Direction::Vertical => cross_size,
        };
        (size, Direction::Horizontal)
    };

    let overhead = border_padding_overhead(hints, overhead_axis);
    Constraint::Length(content_size.saturating_add(overhead))
}

/// Estimate the minimum height a constraint needs.
fn constraint_min_height(c: &Constraint) -> u16 {
    match c {
        Constraint::Length(n) | Constraint::Min(n) => *n,
        Constraint::Fill(_) => 1,
        Constraint::Percentage(_) => 1,
        Constraint::Max(n) => *n,
        _ => 1,
    }
}

/// Estimate the minimum width a constraint needs.
fn constraint_min_width(c: &Constraint) -> u16 {
    match c {
        Constraint::Length(n) | Constraint::Min(n) => *n,
        Constraint::Fill(_) => 1,
        Constraint::Percentage(_) => 1,
        Constraint::Max(n) => *n,
        _ => 1,
    }
}

/// Border + padding cells consumed along the main axis.
fn border_padding_overhead(hints: &StyleHints, dir: Direction) -> u16 {
    match dir {
        Direction::Vertical => {
            let b = u16::from(hints.borders.intersects(Borders::TOP))
                + u16::from(hints.borders.intersects(Borders::BOTTOM));
            b + hints.padding.top + hints.padding.bottom
        }
        Direction::Horizontal => {
            let b = u16::from(hints.borders.intersects(Borders::LEFT))
                + u16::from(hints.borders.intersects(Borders::RIGHT));
            b + hints.padding.left + hints.padding.right
        }
    }
}

/// `<slot>` — renders the caller's slot children (or the element's own children
/// as fallback when no slot was passed).
fn build_slot(
    el: &Element,
    ctx: &TemplateContext,
    _parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> WidgetChild {
    let (nodes, slot_ctx) = if let Some((slot_nodes, slot_ctx_box)) = &ctx.slot {
        (slot_nodes.as_slice(), slot_ctx_box.as_ref())
    } else {
        (el.children.as_slice(), ctx)
    };
    let children = build_children(nodes, slot_ctx, Direction::Vertical, inherited, renderer);
    WidgetChild {
        node: WidgetNode::Container {
            direction: Direction::Vertical,
            gap: 0,
            children,
            style: inherited,
            block: None,
            scroll_offset: 0,
            scroll_bottom: false,
            scrollable: false,
        },
        constraint: Constraint::Fill(1),
    }
}

/// `<slot-rotate>` — TUI does not have CSS animations, but the renderer is
/// invoked on every poll-and-draw cycle (typically every ~100 ms via
/// [`crate::HotTemplate::poll_and_draw_full`]), so we cycle the displayed
/// phrase from wall-clock time. The visible phrase index is
/// `floor(now_ms / interval_ms) % phrases.len()`, so all `slot-rotate`
/// elements with the same interval stay in phase across re-renders and
/// across hot-reloads.
fn build_slot_rotate(
    el: &Element,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
) -> WidgetChild {
    let phrases = slot_rotate_child_phrases(&el.children).unwrap_or_default();
    let interval_ms = slot_rotate_interval_ms(el, ctx);
    let line = current_slot_rotate_line(&phrases, interval_ms, current_unix_ms());

    let classes = active_classes(el, ctx);
    let hints = parse_classes(&classes);
    let constraint = hints.constraint_for(parent_dir);
    let style = inherited.patch(hints.to_style());

    WidgetChild {
        node: WidgetNode::Content {
            lines: vec![line],
            style,
            block: hints_to_block(&hints),
            alignment: hints.alignment,
        },
        constraint,
    }
}

/// Pull the `interval={ms}` binding off `el`, evaluate it against `ctx`, and
/// fall back to 3200 ms (the same default used by the web/SSR backends) on
/// any failure to match the cross-target visual behaviour of `slot-rotate`.
fn slot_rotate_interval_ms(el: &Element, ctx: &TemplateContext) -> u64 {
    const DEFAULT_INTERVAL_MS: u64 = 3200;
    for b in &el.bindings {
        if b.prop == "interval" {
            let v = value_to_str(&eval_value(&b.value, ctx));
            let trimmed = v.trim_matches('"').trim();
            return trimmed.parse().unwrap_or(DEFAULT_INTERVAL_MS);
        }
    }
    DEFAULT_INTERVAL_MS
}

/// Compute the `Line` for the current phrase given the rotation parameters.
///
/// Pulled out so the time-based selection is unit-testable (the production
/// caller passes `current_unix_ms()`; tests pass deterministic values).
fn current_slot_rotate_line(phrases: &[String], interval_ms: u64, now_ms: u64) -> Line<'static> {
    if phrases.is_empty() {
        return Line::raw("");
    }
    let interval = interval_ms.max(1);
    let idx = ((now_ms / interval) as usize) % phrases.len();
    Line::raw(phrases[idx].clone())
}

fn current_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn build_if(
    block: &IfBlock,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> Vec<WidgetChild> {
    let body = if eval_condition(ctx, &block.condition) {
        &block.then_children
    } else if let Some(ref else_children) = block.else_children {
        else_children
    } else {
        return vec![];
    };
    build_children(body, ctx, parent_dir, inherited, renderer)
}

fn build_for(
    block: &ForBlock,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> Vec<WidgetChild> {
    let items = ctx.get_list_ref(&block.iterator);
    let mut out = Vec::new();
    let mut child_ctx = ctx.clone();
    let pattern = block.pattern.trim();
    let has_pattern = !pattern.is_empty();
    // Keys written by the previous iteration, so the scope can be restored in
    // time proportional to the item instead of re-cloning the whole context
    // (which holds the item list itself) on every pass.
    let mut written: Vec<String> = Vec::new();
    for item_ctx in items {
        let item_str = if has_pattern {
            item_ctx.get_str("value")
        } else {
            String::new()
        };
        // Restore child_ctx
        for key in written.drain(..) {
            match ctx.vars.get(&key) {
                Some(v) => {
                    child_ctx.vars.insert(key, v.clone());
                }
                None => {
                    child_ctx.vars.remove(&key);
                }
            }
        }
        // Merge all fields from the item context into the child context.
        for (k, v) in &item_ctx.vars {
            child_ctx.vars.insert(k.clone(), v.clone());
            written.push(k.clone());
        }
        // Bind the loop pattern variable to the item's "value" field if present.
        if has_pattern && !item_str.is_empty() {
            child_ctx
                .vars
                .insert(pattern.to_string(), TemplateValue::Str(item_str));
            written.push(pattern.to_string());
        }
        out.extend(build_children(
            &block.body,
            &child_ctx,
            parent_dir,
            inherited,
            renderer,
        ));
    }
    out
}

fn build_match(
    block: &MatchBlock,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> Vec<WidgetChild> {
    let val = value_to_str(&eval_value(&block.expr, ctx));
    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        let matched = if pattern == "_" {
            true
        } else if pattern.starts_with('"') && pattern.ends_with('"') {
            pattern[1..pattern.len() - 1] == *val
        } else {
            pattern == val
        };
        if matched {
            return build_children(&arm.body, ctx, parent_dir, inherited, renderer);
        }
    }
    vec![]
}

fn prepare_include_context(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    full_path: &std::path::Path,
) -> TemplateContext {
    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = full_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_value(expr, ctx));
    }
    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }
    child_ctx
}

fn build_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    parent_dir: Direction,
    inherited: Style,
    renderer: &mut BufferRenderer,
) -> Vec<WidgetChild> {
    // Split `path#ComponentName` if present.
    let (file_path, component_name) = if let Some((f, c)) = inc.path.split_once('#') {
        (f, Some(c))
    } else {
        (inc.path.as_str(), None)
    };

    let full_path = match resolve_include_path(ctx.base_dir.as_deref(), file_path) {
        Ok(path) => path,
        Err(e) => return error_children(&e.to_string()),
    };

    // Load file content: virtual files take priority, then filesystem.
    let content = match read_file(ctx, &full_path) {
        Ok(c) => c,
        Err(e) => return error_children(&e.to_string()),
    };
    let child_ctx = prepare_include_context(inc, ctx, &full_path);

    // Pull the cached parsed nodes (or parse + cache on miss / re-parse on
    // invalidation). Missing or invalid includes retain the existing inline
    // warning-node behaviour and MUST NOT be cached.
    let cached_nodes = match renderer.load_include(&full_path, component_name, &content) {
        Ok(nodes) => nodes,
        Err(error_nodes) => return error_nodes,
    };
    build_children(&cached_nodes, &child_ctx, parent_dir, inherited, renderer)
}

// ─── Phase 2: Paint ───────────────────────────────────────────────────────────

/// Recursively render a `WidgetNode` tree into `frame` at `area`.
pub fn paint_node(node: &WidgetNode, frame: &mut Frame, area: Rect) {
    paint_node_buf(node, frame.buffer_mut(), area);
}

fn paint_node_buf(node: &WidgetNode, buf: &mut Buffer, area: Rect) {
    match node {
        WidgetNode::Container {
            direction,
            gap,
            children,
            style,
            block,
            scroll_offset,
            scroll_bottom,
            scrollable,
        } => {
            // Fill background / draw border, then recurse into the inner area.
            let inner = if let Some(spec) = block {
                let b = spec.to_block().style(*style);
                let inner = b.inner(area);
                b.render(area, buf);
                inner
            } else if *style != Style::default() {
                // Background colour only — no border.
                Block::default().style(*style).render(area, buf);
                area
            } else {
                area
            };

            if *scrollable {
                Scrollable::new(
                    children,
                    *direction,
                    *gap,
                    *scroll_offset,
                    *scroll_bottom,
                    inner,
                )
                .paint(buf);
            } else {
                paint_children_buf(children, buf, inner, *direction, *gap);
            }
        }

        WidgetNode::Content {
            lines,
            style,
            block,
            alignment,
        } => {
            let text = Text::from(lines.clone());
            let para = Paragraph::new(text)
                .style(*style)
                .alignment(*alignment)
                .wrap(Wrap { trim: false });
            if let Some(spec) = block {
                para.block(spec.to_block()).render(area, buf);
            } else {
                para.render(area, buf);
            }
        }

        WidgetNode::Empty => {}
    }
}

/// Preferred size of a constraint along the layout axis, before growing or
/// shrinking to fit the available space.
fn constraint_preferred(c: &Constraint, total: u16) -> u16 {
    match c {
        Constraint::Length(v) | Constraint::Min(v) | Constraint::Max(v) => *v,
        Constraint::Percentage(p) => (u32::from(total) * u32::from(*p) / 100) as u16,
        Constraint::Ratio(a, b) => {
            if *b == 0 {
                0
            } else {
                (u64::from(total) * u64::from(*a) / u64::from(*b)) as u16
            }
        }
        Constraint::Fill(_) => 0,
    }
}

/// How eagerly a constraint absorbs leftover space. `Fill(w)` grows with
/// weight `w`; `Min(_)` grows with weight 1; everything else stays put.
fn constraint_grow_weight(c: &Constraint) -> u32 {
    match c {
        Constraint::Fill(w) => u32::from(*w),
        Constraint::Min(_) => 1,
        _ => 0,
    }
}

/// Smallest size a constraint may be shrunk to before the layout is allowed to
/// violate it.
fn constraint_floor(c: &Constraint) -> u16 {
    match c {
        Constraint::Min(v) => *v,
        _ => 0,
    }
}

/// Split `area` into one `Rect` per constraint in linear time.
///
/// This replaces `ratatui::Layout::split` on the hot path. The cassowary solver
/// behind `Layout` degrades catastrophically once a single split carries more
/// than ~30 constraints — which a `for` loop over a list trivially produces —
/// so the flex distribution is computed directly instead: preferred sizes, then
/// leftover space to the growable children (or the last child, matching
/// `Flex::Legacy`), then proportional shrinking from the end when the children
/// overflow.
fn split_area(constraints: &[Constraint], area: Rect, direction: Direction, gap: u16) -> Vec<Rect> {
    let n = constraints.len();
    if n == 0 {
        return Vec::new();
    }
    let total = match direction {
        Direction::Horizontal => area.width,
        Direction::Vertical => area.height,
    };
    let spacing = gap.saturating_mul((n as u16).saturating_sub(1));
    let avail = u32::from(total.saturating_sub(spacing));

    let mut sizes: Vec<u32> = constraints
        .iter()
        .map(|c| u32::from(constraint_preferred(c, total)))
        .collect();
    let sum: u32 = sizes.iter().sum();

    if sum < avail {
        let extra = avail - sum;
        let weights: Vec<u32> = constraints.iter().map(constraint_grow_weight).collect();
        let wsum: u32 = weights.iter().sum();
        if let Some(base) = extra.checked_div(wsum) {
            let mut used = 0;
            for (i, w) in weights.iter().enumerate() {
                let add = base * w;
                sizes[i] += add;
                used += add;
            }
            let mut rem = extra - used;
            for (i, w) in weights.iter().enumerate() {
                if rem == 0 {
                    break;
                }
                if *w > 0 {
                    sizes[i] += 1;
                    rem -= 1;
                }
            }
        } else if let Some(last) = sizes.last_mut() {
            *last += extra;
        }
    } else if sum > avail {
        // Shrink from the end, first down to each child's floor, then past it.
        let mut deficit = sum - avail;
        for i in (0..n).rev() {
            if deficit == 0 {
                break;
            }
            let floor = u32::from(constraint_floor(&constraints[i]));
            let slack = sizes[i].saturating_sub(floor);
            let cut = slack.min(deficit);
            sizes[i] -= cut;
            deficit -= cut;
        }
        for i in (0..n).rev() {
            if deficit == 0 {
                break;
            }
            let cut = sizes[i].min(deficit);
            sizes[i] -= cut;
            deficit -= cut;
        }
    }

    // Clamp `Max` constraints after distribution so a `Max(v)` never exceeds v.
    for (i, c) in constraints.iter().enumerate() {
        if let Constraint::Max(v) = c {
            sizes[i] = sizes[i].min(u32::from(*v));
        }
    }

    let mut out = Vec::with_capacity(n);
    let mut offset = u32::from(match direction {
        Direction::Horizontal => area.x,
        Direction::Vertical => area.y,
    });
    let end = offset + u32::from(total);
    for size in sizes {
        let start = offset.min(end);
        let len = size.min(end - start);
        out.push(match direction {
            Direction::Horizontal => Rect::new(start as u16, area.y, len as u16, area.height),
            Direction::Vertical => Rect::new(area.x, start as u16, area.width, len as u16),
        });
        offset = start + len + u32::from(gap);
    }
    out
}

fn paint_children_buf(
    children: &[WidgetChild],
    buf: &mut Buffer,
    area: Rect,
    direction: Direction,
    gap: u16,
) {
    if children.is_empty() {
        return;
    }
    let constraints: Vec<Constraint> = children.iter().map(|c| c.constraint).collect();
    let chunks = split_area(&constraints, area, direction, gap);

    for (i, child) in children.iter().enumerate() {
        if let Some(&chunk) = chunks.get(i) {
            paint_node_buf(&child.node, buf, chunk);
        }
    }
}

struct Scrollable<'a> {
    children: &'a [WidgetChild],
    direction: Direction,
    gap: u16,
    scroll_offset: usize,
    scroll_to_bottom: bool,
    viewport: Rect,
}

impl<'a> Scrollable<'a> {
    fn new(
        children: &'a [WidgetChild],
        direction: Direction,
        gap: u16,
        scroll_offset: usize,
        scroll_to_bottom: bool,
        viewport: Rect,
    ) -> Self {
        Scrollable {
            children,
            direction,
            gap,
            scroll_offset,
            scroll_to_bottom,
            viewport,
        }
    }

    fn total_content_height(&self) -> u16 {
        let n = self.children.len() as u16;
        let total_gap = self.gap.saturating_mul(n.saturating_sub(1));
        let sum: u16 = self
            .children
            .iter()
            .map(|c| constraint_min_height(&c.constraint))
            .sum();
        sum.saturating_add(total_gap)
    }

    fn clamped_offset(&self) -> usize {
        let total = self.total_content_height() as usize;
        let max = total.saturating_sub(self.viewport.height as usize);
        if self.scroll_to_bottom {
            return max;
        }
        self.scroll_offset.min(max)
    }

    fn paint(&self, buf: &mut Buffer) {
        if self.children.is_empty() || self.viewport.height == 0 {
            return;
        }
        if self.direction != Direction::Vertical {
            paint_children_buf(self.children, buf, self.viewport, self.direction, self.gap);
            return;
        }
        let total_height = self.total_content_height();
        if total_height <= self.viewport.height {
            paint_children_buf(self.children, buf, self.viewport, self.direction, self.gap);
            return;
        }
        let offset = self.clamped_offset();
        let virtual_area = Rect::new(0, 0, self.viewport.width, total_height);
        let mut virtual_buf = Buffer::empty(virtual_area);
        let constraints: Vec<Constraint> = self.children.iter().map(|c| c.constraint).collect();
        let chunks = split_area(&constraints, virtual_area, self.direction, self.gap);
        let vis_start = offset as u16;
        let vis_end = vis_start + self.viewport.height;
        for (i, child) in self.children.iter().enumerate() {
            if let Some(&chunk) = chunks.get(i) {
                if chunk.bottom() <= vis_start || chunk.y >= vis_end {
                    continue;
                }
                paint_node_buf(&child.node, &mut virtual_buf, chunk);
            }
        }
        for y in 0..self.viewport.height {
            let src_y = offset as u16 + y;
            if src_y >= total_height {
                break;
            }
            for x in 0..self.viewport.width {
                let cell = std::mem::take(&mut virtual_buf[(x, src_y)]);
                buf[(self.viewport.x + x, self.viewport.y + y)] = cell;
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Resolve active classes for an element (static + conditional).
fn active_classes(el: &Element, ctx: &TemplateContext) -> Vec<String> {
    let mut classes: Vec<String> = el
        .classes
        .iter()
        .filter_map(|c| active_class(ctx, &interpolate_str(ctx, c)))
        .collect();
    for cc in &el.conditional_classes {
        if eval_condition(ctx, &cc.condition) {
            if let Some(class) = active_class(ctx, &interpolate_str(ctx, &cc.class)) {
                classes.push(class);
            }
        }
    }
    classes
}

fn active_class(ctx: &TemplateContext, class: &str) -> Option<String> {
    if let Some((prefix, rest)) = class.split_once(':') {
        if matches_target_prefix(ctx, prefix) {
            return Some(rest.to_string());
        }
        if known_target_prefix(prefix) {
            return None;
        }
    }
    Some(class.to_string())
}

fn scroll_config(el: &Element, ctx: &TemplateContext, classes: &[String]) -> ScrollConfig {
    let scrollable = classes.iter().any(|class| {
        matches!(
            class.as_str(),
            "overflow-y-scroll" | "overflow-scroll" | "virtual-scroll"
        )
    });
    if !scrollable {
        return ScrollConfig::default();
    }
    let scroll_to_bottom = el
        .bindings
        .iter()
        .find(|b| b.prop == "scroll-bottom")
        .map(|b| {
            let v = eval_value(&b.value, ctx);
            match v {
                TemplateValue::Bool(b) => b,
                _ => {
                    let s = value_to_str(&v);
                    s == "true" || s == "1"
                }
            }
        })
        .unwrap_or(false);
    let offset = el
        .bindings
        .iter()
        .find(|binding| {
            matches!(
                binding.prop.as_str(),
                "scroll-offset" | "scroll-y" | "scroll"
            )
        })
        .map(|binding| value_to_str(&eval_value(&binding.value, ctx)))
        .or_else(|| {
            let value = ctx.get_str("scroll_offset");
            (!value.is_empty()).then_some(value)
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    ScrollConfig {
        offset,
        scroll_to_bottom,
        scrollable: true,
    }
}

#[derive(Default)]
struct ScrollConfig {
    offset: usize,
    scroll_to_bottom: bool,
    scrollable: bool,
}

fn matches_target_prefix(ctx: &TemplateContext, prefix: &str) -> bool {
    match prefix {
        "tui" => ctx.get_bool("is_tui") || ctx.get_str("crepus_target") == "tui",
        "gpui" | "gui" => ctx.get_bool("is_gui") || ctx.get_str("crepus_target") == "gpui",
        "web" => ctx.get_bool("is_web") || ctx.get_str("crepus_target") == "web",
        "macos" | "darwin" => ctx.get_bool("is_macos") || ctx.get_str("crepus_platform") == "macos",
        "windows" | "win32" => {
            ctx.get_bool("is_windows") || ctx.get_str("crepus_platform") == "windows"
        }
        "linux" => ctx.get_bool("is_linux") || ctx.get_str("crepus_platform") == "linux",
        _ => false,
    }
}

fn known_target_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "tui" | "gpui" | "gui" | "web" | "macos" | "darwin" | "windows" | "win32" | "linux"
    )
}

/// Build a ratatui `Line` from a `TextPart` slice.
fn build_line(parts: &[TextPart], ctx: &TemplateContext) -> Line<'static> {
    let spans: Vec<Span<'static>> = parts
        .iter()
        .map(|part| match part {
            TextPart::Literal(s) => Span::raw(s.clone()),
            TextPart::Expr(expr) => Span::raw(value_to_str(&eval_value(expr, ctx))),
        })
        .collect();
    Line::from(spans)
}

/// Convert `StyleHints` into a `BlockSpec` if borders / title / padding are needed.
fn hints_to_block(hints: &StyleHints) -> Option<BlockSpec> {
    if !hints.needs_block() {
        return None;
    }
    let mut border_style = Style::default();
    if let Some(fg) = hints.border_fg {
        border_style = border_style.fg(fg);
    }
    Some(BlockSpec {
        borders: hints.borders,
        border_type: hints.border_type,
        border_style,
        title: hints.title.clone(),
        title_alignment: hints.title_alignment,
        padding: hints.padding,
    })
}

/// Return a single-line error `WidgetChild` in red (mirrors the runtime convention).
fn error_children(msg: &str) -> Vec<WidgetChild> {
    vec![WidgetChild {
        node: WidgetNode::Content {
            lines: vec![Line::from(vec![Span::styled(
                format!(" ⚠ {msg} "),
                Style::default().fg(Color::Red),
            )])],
            style: Style::default(),
            block: None,
            alignment: Alignment::Left,
        },
        constraint: Constraint::Length(1),
    }]
}

#[cfg(test)]
mod slot_rotate_tests {
    use super::*;

    fn lines(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| (*s).to_string()).collect()
    }

    fn rendered(line: Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn current_slot_rotate_line_cycles_with_time() {
        let phrases = lines(&["one", "two", "three"]);
        let interval = 1000u64;

        // t=0   → idx 0
        // t=999 → idx 0 (still in the first window)
        // t=1000→ idx 1
        // t=2999→ idx 2
        // t=3000→ wraps back to idx 0
        assert_eq!(
            rendered(current_slot_rotate_line(&phrases, interval, 0)),
            "one"
        );
        assert_eq!(
            rendered(current_slot_rotate_line(&phrases, interval, 999)),
            "one"
        );
        assert_eq!(
            rendered(current_slot_rotate_line(&phrases, interval, 1000)),
            "two"
        );
        assert_eq!(
            rendered(current_slot_rotate_line(&phrases, interval, 2999)),
            "three"
        );
        assert_eq!(
            rendered(current_slot_rotate_line(&phrases, interval, 3000)),
            "one"
        );
    }

    #[test]
    fn current_slot_rotate_line_handles_empty() {
        let line = current_slot_rotate_line(&[], 1000, 12345);
        assert_eq!(rendered(line), "");
    }

    #[test]
    fn current_slot_rotate_line_handles_zero_interval() {
        // Guard against the obvious division-by-zero footgun.
        let phrases = lines(&["a", "b"]);
        let line = current_slot_rotate_line(&phrases, 0, 4242);
        // Falls back to a 1ms interval, so idx = 4242 % 2 = 0 → "a".
        assert_eq!(rendered(line), "a");
    }

    #[test]
    fn current_slot_rotate_line_handles_single_phrase() {
        let phrases = lines(&["solo"]);
        // Any time should still produce "solo" — modulo 1 is 0.
        for t in [0u64, 1, 1000, 1_000_000_000] {
            assert_eq!(
                rendered(current_slot_rotate_line(&phrases, 1000, t)),
                "solo"
            );
        }
    }
}
