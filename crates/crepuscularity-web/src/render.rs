//! The single AST walker behind both the plain HTML renderer and the SSR renderer.
//!
//! [`Walker`] is parameterised by an emit mode: in plain mode it writes bare HTML and
//! tracks include nesting through a threaded depth counter; in SSR mode it additionally
//! allocates `data-crepus-id` bindings, wraps dynamic regions in `display:contents`
//! markers, and defers include-depth accounting to [`crate::ssr::IncludeDepthGuard`].

use std::cell::Cell;
use std::sync::Arc;

use crepuscularity_core::analysis::{classify_node, Region};
use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::parser::parse_component_file;
use crepuscularity_core::preprocess::{slot_rotate_child_phrases, slot_rotate_words_json_attr};
use crepuscularity_core::CrepusError;
use serde_json::{json, Value};

pub type BindMap = serde_json::Map<String, Value>;

pub(crate) struct SsrState<'a> {
    counter: &'a Cell<u32>,
    bind: &'a mut BindMap,
    root_pending: bool,
}

pub(crate) struct Walker<'a> {
    ssr: Option<SsrState<'a>>,
    depth: usize,
}

impl<'a> Walker<'a> {
    pub(crate) fn plain(depth: usize) -> Self {
        Self { ssr: None, depth }
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
        }
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

    /// Wrap `inner` in the hydration marker span when an id was allocated.
    fn finish(kind: &str, id: Option<u32>, inner: String) -> String {
        match id {
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
            html.push_str(&self.node(node, cur)?);
        }

        Ok(html)
    }

    /// Render a nested node list that can never carry the document root marker.
    fn nested(&mut self, nodes: &[Node], ctx: &TemplateContext) -> Result<String, CrepusError> {
        let saved = self.set_root_pending(false);
        let out = self.nodes(nodes, ctx);
        self.set_root_pending(saved);
        out
    }

