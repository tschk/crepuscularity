/// Direct calls synchronously read and parse includes, caching only for that call.
/// HotReloadView supplies immutable worker-prepared includes instead.
///
/// Runtime GPUI renderer — walks the AST and builds GPUI elements dynamically.
///
/// Supports:
/// - Dynamic theme colors via context expressions in class values
/// - GPUI animations via `animate:property={duration easing}` attributes
/// - All standard Tailwind-like classes mapped to GPUI methods
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    bounce, div, ease_in_out, ease_out_quint, linear, quadratic, rgb, Animation, AnimationExt,
    AnyElement, ElementId, IntoElement, ParentElement, SharedString, Styled,
};

use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::preprocess::slot_rotate_child_phrases;
use crepuscularity_core::virtual_files::lookup_virtual_file;

mod prepared;
pub(crate) use prepared::{prepare_includes, PreparedIncludes};

#[derive(Default)]
struct IncludeCache {
    bundles: Vec<BundleCache>,
    prepared: Option<Arc<PreparedIncludes>>,
}

type IncludeIdentity = (std::path::PathBuf, std::path::PathBuf);

struct BundleCache {
    virtual_files: Arc<std::collections::HashMap<String, String>>,
    nodes: std::collections::HashMap<IncludeIdentity, Result<Arc<Vec<Node>>, String>>,
    sources: std::collections::HashMap<std::path::PathBuf, Arc<str>>,
    resolutions: std::collections::HashMap<
        (Option<std::path::PathBuf>, String),
        Result<std::path::PathBuf, String>,
    >,
    components: std::collections::HashMap<
        std::path::PathBuf,
        Result<Arc<crepuscularity_core::parser::ComponentFile>, String>,
    >,
}

impl IncludeCache {
    fn bundle(&mut self, ctx: &TemplateContext) -> &mut BundleCache {
        let index = self
            .bundles
            .iter()
            .position(|bundle| Arc::ptr_eq(&bundle.virtual_files, &ctx.virtual_files))
            .unwrap_or_else(|| {
                self.bundles.push(BundleCache {
                    virtual_files: Arc::clone(&ctx.virtual_files),
                    nodes: Default::default(),
                    sources: Default::default(),
                    resolutions: Default::default(),
                    components: Default::default(),
                });
                self.bundles.len() - 1
            });
        &mut self.bundles[index]
    }

    fn resolve(
        &mut self,
        ctx: &TemplateContext,
        literal: &str,
    ) -> Result<std::path::PathBuf, String> {
        let key = (ctx.base_dir.clone(), literal.to_owned());
        if let Some(prepared) = &self.prepared {
            return prepared
                .bundle(ctx)
                .and_then(|bundle| bundle.resolutions.get(&key))
                .cloned()
                .unwrap_or_else(|| Err("include was not prepared for this context".into()));
        }
        if let Some(result) = self.bundle(ctx).resolutions.get(&key) {
            return result.clone();
        }
        let path = resolve_include_path(ctx.base_dir.as_deref(), literal)
            .map_err(|error| error.to_string())?;
        self.bundle(ctx).resolutions.insert(key, Ok(path.clone()));
        Ok(path)
    }

    fn source(
        &mut self,
        ctx: &TemplateContext,
        path: &std::path::Path,
    ) -> Result<Arc<str>, String> {
        if let Some(source) = self.bundle(ctx).sources.get(path) {
            return Ok(Arc::clone(source));
        }
        let source: Arc<str> = lookup_virtual_file(ctx, path)
            .map(Ok)
            .unwrap_or_else(|| std::fs::read_to_string(path))
            .map_err(|error| format!("include error: {:?}: {}", path, error))?
            .into();
        Ok(source)
    }

    #[cfg(test)]
    fn nodes(
        &mut self,
        ctx: &TemplateContext,
        path: &std::path::Path,
    ) -> Result<Arc<Vec<Node>>, String> {
        self.nodes_with_frontend(ctx, path, path)
    }

