//! The single AST walker behind both the plain HTML renderer and the SSR renderer.
//!
//! [`Walker`] is parameterised by an emit mode: in plain mode it writes bare HTML and
//! tracks include nesting through a threaded depth counter; in SSR mode it additionally
//! allocates `data-crepus-id` bindings, wraps dynamic regions in spans or comment
//! ranges for restricted HTML parents, and defers include-depth accounting to [`crate::ssr::IncludeDepthGuard`].

use std::cell::Cell;

use crepuscularity_core::analysis::{classify_regions, Region, RegionMap};
use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::{eval_binding, eval_expr};
use crepuscularity_core::parser::parse_component_file;
use crepuscularity_core::preprocess::{slot_rotate_child_phrases, slot_rotate_words_json_attr};
use crepuscularity_core::CrepusError;
use serde_json::{json, Value};

// A span is not a valid region wrapper in these HTML insertion contexts.
fn restricted_marker_parent(tag: &str) -> bool {
    [
        "table", "thead", "tbody", "tfoot", "tr", "colgroup", "select", "optgroup", "option",
    ]
    .iter()
    .any(|parent| tag.eq_ignore_ascii_case(parent))
}

pub type BindMap = serde_json::Map<String, Value>;

struct SlotFrame<'a> {
    nodes: &'a [Node],
    context: &'a TemplateContext,
    caller_slot: Option<&'a SlotFrame<'a>>,
    regions: Option<&'a RegionMap<'a>>,
}

pub(crate) struct SsrState<'a> {
    counter: &'a Cell<u32>,
    bind: &'a mut BindMap,
    root_pending: bool,
}

pub(crate) struct Walker<'a> {
    ssr: Option<SsrState<'a>>,
    depth: usize,
    range_markers: bool,
    virtual_only: bool,
}

impl<'a> Walker<'a> {
    pub(crate) fn plain(depth: usize) -> Self {
        Self {
            ssr: None,
            depth,
            range_markers: false,
            virtual_only: false,
        }
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn ssr(counter: &'a Cell<u32>, bind: &'a mut BindMap, root_pending: bool) -> Self {
        Self {
            ssr: Some(SsrState {
                counter,
                bind,
                root_pending,
            }),
            depth: 0,
            range_markers: false,
            virtual_only: false,
        }
    }

    pub(crate) fn with_virtual_only(mut self, virtual_only: bool) -> Self {
        self.virtual_only = virtual_only;
        self
    }

    fn read_include(
        &self,
        ctx: &TemplateContext,
        path: &str,
    ) -> Result<(std::path::PathBuf, String), CrepusError> {
        if !self.virtual_only {
            let resolved = crate::resolve_include_path(ctx.base_dir.as_deref(), path)?;
            let content = crate::read_file(ctx, &resolved)?;
            return Ok((resolved, content));
        }
        let requested = std::path::Path::new(path);
        if requested.has_root()
            || requested.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(CrepusError::include_path(format!(
                "include path outside base dir: {path}"
            )));
        }
        // Virtual bundles are resolved lexically: disk symlinks must not affect them.
        let resolved = ctx
            .base_dir
            .as_deref()
            .map_or_else(|| requested.to_path_buf(), |base| base.join(requested));
        let content = crepuscularity_core::virtual_files::lookup_virtual_file(ctx, &resolved)
            .ok_or_else(|| {
                CrepusError::render(format!(
                    "include error: file not in virtual bundle: {}",
                    resolved.display()
                ))
            })?;
        Ok((resolved, content))
    }

    fn is_ssr(&self) -> bool {
        self.ssr.is_some()
    }

    fn root_pending(&self) -> bool {
        self.ssr.as_ref().is_some_and(|s| s.root_pending)
    }

    /// Set the pending-root flag, returning the previous value. Plain mode has no root marker.
    fn set_root_pending(&mut self, value: bool) -> bool {
        match &mut self.ssr {
            Some(state) => std::mem::replace(&mut state.root_pending, value),
            None => false,
        }
    }