    pub(crate) fn node(
        &mut self,
        node: &Node,
        ctx: &TemplateContext,
    ) -> Result<String, CrepusError> {
        match node {
            Node::Element(el) => self.element(el, ctx),
            Node::Text(parts) => {
                let id = if self.is_ssr() && classify_node(node) == Region::Dynamic {
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
                Ok(Self::finish("text", id, inner))
            }
            Node::If(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "if",
                        "condition": block.condition,
                    })
                });
                let inner = if ctx.eval_condition(&block.condition)? {
                    self.nested(&block.then_children, ctx)?
                } else if let Some(else_children) = &block.else_children {
                    self.nested(else_children, ctx)?
                } else {
                    String::new()
                };
                Ok(Self::finish("if", id, inner))
            }
            Node::For(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "for",
                        "pattern": block.pattern,
                        "iterator": block.iterator,
                    })
                });
                let inner = self.for_body(block, ctx)?;
                Ok(Self::finish("for", id, inner))
            }
            Node::Match(block) => {
                let id = self.alloc(|| {
                    json!({
                        "kind": "match",
                        "expr": block.expr,
                    })
                });
                let inner = self.match_body(block, ctx)?;
                Ok(Self::finish("match", id, inner))
            }
            Node::LetDecl(_) => Ok(String::new()),
            Node::Include(inc) => self.include(inc, ctx),
            Node::Embed(embed) => crate::render_embed(embed, ctx),
            Node::RawText(expr) => {
                let id = self.alloc(|| json!({ "kind": "raw", "expr": expr }));
                let inner = crate::escape_html(&value_to_str(&eval_expr(expr, ctx)?));
                Ok(Self::finish("raw", id, inner))
            }
            Node::RawHtml(expr) => {
                let id = self.alloc(|| json!({ "kind": "raw", "expr": expr }));
                let inner = ammonia::clean(&value_to_str(&eval_expr(expr, ctx)?));
                Ok(Self::finish("raw", id, inner))
            }
        }
    }

    fn for_body(&mut self, block: &ForBlock, ctx: &TemplateContext) -> Result<String, CrepusError> {
        let items = ctx.get_list_ref(&block.iterator);
        let mut out = String::new();
        let pattern = block.pattern.trim();
        let has_pattern = !pattern.is_empty();
        let mut child_ctx = ctx.clone();

        for item_ctx in items {
            child_ctx.vars.clone_from(&ctx.vars);
            for (k, v) in &item_ctx.vars {
                child_ctx.vars.insert(k.clone(), v.clone());
            }
            if has_pattern {
                let item_str = item_ctx.get_str("value");
                if !item_str.is_empty() {
                    child_ctx
                        .vars
                        .insert(pattern.to_string(), TemplateValue::Str(item_str));
                } else {
                    child_ctx
                        .vars
                        .insert(pattern.to_string(), TemplateValue::Scope(item_ctx.clone()));
                }
            }
            out.push_str(&self.nested(&block.body, &child_ctx)?);
        }

        Ok(out)
    }

    fn match_body(
        &mut self,
        block: &MatchBlock,
        ctx: &TemplateContext,
    ) -> Result<String, CrepusError> {
        let val = eval_expr(&block.expr, ctx)?;
        let value = value_to_str(&val);

        for arm in &block.arms {
            let pattern = arm.pattern.trim();
            if pattern == "_" {
                return self.nested(&arm.body, ctx);
            }
            if pattern.starts_with('"') && pattern.ends_with('"') {
                let lit = &pattern[1..pattern.len() - 1];
                if value == lit {
                    return self.nested(&arm.body, ctx);
                }
            }
            if value == pattern {
                return self.nested(&arm.body, ctx);
            }
        }

        Ok(String::new())
    }

    fn element(&mut self, el: &Element, ctx: &TemplateContext) -> Result<String, CrepusError> {
        if el.tag == "slot" {
            return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
                self.nested(slot_nodes, slot_ctx)
            } else {
                self.nested(&el.children, ctx)
            };
        }

        if el.tag == "slot-rotate" {
            return self.slot_rotate(el, ctx);
        }

        let inject_id =
            self.is_ssr() && classify_node(&Node::Element(el.clone())) == Region::Dynamic;
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
            out.push_str(&crate::escape_html(dom_id));
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
        push_handlers_and_animations(&mut out, el);

        if crate::void_html::is_void_html_tag(&el.tag) {
            out.push_str(" />");
            return Ok(out);
        }

        out.push('>');
        for child in &el.children {
            out.push_str(&self.node(child, ctx)?);
        }
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
            if b.prop == "interval" {
                let v = value_to_str(&eval_expr(&b.value, ctx)?);
                let v = v.trim_matches('"').trim();
                interval_ms = v.parse().unwrap_or(3200);
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
            out.push_str(&crate::escape_html(dom_id));
            out.push('"');
        }
        if let Some(id) = id_opt {
            out.push_str(&format!(r#" data-crepus-id="c{id}""#));
            out.push_str(" data-crepus-kind=\"slot-rotate\"");
        }
        if let Some(class_attr) =
            crate::build_class_attr(&class_names, &el.conditional_classes, ctx)?
        {
            out.push_str(" class=\"");
            out.push_str(&class_attr);
            out.push('"');
        }
        out.push_str(" data-slot-words=\"");
        out.push_str(&crate::escape_html(&words_json));
        out.push('"');
        out.push_str(" data-slot-interval=\"");
        out.push_str(&crate::escape_html(&interval_ms.to_string()));
        out.push('"');
        out.push_str(" aria-live=\"polite\"");

        push_bindings(&mut out, el, ctx, true)?;
        push_handlers_and_animations(&mut out, el);

        out.push_str("></span>");
        Ok(out)
    }

    fn include(&mut self, inc: &IncludeNode, ctx: &TemplateContext) -> Result<String, CrepusError> {
        #[cfg(feature = "ssr")]
        if self.is_ssr() {
            let _depth_guard = crate::ssr::IncludeDepthGuard::enter(&inc.path)?;
            let id = self.alloc(|| json!({ "kind": "include", "path": inc.path }));
            let inner = self.include_body(inc, ctx)?;
            return Ok(Self::finish("include", id, inner));
        }

        if self.depth >= crate::MAX_INCLUDE_DEPTH {
            return Err(CrepusError::render(format!(
                "maximum include depth ({}) exceeded; possible circular include involving '{}'",
                crate::MAX_INCLUDE_DEPTH,
                inc.path
            )));
        }
        self.include_body(inc, ctx)
    }

    fn include_body(
        &mut self,
        inc: &IncludeNode,
        ctx: &TemplateContext,
    ) -> Result<String, CrepusError> {
        if let Some((file_part, comp_name)) = inc.path.split_once('#') {
            return self.named_component(inc, ctx, file_part, comp_name);
        }

        let file_path = crate::resolve_include_path(ctx.base_dir.as_deref(), &inc.path)?;
        let content = crate::read_file(ctx, &file_path)?;

        // The SSR path shares the process-wide AST cache; the plain path keeps the source
        // path so parse diagnostics stay attributable to the included file.
        let cached;
        let owned;
        let nodes: &[Node] = if self.is_ssr() {
            cached = crepuscularity_core::ast_cache::parse_content(&content)
                .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?;
            &cached
        } else {
            owned =
                crepuscularity_core::parser::parse_template_with_path(&content, Some(&file_path))
                    .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?;
            &owned
        };

        let mut child_ctx = TemplateContext::new();
        child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
        child_ctx.virtual_files = ctx.virtual_files.clone();
        for (key, expr) in &inc.props {
            child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
        }
        if !inc.slot.is_empty() {
            child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
        }

        self.depth += 1;
        let out = self.nested(nodes, &child_ctx);
        self.depth -= 1;
        out
    }

    fn named_component(
        &mut self,
        inc: &IncludeNode,
        ctx: &TemplateContext,
        file_part: &str,
        comp_name: &str,
    ) -> Result<String, CrepusError> {
        if !self.is_ssr() && self.depth >= crate::MAX_INCLUDE_DEPTH {
            return Err(CrepusError::render(format!(
                "maximum include depth ({}) exceeded; possible circular include involving '{file_part}#{comp_name}'",
                crate::MAX_INCLUDE_DEPTH
            )));
        }

        let file_path = crate::resolve_include_path(ctx.base_dir.as_deref(), file_part)?;
        let content = crate::read_file(ctx, &file_path)?;
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
        if !inc.slot.is_empty() {
            child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
        }

        self.depth += 1;
        let out = self.nested(&comp.nodes, &child_ctx);
        self.depth -= 1;
        out
    }
}