    fn nodes_with_frontend(
        &mut self,
        ctx: &TemplateContext,
        path: &std::path::Path,
        frontend: &std::path::Path,
    ) -> Result<Arc<Vec<Node>>, String> {
        let key = (path.to_path_buf(), frontend.to_path_buf());
        if let Some(prepared) = &self.prepared {
            return prepared
                .bundle(ctx)
                .and_then(|bundle| bundle.nodes.get(&key))
                .cloned()
                .unwrap_or_else(|| Err("include was not prepared for this context".into()));
        }
        if let Some(result) = self.bundle(ctx).nodes.get(&key) {
            return result.clone();
        }
        let source = self.source(ctx, path)?;
        let nodes = if lookup_virtual_file(ctx, path).is_some() {
            // Virtual ASTs belong to this render's retained bundle snapshot.
            crepuscularity_core::parse_template_with_path(&source, Some(frontend)).map(Arc::new)
        } else {
            crepuscularity_core::ast_cache::parse_content_with_path(&source, Some(frontend))
        }
        .map_err(|error| error.to_string())?;
        self.bundle(ctx).sources.insert(path.to_path_buf(), source);
        self.bundle(ctx).nodes.insert(key, Ok(Arc::clone(&nodes)));
        Ok(nodes)
    }

    fn components(
        &mut self,
        ctx: &TemplateContext,
        path: &std::path::Path,
    ) -> Result<Arc<crepuscularity_core::parser::ComponentFile>, String> {
        if let Some(prepared) = &self.prepared {
            return prepared
                .bundle(ctx)
                .and_then(|bundle| bundle.components.get(path))
                .cloned()
                .unwrap_or_else(|| Err("include was not prepared for this context".into()));
        }
        if let Some(result) = self.bundle(ctx).components.get(path) {
            return result.clone();
        }
        let content = self.source(ctx, path)?;
        let file = Arc::new(
            crepuscularity_core::parser::parse_component_file(&content)
                .map_err(|error| format!("component file parse error: {error}"))?,
        );
        self.bundle(ctx).sources.insert(path.to_path_buf(), content);
        self.bundle(ctx)
            .components
            .insert(path.to_path_buf(), Ok(Arc::clone(&file)));
        Ok(file)
    }
}

pub(crate) fn render_nodes_prepared(
    nodes: &[Node],
    ctx: &TemplateContext,
    prepared: Arc<PreparedIncludes>,
) -> AnyElement {
    render_nodes_cached(
        nodes,
        ctx,
        &mut IncludeCache {
            prepared: Some(prepared),
            ..Default::default()
        },
    )
}

use crate::styler::{apply_class_with_ctx, parse_duration_ms};
use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};

fn eval_expr_value(expr: &str, ctx: &TemplateContext) -> TemplateValue {
    crepuscularity_core::eval::eval_expr(expr, ctx).unwrap_or(TemplateValue::Null)
}

fn eval_condition_bool(ctx: &TemplateContext, expr: &str) -> bool {
    ctx.eval_condition(expr).unwrap_or(false)
}

/// Render a list of nodes into a single `AnyElement`, threading `LetDecl`s into
/// a running context clone so later siblings see the declared variables.
pub fn render_nodes(nodes: &[Node], ctx: &TemplateContext) -> AnyElement {
    render_nodes_cached(nodes, ctx, &mut IncludeCache::default())
}

fn render_nodes_cached(
    nodes: &[Node],
    ctx: &TemplateContext,
    cache: &mut IncludeCache,
) -> AnyElement {
    let mut ctx = ctx.clone();
    let mut rendered: Vec<AnyElement> = Vec::new();

    for node in nodes {
        if let Node::LetDecl(decl) = node {
            if decl.is_default && ctx.vars.contains_key(&decl.name) {
                // Default: skip if already set by parent props
            } else {
                let val = eval_expr_value(&decl.expr, &ctx);
                ctx.vars.insert(decl.name.clone(), val);
            }
        } else {
            rendered.push(render_node_cached(node, &ctx, cache));
        }
    }

    match rendered.len() {
        0 => div().into_any_element(),
        1 => rendered.remove(0),
        _ => {
            let mut d = div();
            for child in rendered {
                d = d.child(child);
            }
            d.into_any_element()
        }
    }
}

pub fn render_node(node: &Node, ctx: &TemplateContext) -> AnyElement {
    render_node_cached(node, ctx, &mut IncludeCache::default())
}

fn render_node_cached(node: &Node, ctx: &TemplateContext, cache: &mut IncludeCache) -> AnyElement {
    match node {
        Node::Element(el) => render_element(el, ctx, cache),
        Node::Text(parts) => {
            let text = render_text(parts, ctx);
            div().child(SharedString::from(text)).into_any_element()
        }
        Node::If(block) => render_if(block, ctx, cache),
        Node::For(block) => render_for(block, ctx, cache),
        Node::Match(block) => render_match(block, ctx, cache),
        Node::LetDecl(_) => div().into_any_element(), // handled in render_nodes_with_ctx
        Node::RawText(expr) => {
            let val = eval_expr_value(expr, ctx);
            div()
                .child(SharedString::from(value_to_str(&val)))
                .into_any_element()
        }
        Node::RawHtml(expr) => {
            let val = eval_expr_value(expr, ctx);
            div()
                .child(SharedString::from(value_to_str(&val)))
                .into_any_element()
        }
        Node::Include(inc) => render_include(inc, ctx, cache),
        Node::Embed(_) => div().into_any_element(),
    }
}