    /// Allocate a hydration binding id. Returns `None` — and never builds the
    /// descriptor — in plain mode.
    fn alloc(&mut self, detail: impl FnOnce() -> Value) -> Option<u32> {
        let state = self.ssr.as_mut()?;
        let id = state.counter.get();
        state.counter.set(id + 1);
        state.bind.insert(id.to_string(), detail());
        Some(id)
    }

    /// Wrap `inner` in a marker span or restricted-parent comment range.
    fn finish(&mut self, kind: &str, id: Option<u32>, inner: String) -> String {
        match id {
            Some(id) if self.range_markers => {
                if let Some(descriptor) = self
                    .ssr
                    .as_mut()
                    .and_then(|state| state.bind.get_mut(&id.to_string()))
                {
                    descriptor["marker"] = json!("range");
                }
                // HTML can insert implicit table bodies between these endpoints;
                // ranges do not promise that the two comments share a parent.
                format!("<!--crepus:{kind}:c{id}:start-->{inner}<!--crepus:{kind}:c{id}:end-->")
            }
            Some(id) => format!(
                r#"<span style="display:contents" data-crepus-kind="{kind}" data-crepus-id="c{id}">{inner}</span>"#
            ),
            None => inner,
        }
    }

    pub(crate) fn nodes(
        &mut self,
        nodes: &[Node],
        base: &TemplateContext,
    ) -> Result<String, CrepusError> {
        let regions = self.is_ssr().then(|| classify_regions(nodes));
        self.nodes_with_slot(nodes, base, None, regions.as_ref())
    }

    fn nodes_with_slot(
        &mut self,
        nodes: &[Node],
        base: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        let _span = (!self.is_ssr())
            .then(|| tracing::debug_span!("render_html", node_count = nodes.len()).entered());

        let mut overlay: Option<TemplateContext> = None;
        let mut html = String::new();

        for node in nodes {
            if let Node::LetDecl(decl) = node {
                let cur = overlay.as_ref().unwrap_or(base);
                if decl.is_default && cur.vars.contains_key(&decl.name) {
                    continue;
                }
                let overlay_ctx = overlay.get_or_insert_with(|| base.clone());
                let val = eval_expr(&decl.expr, overlay_ctx)?;
                overlay_ctx.vars.insert(decl.name.clone(), val);
                continue;
            }
            let cur = overlay.as_ref().unwrap_or(base);
            html.push_str(&self.node_with_slot(node, cur, slot, regions)?);
        }

        Ok(html)
    }

    /// Render a nested node list that can never carry the document root marker.
    fn nested(
        &mut self,
        nodes: &[Node],
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        let saved = self.set_root_pending(false);
        let out = self.nodes_with_slot(nodes, ctx, slot, regions);
        self.set_root_pending(saved);
        out
    }

    #[cfg(feature = "hydration")]
    pub(crate) fn node(
        &mut self,
        node: &Node,
        ctx: &TemplateContext,
    ) -> Result<String, CrepusError> {
        let regions = self
            .is_ssr()
            .then(|| classify_regions(std::slice::from_ref(node)));
        self.node_with_slot(node, ctx, None, regions.as_ref())
    }

