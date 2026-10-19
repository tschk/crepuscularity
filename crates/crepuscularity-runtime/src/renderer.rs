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
    render_nodes_with_ctx(nodes, ctx.clone())
}

fn render_nodes_with_ctx(nodes: &[Node], mut ctx: TemplateContext) -> AnyElement {
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
            rendered.push(render_node(node, &ctx));
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
    match node {
        Node::Element(el) => render_element(el, ctx),
        Node::Text(parts) => {
            let text = render_text(parts, ctx);
            div().child(SharedString::from(text)).into_any_element()
        }
        Node::If(block) => render_if(block, ctx),
        Node::For(block) => render_for(block, ctx),
        Node::Match(block) => render_match(block, ctx),
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
        Node::Include(inc) => render_include(inc, ctx),
        Node::Embed(_) => div().into_any_element(),
    }
}

fn render_element(el: &Element, ctx: &TemplateContext) -> AnyElement {
    // Intercept the `slot` pseudo-tag: render slot content from parent, or fallback children.
    if el.tag == "slot" {
        return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
            render_nodes(slot_nodes, slot_ctx)
        } else {
            render_nodes(&el.children, ctx)
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
        let child_el = render_node(child, ctx);
        d = d.child(child_el);
    }

    // If animations are present, wrap with GPUI animation
    if !el.animations.is_empty() {
        return render_with_animations(d, &el.animations, &el.tag);
    }

    d.into_any_element()
}

/// Wrap a div with GPUI animations based on the parsed animation specs.
fn render_with_animations<E: Styled + IntoElement + 'static>(
    d: E,
    animations: &[AnimationSpec],
    path: &str,
    explicit_id: Option<SharedString>,
) -> AnyElement {
    // shortcut: positional IDs change on sibling reorder; use explicit IDs for reorderable nodes.
    let id = ElementId::Name(animation_state_id(path, explicit_id, animations));

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
fn apply_animation_property<E: Styled>(d: E, property: &str, delta: f32) -> E {
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

fn render_if(block: &IfBlock, ctx: &TemplateContext) -> AnyElement {
    if eval_condition_bool(ctx, &block.condition) {
        render_nodes(&block.then_children, ctx)
    } else if let Some(else_children) = &block.else_children {
        render_nodes(else_children, ctx)
    } else {
        div().into_any_element()
    }
}

fn render_for(block: &ForBlock, ctx: &TemplateContext) -> AnyElement {
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

        let child = render_nodes(&block.body, &child_ctx);
        d = d.child(child);
    }
    d.into_any_element()
}

fn render_match(block: &MatchBlock, ctx: &TemplateContext) -> AnyElement {
    let val = eval_expr_value(&block.expr, ctx);
    let value = value_to_str(&val);

    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        if pattern == "_" {
            return render_nodes(&arm.body, ctx);
        }
        if pattern.starts_with('"') && pattern.ends_with('"') {
            let lit = &pattern[1..pattern.len() - 1];
            if value == lit {
                return render_nodes(&arm.body, ctx);
            }
        }
        if value == pattern {
            return render_nodes(&arm.body, ctx);
        }
    }

    div().into_any_element()
}

fn render_include(inc: &IncludeNode, ctx: &TemplateContext) -> AnyElement {
    // Multi-component syntax: "path/file.crepus#ComponentName"
    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return render_named_component(inc, ctx, file_part, comp_name);
    }

    // Single-component file: resolve path relative to the current file's directory.
    let file_path = match resolve_include_path(ctx.base_dir.as_deref(), &inc.path) {
        Ok(path) => path,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    let nodes = match crepuscularity_core::ast_cache::parse_file(&file_path) {
        Ok(n) => n,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    // Build child context: fresh vars from evaluated props, correct base_dir, and slot.
    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());

    for (key, expr) in &inc.props {
        let val = eval_expr_value(expr, ctx);
        child_ctx.vars.insert(key.clone(), val);
    }

    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    render_nodes(nodes.as_ref(), &child_ctx)
}

/// Render a named component from a multi-component file (`path#Name` syntax).
fn render_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
) -> AnyElement {
    let file_path = match resolve_include_path(ctx.base_dir.as_deref(), file_part) {
        Ok(path) => path,
        Err(e) => {
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(e.to_string()))
                .into_any_element();
        }
    };

    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("include error: {:?}: {}", file_path, e);
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(msg))
                .into_any_element();
        }
    };

    let comp_file = match crepuscularity_core::parser::parse_component_file(&content) {
        Ok(cf) => cf,
        Err(e) => {
            let msg = format!("component file parse error: {e}");
            return div()
                .text_color(rgb(0xff4444))
                .child(SharedString::from(msg))
                .into_any_element();
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

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());

    // Inject TOML defaults first — passed props override them.
    for (key, expr) in &comp.meta.defaults {
        let val = eval_expr_value(expr, &TemplateContext::new());
        child_ctx.vars.insert(key.clone(), val);
    }

    // Apply passed props.
    for (key, expr) in &inc.props {
        let val = eval_expr_value(expr, ctx);
        child_ctx.vars.insert(key.clone(), val);
    }

    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    render_nodes(&comp.nodes, &child_ctx)
}