fn render_element(el: &Element, ctx: &TemplateContext, cache: &mut IncludeCache) -> AnyElement {
    // Intercept the `slot` pseudo-tag: render slot content from parent, or fallback children.
    if el.tag == "slot" {
        return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
            render_nodes_cached(slot_nodes, slot_ctx, cache)
        } else {
            render_nodes_cached(&el.children, ctx, cache)
        };
    }

    // Web-only rotating text: GPUI shows the first phrase as a static preview.
    if el.tag == "slot-rotate" {
        let label = slot_rotate_child_phrases(&el.children)
            .ok()
            .and_then(|p| p.into_iter().next())
            .unwrap_or_default();
        let mut d = div();
        for class in &el.classes {
            d = apply_class_with_ctx(d, class, Some(ctx));
        }
        for cc in &el.conditional_classes {
            if eval_condition_bool(ctx, &cc.condition) {
                d = apply_class_with_ctx(d, &cc.class, Some(ctx));
            }
        }
        return d.child(SharedString::from(label)).into_any_element();
    }

    let mut d = base_tag_element(&el.tag);

    // Apply static and dynamic classes with context for expression resolution
    for class in &el.classes {
        d = apply_class_with_ctx(d, class, Some(ctx));
    }

    // Apply conditional classes
    for cc in &el.conditional_classes {
        if eval_condition_bool(ctx, &cc.condition) {
            d = apply_class_with_ctx(d, &cc.class, Some(ctx));
        }
    }

    // Render children
    for child in &el.children {
        let child_el = render_node_cached(child, ctx, cache);
        d = d.child(child_el);
    }

    // If animations are present, wrap with GPUI animation
    if !el.animations.is_empty() {
        return render_with_animations(d, &el.animations, &el.tag);
    }

    d.into_any_element()
}

/// Wrap a div with GPUI animations based on the parsed animation specs.
fn render_with_animations(d: gpui::Div, animations: &[AnimationSpec], tag: &str) -> AnyElement {
    // Generate a stable element ID from the tag + animation properties
    let props: Vec<&str> = animations.iter().map(|a| a.property.as_str()).collect();
    let id_str = format!("crepus-anim-{}-{}", tag, props.join("-"));
    let id = ElementId::Name(SharedString::from(id_str));

    if animations.len() == 1 {
        let spec = &animations[0];
        let duration_ms = parse_duration_ms(&spec.duration_expr).unwrap_or(300);
        let duration = Duration::from_millis(duration_ms);

        let mut anim = Animation::new(duration);
        anim = apply_easing(anim, &spec.easing);
        if spec.repeat {
            anim = anim.repeat();
        }

        let property = spec.property.clone();
        d.with_animation(id, anim, move |el, delta| {
            apply_animation_property(el, &property, delta)
        })
        .into_any_element()
    } else {
        let anims: Vec<Animation> = animations
            .iter()
            .map(|spec| {
                let duration_ms = parse_duration_ms(&spec.duration_expr).unwrap_or(300);
                let mut anim = Animation::new(Duration::from_millis(duration_ms));
                anim = apply_easing(anim, &spec.easing);
                if spec.repeat {
                    anim = anim.repeat();
                }
                anim
            })
            .collect();

        let properties: Vec<String> = animations.iter().map(|a| a.property.clone()).collect();
        d.with_animations(id, anims, move |el, ix, delta| {
            if ix < properties.len() {
                apply_animation_property(el, &properties[ix], delta)
            } else {
                el
            }
        })
        .into_any_element()
    }
}

fn apply_easing(anim: Animation, easing: &str) -> Animation {
    match easing {
        "linear" => anim.with_easing(linear),
        "ease-in-out" => anim.with_easing(ease_in_out),
        "quadratic" => anim.with_easing(quadratic),
        "bounce" => anim.with_easing(bounce(quadratic)),
        "ease-out" => anim.with_easing(ease_out_quint()),
        _ => anim, // default: linear
    }
}