    fn node_with_slot(
        &mut self,
        node: &Node,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        match node {
            Node::Element(el) => {
                let inject_id = self.is_ssr()
                    && !matches!(el.tag.as_str(), "slot" | "slot-rotate")
                    && regions
                        .and_then(|index| index.get(node))
                        .expect("SSR AST node is indexed")
                        == &Region::Dynamic;
                self.element(el, ctx, slot, regions, inject_id)
            }
            Node::Text(parts) => {
                let id = if self.is_ssr()
                    && regions
                        .and_then(|index| index.get(node))
                        .expect("SSR AST node is indexed")
                        == &Region::Dynamic
                {
                    self.alloc(|| {
                        json!({
                            "kind": "text",
                            "parts": text_parts_manifest(parts),
                        })
                    })
                } else {
                    None
                };
                let inner = crate::escape_html(&crate::render_text(parts, ctx)?);
                Ok(self.finish("text", id, inner))
            }
            Node::If(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "if",
                        "condition": block.condition,
                    })
                });
                let inner = if ctx.eval_condition(&block.condition)? {
                    self.nested(&block.then_children, ctx, slot, regions)?
                } else if let Some(else_children) = &block.else_children {
                    self.nested(else_children, ctx, slot, regions)?
                } else {
                    String::new()
                };
                Ok(self.finish("if", id, inner))
            }
            Node::For(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "for",
                        "pattern": block.pattern,
                        "iterator": block.iterator,
                    })
                });
                let inner = self.for_body(block, ctx, slot, regions)?;
                Ok(self.finish("for", id, inner))
            }
            Node::Match(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "match",
                        "expr": block.expr,
                    })
                });
                let inner = self.match_body(block, ctx, slot, regions)?;
                Ok(self.finish("match", id, inner))
            }
            Node::LetDecl(_) => Ok(String::new()),
            Node::Include(inc) => self.include(inc, ctx, slot, regions),
            Node::Embed(embed) => crate::render_embed(embed, ctx),
            Node::RawText(expr) => {
                let id = self.alloc(|| json!({ "kind": "raw", "expr": expr }));
                let inner = crate::escape_html(&value_to_str(&eval_expr(expr, ctx)?));
                Ok(self.finish("raw", id, inner))
            }
            Node::RawHtml(expr) => {
                let id = self.alloc(|| json!({ "kind": "raw", "expr": expr }));
                let inner = ammonia::clean(&value_to_str(&eval_expr(expr, ctx)?));
                Ok(self.finish("raw", id, inner))
            }
        }
    }

    fn for_body(
        &mut self,
        block: &ForBlock,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        let TemplateValue::List(items) = eval_expr(&block.iterator, ctx)? else {
            return Ok(String::new());
        };
        let mut out = String::new();
        let pattern = block.pattern.trim();
        let has_pattern = !pattern.is_empty();
        let mut child_ctx = ctx.clone();

        let mut overwritten = Vec::new();
        for item_ctx in items {
            for (k, v) in &item_ctx.vars {
                overwritten.push((k.clone(), child_ctx.vars.insert(k.clone(), v.clone())));
            }
            if has_pattern {
                let value = item_ctx
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| TemplateValue::Scope(item_ctx.clone()));
                overwritten.push((
                    pattern.to_string(),
                    child_ctx.vars.insert(pattern.to_string(), value),
                ));
            }
            let rendered = self.nested(&block.body, &child_ctx, slot, regions);
            // Reverse restores duplicate writes (an item field can also be the alias).
            for (key, value) in overwritten.drain(..).rev() {
                match value {
                    Some(value) => {
                        child_ctx.vars.insert(key, value);
                    }
                    None => {
                        child_ctx.vars.remove(&key);
                    }
                }
            }
            out.push_str(&rendered?);
        }

        Ok(out)
    }

    fn match_body(
        &mut self,
        block: &MatchBlock,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        let val = eval_expr(&block.expr, ctx)?;
        let value = value_to_str(&val);

        for arm in &block.arms {
            let pattern = arm.pattern.trim();
            if pattern == "_" {
                return self.nested(&arm.body, ctx, slot, regions);
            }
            if let Some(literal) = pattern.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                let lit = crepuscularity_core::parser::unescape_crepus_text_literal(literal);
                if value == lit {
                    return self.nested(&arm.body, ctx, slot, regions);
                }
            } else if value == pattern {
                return self.nested(&arm.body, ctx, slot, regions);
            }
        }

        Ok(String::new())
    }

    fn element(
        &mut self,
        el: &Element,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
        inject_id: bool,
    ) -> Result<String, CrepusError> {
        crate::void_html::validate_children(el)?;
        if el.tag == "slot" {
            if let Some(frame) = slot {
                return self.nested(frame.nodes, frame.context, frame.caller_slot, frame.regions);
            }
            return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
                let regions = self.is_ssr().then(|| classify_regions(slot_nodes));
                self.nested(slot_nodes, slot_ctx, None, regions.as_ref())
            } else {
                self.nested(&el.children, ctx, slot, regions)
            };
        }

        if el.tag == "slot-rotate" {
            return self.slot_rotate(el, ctx);
        }

        let inject_root = inject_id && self.root_pending();
        if inject_id {
            self.set_root_pending(false);
        }
        let id_opt = if inject_id {
            self.alloc(|| json!({ "kind": "element", "tag": el.tag }))
        } else {
            None
        };

        let mut out = String::new();
        out.push('<');
        out.push_str(&el.tag);

        if let Some(dom_id) = &el.id {
            out.push_str(" id=\"");
            out.push_str(&crate::escape_html_attribute(dom_id));
            out.push('"');
        }
        if inject_root {
            out.push_str(r#" data-crepus-root="1""#);
        }
        if let Some(id) = id_opt {
            out.push_str(&format!(
                r#" data-crepus-id="c{id}" data-crepus-kind="element""#
            ));
        }

        if let Some(class_attr) =
            crate::build_class_attr(&el.classes, &el.conditional_classes, ctx)?
        {
            out.push_str(" class=\"");
            out.push_str(&class_attr);
            out.push('"');
        }

        push_bindings(&mut out, el, ctx, false)?;
        push_handlers_and_animations(&mut out, el)?;

        if crate::void_html::is_void_html_tag(&el.tag) {
            out.push_str(" />");
            return Ok(out);
        }

        out.push('>');
        let previous =
            std::mem::replace(&mut self.range_markers, restricted_marker_parent(&el.tag));
        let children = (|| -> Result<(), CrepusError> {
            if el.tag.eq_ignore_ascii_case("title") {
                // Title is RCDATA: region wrappers would become visible title text.
                out.push_str(
                    &Walker::plain(self.depth)
                        .with_virtual_only(self.virtual_only)
                        .nodes_with_slot(&el.children, ctx, slot, None)?,
                );
            } else if let Some(value) =
                textarea_content_with_slot(el, ctx, self.depth, slot, self.virtual_only)?
            {
                out.push_str(&value);
            } else {
                out.push_str(&self.nested(&el.children, ctx, slot, regions)?);
            }
            Ok(())
        })();
        self.range_markers = previous;
        children?;
        out.push_str("</");
        out.push_str(&el.tag);
        out.push('>');
        Ok(out)
    }

    fn slot_rotate(&mut self, el: &Element, ctx: &TemplateContext) -> Result<String, CrepusError> {
        let phrases = slot_rotate_child_phrases(&el.children).map_err(CrepusError::render)?;
        if phrases.len() < 2 {
            return Err(CrepusError::render(
                "slot-rotate needs at least two plain-text phrase children",
            ));
        }
        let mut interval_ms = 3200u64;
        for b in &el.bindings {
            if b.prop == "interval" && b.origin != BindingOrigin::Attribute {
                let v = value_to_str(&eval_expr(&b.value, ctx)?);
                let v = v.trim_matches('"').trim();
                interval_ms = v
                    .parse::<u64>()
                    .ok()
                    .filter(|ms| (1..=i32::MAX as u64).contains(ms))
                    .unwrap_or(3200);
            }
        }
        let words_json = slot_rotate_words_json_attr(&phrases);
        let id_opt = self.alloc(|| {
            json!({
                "kind": "slot-rotate",
                "intervalMs": interval_ms,
                "phrases": phrases,
            })
        });

        // Build class list with the "crepus-slot" prefix prepended.
        let mut class_names: Vec<String> = Vec::with_capacity(el.classes.len() + 1);
        class_names.push("crepus-slot".to_string());
        class_names.extend(el.classes.iter().cloned());

        let inject_root = self.set_root_pending(false);

        let mut out = String::new();
        out.push_str("<span");
        if inject_root {
            out.push_str(r#" data-crepus-root="1""#);
        }
        if let Some(dom_id) = &el.id {
            out.push_str(" id=\"");
            out.push_str(&crate::escape_html_attribute(dom_id));
            out.push('"');
        }
        if let Some(id) = id_opt {
            out.push_str(&format!(r#" data-crepus-id="c{id}""#));
            out.push_str(" data-crepus-kind=\"slot-rotate\"");
        }
        let mut class_attr = crate::build_class_attr(&class_names, &el.conditional_classes, ctx)?
            .unwrap_or_default();
        if let Some(binding) = el.bindings.iter().find(|binding| {
            binding.origin == BindingOrigin::Attribute && binding.prop.eq_ignore_ascii_case("class")
        }) {
            if let Some(value) = eval_binding(binding, ctx)? {
                class_attr.push(' ');
                class_attr.push_str(&crate::escape_html_attribute(&value_to_str(&value)));
            }
        }
        if !class_attr.is_empty() {
            out.push_str(" class=\"");
            out.push_str(&class_attr);
            out.push('"');
        }
        out.push_str(" data-slot-words=\"");
        out.push_str(&crate::escape_html_attribute(&words_json));
        out.push('"');
        out.push_str(" data-slot-interval=\"");
        out.push_str(&crate::escape_html_attribute(&interval_ms.to_string()));
        out.push('"');
        out.push_str(" aria-live=\"polite\"");

        push_bindings(&mut out, el, ctx, true)?;
        push_handlers_and_animations(&mut out, el)?;

        out.push_str("></span>");
        Ok(out)
    }

    fn include(
        &mut self,
        inc: &IncludeNode,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        #[cfg(feature = "ssr")]
        if self.is_ssr() {
            let _depth_guard = crate::ssr::IncludeDepthGuard::enter(&inc.path)?;
            let id = self.alloc(|| json!({ "kind": "include", "path": inc.path }));
            let inner = self.include_body(inc, ctx, slot, regions)?;
            return Ok(self.finish("include", id, inner));
        }

        if self.depth >= crate::MAX_INCLUDE_DEPTH {
            return Err(CrepusError::render(format!(
                "maximum include depth ({}) exceeded; possible circular include involving '{}'",
                crate::MAX_INCLUDE_DEPTH,
                inc.path
            )));
        }
        self.include_body(inc, ctx, slot, regions)
    }

    fn include_body(
        &mut self,
        inc: &IncludeNode,
        ctx: &TemplateContext,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        if let Some((file_part, comp_name)) = inc.path.split_once('#') {
            return self.named_component(inc, ctx, file_part, comp_name, slot, regions);
        }

        let (file_path, content) = self.read_include(ctx, &inc.path)?;

        let frontend_path =
            crepuscularity_core::include_paths::include_frontend_path(&file_path, &inc.path);
        let nodes =
            crepuscularity_core::ast_cache::parse_content_with_path(&content, Some(&frontend_path))
                .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?;

        let mut child_ctx = TemplateContext::new();
        child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
        child_ctx.virtual_files = ctx.virtual_files.clone();
        for (key, expr) in &inc.props {
            child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
        }
        // Rendering is synchronous: borrow the caller until this include finishes.
        let frame = SlotFrame {
            nodes: &inc.slot,
            context: ctx,
            caller_slot: slot,
            regions,
        };
        let slot = (!inc.slot.is_empty()).then_some(&frame);

        self.depth += 1;
        let regions = self.is_ssr().then(|| classify_regions(&nodes));
        let out = self.nested(&nodes, &child_ctx, slot, regions.as_ref());
        self.depth -= 1;
        out
    }

    fn named_component(
        &mut self,
        inc: &IncludeNode,
        ctx: &TemplateContext,
        file_part: &str,
        comp_name: &str,
        slot: Option<&SlotFrame<'_>>,
        regions: Option<&RegionMap<'_>>,
    ) -> Result<String, CrepusError> {
        if !self.is_ssr() && self.depth >= crate::MAX_INCLUDE_DEPTH {
            return Err(CrepusError::render(format!(
                "maximum include depth ({}) exceeded; possible circular include involving '{file_part}#{comp_name}'",
                crate::MAX_INCLUDE_DEPTH
            )));
        }

        let (file_path, content) = self.read_include(ctx, file_part)?;
        let comp_file = parse_component_file(&content)
            .map_err(|e| CrepusError::render(format!("component file parse error: {e}")))?;
        let comp = comp_file.components.get(comp_name).ok_or_else(|| {
            CrepusError::render(format!(
                "component '{}' not found in {}",
                comp_name, file_part
            ))
        })?;

        let mut child_ctx = TemplateContext::new();
        child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
        child_ctx.virtual_files = ctx.virtual_files.clone();
        for (key, expr) in &comp.meta.defaults {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
        for (key, expr) in &inc.props {
            child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
        }
        // Rendering is synchronous: borrow the caller until this include finishes.
        let frame = SlotFrame {
            nodes: &inc.slot,
            context: ctx,
            caller_slot: slot,
            regions,
        };
        let slot = (!inc.slot.is_empty()).then_some(&frame);

        self.depth += 1;
        let regions = self.is_ssr().then(|| classify_regions(&comp.nodes));
        let out = self.nested(&comp.nodes, &child_ctx, slot, regions.as_ref());
        self.depth -= 1;
        out
    }
}

#[cfg(feature = "hydration")]
pub(crate) fn textarea_content(
    el: &Element,
    ctx: &TemplateContext,
    depth: usize,
) -> Result<Option<String>, CrepusError> {
    textarea_content_with_slot(el, ctx, depth, None, false)
}

fn textarea_content_with_slot(
    el: &Element,
    ctx: &TemplateContext,
    depth: usize,
    slot: Option<&SlotFrame<'_>>,
    virtual_only: bool,
) -> Result<Option<String>, CrepusError> {
    if !el.tag.eq_ignore_ascii_case("textarea") {
        return Ok(None);
    }
    let mut content = if let Some(binding) = el.bindings.iter().find(|binding| {
        binding.prop.eq_ignore_ascii_case("value") && binding.origin != BindingOrigin::Attribute
    }) {
        crate::escape_html(&value_to_str(&eval_expr(&binding.value, ctx)?))
    } else {
        // Textarea content is raw text in HTML; SSR spans would become visible text.
        Walker::plain(depth)
            .with_virtual_only(virtual_only)
            .nodes_with_slot(&el.children, ctx, slot, None)?
    };
    // Character references keep CR intact through HTML newline preprocessing.
    content = content.replace('\r', "&#13;");
    // HTML parsing consumes one newline immediately after the textarea opening tag.
    if content.starts_with('\n') {
        content.insert(0, '\n');
    }
    Ok(Some(content))
}

pub(crate) fn push_bindings(
    out: &mut String,
    el: &Element,
    ctx: &TemplateContext,
    skip_interval: bool,
) -> Result<(), CrepusError> {
    for binding in &el.bindings {
        if el.tag == "slot-rotate"
            && binding.origin == BindingOrigin::Attribute
            && binding.prop.eq_ignore_ascii_case("class")
        {
            continue;
        }
        if el.tag.eq_ignore_ascii_case("textarea")
            && binding.prop.eq_ignore_ascii_case("value")
            && binding.origin != BindingOrigin::Attribute
        {
            continue;
        }
        if skip_interval && binding.prop == "interval" && binding.origin != BindingOrigin::Attribute
        {
            continue;
        }
        let Some(evaluated) = eval_binding(binding, ctx)? else {
            continue;
        };
        if matches!(evaluated, TemplateValue::Bool(false) | TemplateValue::Null)
            && matches!(
                binding.prop.to_ascii_lowercase().as_str(),
                "allowfullscreen"
                    | "async"
                    | "autofocus"
                    | "autoplay"
                    | "checked"
                    | "controls"
                    | "default"
                    | "defer"
                    | "disabled"
                    | "download"
                    | "formnovalidate"
                    | "hidden"
                    | "inert"
                    | "ismap"
                    | "itemscope"
                    | "loop"
                    | "multiple"
                    | "muted"
                    | "nomodule"
                    | "novalidate"
                    | "open"
                    | "playsinline"
                    | "readonly"
                    | "required"
                    | "reversed"
                    | "selected"
            )
        {
            continue;
        }
        let value = if binding.prop.eq_ignore_ascii_case("download")
            && matches!(evaluated, TemplateValue::Bool(true))
        {
            String::new()
        } else {
            value_to_str(&evaluated)
        };
        if crate::is_url_attr(&binding.prop) && !crate::is_safe_url_value(&binding.prop, &value) {
            continue;
        }
        out.push(' ');
        out.push_str(&binding.prop);
        out.push_str("=\"");
        out.push_str(&crate::escape_html_attribute(&value));
        out.push('"');
    }
    Ok(())
}

pub(crate) fn push_handlers_and_animations(
    out: &mut String,
    el: &Element,
) -> Result<(), CrepusError> {
    for (index, handler) in el.event_handlers.iter().enumerate() {
        for modifier in &handler.modifiers {
            if matches!(
                modifier.as_str(),
                "prevent" | "stop" | "immediate" | "trusted" | "self" | "once" | "capture"
            ) {
                continue;
            }
            let lower = modifier.to_lowercase();
            let parts: Vec<_> = lower.split('+').collect();
            if !matches!(
                handler.event.to_ascii_lowercase().as_str(),
                "keydown" | "keyup"
            ) || parts.last().is_none_or(|key| key.is_empty())
                || parts[..parts.len() - 1]
                    .iter()
                    .any(|part| !matches!(*part, "ctrl" | "cmd" | "shift" | "alt" | "opt"))
            {
                return Err(CrepusError::Parse(format!(
                    "Unsupported web event modifier `{modifier}` on `{}`",
                    handler.event
                )));
            }
        }
        if el.event_handlers[..index]
            .iter()
            .any(|previous| previous.event.eq_ignore_ascii_case(&handler.event))
        {
            continue;
        }
        out.push(' ');
        out.push_str("data-on");
        out.push_str(&handler.event);
        out.push_str("=\"");
        out.push_str(&crate::escape_html_attribute(&handler.handler));
        out.push('"');
        let handlers: Vec<_> = el
            .event_handlers
            .iter()
            .filter(|entry| entry.event.eq_ignore_ascii_case(&handler.event))
            .map(|entry| json!({ "handler": entry.handler, "modifiers": entry.modifiers }))
            .collect();
        if handlers.len() > 1 {
            out.push_str(" data-crepus-handlers-");
            out.push_str(&handler.event);
            out.push_str("=\"");
            out.push_str(&crate::escape_html_attribute(
                &serde_json::to_string(&handlers).expect("event handlers serialize"),
            ));
            out.push('"');
        }
        if !handler.modifiers.is_empty() {
            out.push_str(" data-crepus-modifiers-");
            out.push_str(&handler.event);
            out.push_str("=\"");
            let modifiers = serde_json::to_string(&handler.modifiers)
                .expect("event modifiers contain only strings");
            out.push_str(&crate::escape_html_attribute(&modifiers));
            out.push('"');
        }
    }

    for animation in &el.animations {
        out.push(' ');
        out.push_str("data-animate-");
        out.push_str(&animation.property);
        out.push_str("=\"");
        out.push_str(&crate::escape_html_attribute(&format!(
            "{} {}",
            animation.duration_expr, animation.easing
        )));
        if animation.repeat {
            out.push_str(" repeat");
        }
        out.push('"');
    }
    Ok(())
}

fn text_parts_manifest(parts: &[TextPart]) -> Value {
    let arr: Vec<Value> = parts
        .iter()
        .map(|p| match p {
            TextPart::Literal(s) => json!({"t": "lit", "v": s}),
            TextPart::Expr(e) => json!({"t": "expr", "v": e}),
        })
        .collect();
    Value::Array(arr)
}