fn push_bindings(
    out: &mut String,
    el: &Element,
    ctx: &TemplateContext,
    skip_interval: bool,
) -> Result<(), CrepusError> {
    for binding in &el.bindings {
        if skip_interval && binding.prop == "interval" {
            continue;
        }
        let evaluated = eval_expr(&binding.value, ctx)?;
        let value = match evaluated {
            TemplateValue::Bool(false)
                if is_boolean_html_attribute(&binding.prop)
                    || binding.prop.eq_ignore_ascii_case("hidden") =>
            {
                continue
            }
            TemplateValue::Bool(true) if binding.prop.eq_ignore_ascii_case("hidden") => {
                "hidden".to_string()
            }
            TemplateValue::Bool(true) if is_boolean_html_attribute(&binding.prop) => String::new(),
            value => value_to_str(&value),
        };
        if crate::is_url_attr(&binding.prop) && !crate::is_safe_url_value(&binding.prop, &value) {
            continue;
        }
        out.push(' ');
        out.push_str(&binding.prop);
        out.push_str("=\"");
        out.push_str(&crate::escape_html(&value));
        out.push('"');
    }
    Ok(())
}

fn is_boolean_html_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "allowfullscreen"
            | "async"
            | "autofocus"
            | "autoplay"
            | "checked"
            | "controls"
            | "default"
            | "defer"
            | "disabled"
            | "disablepictureinpicture"
            | "disableremoteplayback"
            | "formnovalidate"
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
}

fn push_handlers_and_animations(out: &mut String, el: &Element) {
    for handler in &el.event_handlers {
        out.push(' ');
        out.push_str("data-on");
        out.push_str(&handler.event);
        out.push_str("=\"");
        out.push_str(&crate::escape_html(&handler.handler));
        out.push('"');
    }

    for animation in &el.animations {
        out.push(' ');
        out.push_str("data-animate-");
        out.push_str(&animation.property);
        out.push_str("=\"");
        out.push_str(&crate::escape_html(&format!(
            "{} {}",
            animation.duration_expr, animation.easing
        )));
        out.push('"');
    }
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