/// Apply an animation delta (0.0 - 1.0) to a specific property on a div.
fn apply_animation_property(d: gpui::Div, property: &str, delta: f32) -> gpui::Div {
    match property {
        "opacity" | "fade" | "fade-in" => d.opacity(delta),
        "fade-out" => d.opacity(1.0 - delta),
        "pulse" => d.opacity(0.4 + delta * 0.6),
        "scale" => {
            // Scale from 0.8 to 1.0
            let scale = 0.8 + delta * 0.2;
            d.opacity(scale) // GPUI doesn't have transform: scale; use opacity as approximation
        }
        "slide-down" => {
            // Slide from -10px to 0px
            let offset = gpui::px(-10.0 * (1.0 - delta));
            d.mt(offset)
        }
        "slide-up" => {
            let offset = gpui::px(10.0 * (1.0 - delta));
            d.mt(offset)
        }
        "slide-right" => {
            let offset = gpui::px(-20.0 * (1.0 - delta));
            d.ml(offset)
        }
        "slide-left" => {
            let offset = gpui::px(20.0 * (1.0 - delta));
            d.ml(offset)
        }
        "grow" => {
            // Grow from w-0 to full
            let pct = gpui::relative(delta);
            d.w(pct)
        }
        _ => d,
    }
}

fn base_tag_element(tag: &str) -> gpui::Div {
    match tag {
        "button" => div().cursor_pointer(),
        _ => div(),
    }
}

fn render_text(parts: &[TextPart], ctx: &TemplateContext) -> String {
    let mut result = String::new();
    for part in parts {
        match part {
            TextPart::Literal(text) => result.push_str(text),
            TextPart::Expr(expr) => {
                let val = eval_expr_value(expr, ctx);
                result.push_str(&value_to_str(&val));
            }
        }
    }
    result
}

fn render_if(block: &IfBlock, ctx: &TemplateContext, cache: &mut IncludeCache) -> AnyElement {
    if eval_condition_bool(ctx, &block.condition) {
        render_nodes_cached(&block.then_children, ctx, cache)
    } else if let Some(else_children) = &block.else_children {
        render_nodes_cached(else_children, ctx, cache)
    } else {
        div().into_any_element()
    }
}

fn render_for(block: &ForBlock, ctx: &TemplateContext, cache: &mut IncludeCache) -> AnyElement {
    let items = ctx.get_list_ref(&block.iterator);

    let mut d = div();
    let mut child_ctx = ctx.clone();
    let pattern = block.pattern.trim();
    let has_pattern = !pattern.is_empty();
    for item_ctx in items {
        let item_str = if has_pattern {
            item_ctx.get_str("value")
        } else {
            String::new()
        };
        child_ctx.vars.clone_from(&ctx.vars);
        for (k, v) in &item_ctx.vars {
            child_ctx.vars.insert(k.clone(), v.clone());
        }
        if has_pattern && !item_str.is_empty() {
            child_ctx
                .vars
                .insert(pattern.to_string(), TemplateValue::Str(item_str));
        }

        let child = render_nodes_cached(&block.body, &child_ctx, cache);
        d = d.child(child);
    }
    d.into_any_element()
}

fn render_match(block: &MatchBlock, ctx: &TemplateContext, cache: &mut IncludeCache) -> AnyElement {
    let val = eval_expr_value(&block.expr, ctx);
    let value = value_to_str(&val);

    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        if pattern == "_" {
            return render_nodes_cached(&arm.body, ctx, cache);
        }
        if pattern.starts_with('"') && pattern.ends_with('"') {
            let lit = &pattern[1..pattern.len() - 1];
            if value == lit {
                return render_nodes_cached(&arm.body, ctx, cache);
            }
        }
        if value == pattern {
            return render_nodes_cached(&arm.body, ctx, cache);
        }
    }

    div().into_any_element()
}

fn render_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    cache: &mut IncludeCache,
) -> AnyElement {
    stacker::maybe_grow(512 * 1024, 2 * 1024 * 1024, || {
        render_include_inner(inc, ctx, cache)
    })
}

fn render_include_inner(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    cache: &mut IncludeCache,
) -> AnyElement {
    let Some(_guard) = IncludeGuard::enter() else {
        return div()
            .text_color(rgb(0xff4444))
            .child("maximum include depth (64) exceeded; possible circular include")
            .into_any_element();
    };
    // Multi-component syntax: "path/file.crepus#ComponentName"
    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return render_named_component(inc, ctx, file_part, comp_name, cache);
    }

    // Single-component file: resolve path relative to the current file's directory.
    let file_path = match cache.resolve(ctx, &inc.path) {
        Ok(path) => path,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    let frontend = crepuscularity_core::include_paths::include_frontend_path(&file_path, &inc.path);
    let nodes = match cache.nodes_with_frontend(ctx, &file_path, &frontend) {
        Ok(n) => n,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    // Build child context: fresh vars from evaluated props, correct base_dir, and slot.
    let mut child_ctx = include_context(ctx, &file_path);

    apply_include_props_and_slot(inc, ctx, &mut child_ctx);

    render_nodes_cached(nodes.as_ref(), &child_ctx, cache)
}

/// Render a named component from a multi-component file (`path#Name` syntax).
fn render_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
    cache: &mut IncludeCache,
) -> AnyElement {
    let file_path = match cache.resolve(ctx, file_part) {
        Ok(path) => path,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    let comp_file = match cache.components(ctx, &file_path) {
        Ok(file) => file,
        Err(error) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(error))
                .into_any_element()
        }
    };

    let comp = match comp_file.components.get(comp_name) {
        Some(c) => c,
        None => {
            let mut keys: Vec<&str> = comp_file.components.keys().map(|s| s.as_str()).collect();
            keys.sort();
            let msg = format!(
                "component '{}' not found in {}; available: [{}]",
                comp_name,
                file_part,
                keys.join(", ")
            );
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(msg))
                .into_any_element();
        }
    };

    let mut child_ctx = include_context(ctx, &file_path);

    // Inject TOML defaults first — passed props override them.
    for (key, expr) in &comp.meta.defaults {
        let val = eval_expr_value(expr, &TemplateContext::new());
        child_ctx.vars.insert(key.clone(), val);
    }

    // Apply passed props.
    apply_include_props_and_slot(inc, ctx, &mut child_ctx);

    render_nodes_cached(&comp.nodes, &child_ctx, cache)
}

fn include_context(ctx: &TemplateContext, path: &std::path::Path) -> TemplateContext {
    let mut child = TemplateContext::new();
    child.base_dir = path.parent().map(|path| path.to_path_buf());
    child.virtual_files = Arc::clone(&ctx.virtual_files);
    child
}

thread_local! {
    static INCLUDE_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct IncludeGuard(std::marker::PhantomData<std::rc::Rc<()>>);

impl IncludeGuard {
    fn enter() -> Option<Self> {
        INCLUDE_DEPTH.with(|depth| {
            if depth.get() >= 64 {
                return None;
            }
            depth.set(depth.get() + 1);
            Some(Self(std::marker::PhantomData))
        })
    }
}

impl Drop for IncludeGuard {
    fn drop(&mut self) {
        INCLUDE_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

// Evaluate each occurrence against its caller; only source ASTs are shared.
fn apply_include_props_and_slot(
    inc: &IncludeNode,
    caller: &TemplateContext,
    child: &mut TemplateContext,
) {
    for (key, expression) in &inc.props {
        child
            .vars
            .insert(key.clone(), eval_expr_value(expression, caller));
    }
    if !inc.slot.is_empty() {
        child.slot = Some((inc.slot.clone(), Arc::new(caller.clone())));
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn cached_source_does_not_capture_props_defaults_or_slot_scopes() {
        let mut caller = TemplateContext::new();
        caller.base_dir = Some(std::path::PathBuf::from("/scope-root"));
        Arc::make_mut(&mut caller.virtual_files).insert("child.crepus".into(), "slot".into());
        let nodes = crepuscularity_core::parse_template(
            "include child.crepus title={value}\n  text \"{value}\"",
        )
        .unwrap();
        let Node::Include(include) = &nodes[0] else {
            panic!("expected include")
        };
        let prepared = prepare_includes(&nodes, &caller);
        let mut cache = IncludeCache {
            prepared: Some(prepared),
            ..Default::default()
        };
        let path = cache.resolve(&caller, "child.crepus").unwrap();
        let source = cache.nodes(&caller, &path).unwrap();
        for value in ["first", "second"] {
            caller.set("value", value);
            let mut child = include_context(&caller, &path);
            child.set("title", "default");
            child.set("default_only", "retained");
            apply_include_props_and_slot(include, &caller, &mut child);
            assert_eq!(child.get_str("title"), value);
            assert_eq!(child.get_str("default_only"), "retained");
            assert!(child.get("value").is_none());
            let (_, slot) = child.slot.as_ref().unwrap();
            assert_eq!(slot.get_str("value"), value);
            assert_eq!(slot.base_dir, caller.base_dir);
            assert!(Arc::ptr_eq(&slot.virtual_files, &caller.virtual_files));
            assert!(Arc::ptr_eq(&source, &cache.nodes(&caller, &path).unwrap()));
        }
    }
}
