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
    AnyElement, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};

use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::preprocess::slot_rotate_child_phrases;

use crate::bindings::{ElementRequest, RuntimeBindings};
use crate::styler::{apply_element_classes, parse_duration_ms};
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
/// The legacy API derives identity from its caller location. Repeated instances
/// at the same call site (including helper functions) must use explicit namespaces.
#[track_caller]
pub fn render_nodes(nodes: &[Node], ctx: &TemplateContext) -> AnyElement {
    let caller = std::panic::Location::caller();
    render_nodes_with_bindings(
        &format!("{caller}"),
        nodes,
        ctx,
        &RuntimeBindings::default(),
    )
}

/// Render with explicitly registered host callbacks and native elements.
/// Use a unique, stable namespace for each template instance in the same view.
/// Inside a reordered loop, put `bind:key={item.id}` on its single root element.
pub fn render_nodes_with_bindings(
    namespace: &str,
    nodes: &[Node],
    ctx: &TemplateContext,
    bindings: &RuntimeBindings,
) -> AnyElement {
    render_nodes_with_ctx(nodes, ctx.clone(), namespace, bindings)
}

pub(crate) fn render_error(message: impl Into<String>) -> AnyElement {
    div()
        .text_color(rgb(0xff4444))
        .child(SharedString::from(message.into()))
        .into_any_element()
}

fn scoped_id(scope: &str, segment: &str) -> String {
    // Length-prefix segments: literal IDs containing `/` cannot alias a path.
    format!("{scope}/{}:{segment}", segment.len())
}

fn node_id(scope: &str, index: usize, node: &Node, ctx: &TemplateContext) -> String {
    if let Node::Element(element) = node {
        if let Some(binding) = element.bindings.iter().find(|b| b.prop == "id") {
            return scoped_id(
                scope,
                &format!("id:{}", value_to_str(&eval_expr_value(&binding.value, ctx))),
            );
        }
        if let Some(id) = &element.id {
            return scoped_id(scope, &format!("id:{id}"));
        }
    }
    scoped_id(scope, &format!("node:{index}"))
}

fn render_children(
    nodes: &[Node],
    mut ctx: TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> Vec<AnyElement> {
    let mut rendered = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Node::LetDecl(decl) = node {
            if !decl.is_default || !ctx.vars.contains_key(&decl.name) {
                let val = eval_expr_value(&decl.expr, &ctx);
                ctx.vars.insert(decl.name.clone(), val);
            }
        } else {
            let id = node_id(scope, index, node, &ctx);
            if !ids.insert(id.clone()) {
                rendered.push(render_error(format!(
                    "Duplicate GPUI template identity `{id}`"
                )));
                continue;
            }
            rendered.push(render_node_in_scope(node, &ctx, &id, bindings));
        }
    }
    rendered
}

fn render_nodes_with_ctx(
    nodes: &[Node],
    ctx: TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    let mut rendered = render_children(nodes, ctx, scope, bindings);
    match rendered.len() {
        1 => rendered.remove(0),
        _ => div().children(rendered).into_any_element(),
    }
}

#[track_caller]
pub fn render_node(node: &Node, ctx: &TemplateContext) -> AnyElement {
    let caller = std::panic::Location::caller();
    let id = node_id(&format!("{caller}"), 0, node, ctx);
    render_node_in_scope(node, ctx, &id, &RuntimeBindings::default())
}

fn render_node_in_scope(
    node: &Node,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    match node {
        Node::Element(el) => render_element(el, ctx, scope, bindings),
        Node::Text(parts) => div()
            .child(SharedString::from(render_text(parts, ctx)))
            .into_any_element(),
        Node::If(block) => render_if(block, ctx, scope, bindings),
        Node::For(block) => render_for(block, ctx, scope, bindings),
        Node::Match(block) => render_match(block, ctx, scope, bindings),
        Node::LetDecl(_) => div().into_any_element(),
        Node::RawText(expr) | Node::RawHtml(expr) => div()
            .child(SharedString::from(value_to_str(&eval_expr_value(
                expr, ctx,
            ))))
            .into_any_element(),
        Node::Include(inc) => render_include(inc, ctx, scope, bindings),
        Node::Embed(_) => {
            render_error("GPUI does not execute web embeds; register a native element instead")
        }
    }
}

fn render_element(
    el: &Element,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    if el.tag == "slot" {
        return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
            render_nodes_with_ctx(slot_nodes, (**slot_ctx).clone(), scope, bindings)
        } else {
            render_nodes_with_ctx(&el.children, ctx.clone(), scope, bindings)
        };
    }
    if bindings.has_element(&el.tag) {
        return bindings.render_element(ElementRequest {
            id: SharedString::from(scope.to_owned()),
            element: el.clone(),
            context: ctx.clone(),
            children: render_children(&el.children, ctx.clone(), scope, bindings),
        });
    }
    if el.tag == "input" {
        return crate::text_input::runtime_input(el, ctx, scope, bindings)
            .unwrap_or_else(render_error);
    }
    if el.tag == "textarea" {
        return render_error(
            "GPUI textarea/multiline editing is not implemented; register a native editor",
        );
    }
    if !matches!(
        el.tag.as_str(),
        "div"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "nav"
            | "aside"
            | "figure"
            | "ul"
            | "ol"
            | "li"
            | "span"
            | "p"
            | "label"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "strong"
            | "em"
            | "small"
            | "button"
            | "img"
            | "image"
            | "svg"
            | "slot-rotate"
    ) {
        return render_error(format!(
            "Unregistered GPUI element `{}`; register a native factory",
            el.tag
        ));
    }
    let classes = el.classes.iter().map(String::as_str).chain(
        el.conditional_classes
            .iter()
            .filter(|cc| eval_condition_bool(ctx, &cc.condition))
            .map(|cc| cc.class.as_str()),
    );
    let mut d = apply_element_classes(
        base_tag_element(&el.tag).id(SharedString::from(scope.to_owned())),
        classes,
        Some(ctx),
    );
    if el.tag == "button" {
        d = d.role(gpui::accesskit::Role::Button);
    }
    if matches!(el.tag.as_str(), "img" | "image" | "svg") {
        d = d.role(gpui::accesskit::Role::Image);
    }
    d = match crate::runtime_element::apply_bindings(d, el, ctx) {
        Ok(d) => d,
        Err(e) => return render_error(e),
    };
    d = match crate::runtime_element::apply_events(d, el, ctx, scope, bindings) {
        Ok(d) => d,
        Err(e) => return render_error(e),
    };
    if el.tag == "img" || el.tag == "image" {
        let source = el
            .bindings
            .iter()
            .find(|b| b.prop == "src" || b.prop == "path");
        let Some(source) = source else {
            return render_error(
                "GPUI image requires src={...} (asset/URL) or path={...} (filesystem)",
            );
        };
        let value = value_to_str(&eval_expr_value(&source.value, ctx));
        let source: gpui::ImageSource = if source.prop == "path" {
            std::path::PathBuf::from(value).into()
        } else {
            SharedString::from(value).into()
        };
        // Img does not expose its own a11y node in the pinned CE release.
        let mut image = gpui::img(source).id("image");
        transfer_image_size(&mut d, &mut image);
        d = d.child(image);
    } else if el.tag == "svg" {
        let Some(source) = el.bindings.iter().find(|b| b.prop == "src") else {
            return render_error("GPUI svg requires src={...}");
        };
        let path = SharedString::from(value_to_str(&eval_expr_value(&source.value, ctx)));
        let mut image = gpui::svg().path(path).id("image");
        transfer_image_size(&mut d, &mut image);
        if image.style().size.width.is_none() {
            image = image.w(gpui::rems(1.));
        }
        if image.style().size.height.is_none() {
            image = image.h(gpui::rems(1.));
        }
        d = d.child(image);
    } else if el.tag == "slot-rotate" {
        let label = slot_rotate_child_phrases(&el.children)
            .ok()
            .and_then(|p| p.into_iter().next())
            .unwrap_or_default();
        d = d.child(SharedString::from(label));
    } else {
        d = d.children(render_children(&el.children, ctx.clone(), scope, bindings));
    }
    if !el.animations.is_empty() {
        if let Some(spec) = el.animations.iter().find(|a| {
            !matches!(
                a.property.as_str(),
                "opacity"
                    | "fade"
                    | "fade-in"
                    | "fade-out"
                    | "pulse"
                    | "slide-down"
                    | "slide-up"
                    | "slide-right"
                    | "slide-left"
                    | "grow"
            )
        }) {
            return render_error(format!(
                "Unsupported GPUI animation `{}`; use a native element",
                spec.property
            ));
        }
        return render_with_animations(d, &el.animations, scope);
    }
    d.into_any_element()
}

// Sizing utilities apply to the actual image; the semantic wrapper follows its
// child. Raster auto dimensions retain CE intrinsic measurement. SVG defaults
// to one rem because the pinned Svg has no intrinsic-size measurement.
fn transfer_image_size(wrapper: &mut impl Styled, image: &mut impl Styled) {
    image.style().size = std::mem::take(&mut wrapper.style().size);
    image.style().min_size = std::mem::take(&mut wrapper.style().min_size);
    image.style().max_size = std::mem::take(&mut wrapper.style().max_size);
}

/// Wrap a div with GPUI animations based on the parsed animation specs.
fn render_with_animations(
    d: gpui::Stateful<gpui::Div>,
    animations: &[AnimationSpec],
    scope: &str,
) -> AnyElement {
    let id_str = scoped_id(scope, "animation");
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
fn apply_animation_property<E: Styled>(d: E, property: &str, delta: f32) -> E {
    match property {
        "opacity" | "fade" | "fade-in" => d.opacity(delta),
        "fade-out" => d.opacity(1.0 - delta),
        "pulse" => d.opacity(0.4 + delta * 0.6),
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

fn render_if(
    block: &IfBlock,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    if eval_condition_bool(ctx, &block.condition) {
        render_nodes_with_ctx(
            &block.then_children,
            ctx.clone(),
            &scoped_id(scope, "then"),
            bindings,
        )
    } else if let Some(else_children) = &block.else_children {
        render_nodes_with_ctx(
            else_children,
            ctx.clone(),
            &scoped_id(scope, "else"),
            bindings,
        )
    } else {
        div().into_any_element()
    }
}

fn render_for(
    block: &ForBlock,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    let items = match crepuscularity_core::eval::eval_expr(&block.iterator, ctx) {
        Ok(TemplateValue::List(items)) => items,
        Ok(_) => return render_error("GPUI loop iterator must evaluate to a list"),
        Err(e) => return render_error(e.to_string()),
    };

    let mut d = div();
    let mut child_ctx = ctx.clone();
    let pattern = block.pattern.trim();
    let has_pattern = !pattern.is_empty();
    let mut keys = std::collections::HashSet::new();
    for (index, item_ctx) in items.iter().enumerate() {
        child_ctx.vars.clone_from(&ctx.vars);
        child_ctx.vars.extend(item_ctx.vars.clone());
        if has_pattern {
            let value = if item_ctx.vars.len() == 1 {
                item_ctx
                    .vars
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| TemplateValue::Scope(item_ctx.clone()))
            } else {
                TemplateValue::Scope(item_ctx.clone())
            };
            child_ctx.vars.insert(pattern.to_owned(), value);
        }
        let key = if let [Node::Element(root)] = block.body.as_slice() {
            root.bindings
                .iter()
                .find(|b| b.prop == "key")
                .map(|b| value_to_str(&eval_expr_value(&b.value, &child_ctx)))
        } else {
            None
        }
        .unwrap_or_else(|| index.to_string());
        if !keys.insert(key.clone()) {
            return render_error(format!("Duplicate GPUI loop key `{key}`"));
        }
        let child = render_nodes_with_ctx(
            &block.body,
            child_ctx.clone(),
            &scoped_id(scope, &format!("item:{key}")),
            bindings,
        );
        d = d.child(child);
    }
    d.into_any_element()
}

fn render_match(
    block: &MatchBlock,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    let val = eval_expr_value(&block.expr, ctx);
    let value = value_to_str(&val);

    for (index, arm) in block.arms.iter().enumerate() {
        let pattern = arm.pattern.trim();
        if pattern == "_" {
            return render_nodes_with_ctx(
                &arm.body,
                ctx.clone(),
                &scoped_id(scope, &format!("arm:{index}")),
                bindings,
            );
        }
        if pattern.starts_with('"') && pattern.ends_with('"') {
            let lit = &pattern[1..pattern.len() - 1];
            if value == lit {
                return render_nodes_with_ctx(
                    &arm.body,
                    ctx.clone(),
                    &scoped_id(scope, &format!("arm:{index}")),
                    bindings,
                );
            }
        }
        if value == pattern {
            return render_nodes_with_ctx(
                &arm.body,
                ctx.clone(),
                &scoped_id(scope, &format!("arm:{index}")),
                bindings,
            );
        }
    }

    div().into_any_element()
}

fn render_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    scope: &str,
    bindings: &RuntimeBindings,
) -> AnyElement {
    // Multi-component syntax: "path/file.crepus#ComponentName"
    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return render_named_component(inc, ctx, file_part, comp_name, scope, bindings);
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

    render_nodes_with_ctx(nodes.as_ref(), child_ctx, scope, bindings)
}

/// Render a named component from a multi-component file (`path#Name` syntax).
fn render_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
    scope: &str,
    bindings: &RuntimeBindings,
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

    render_nodes_with_ctx(&comp.nodes, child_ctx, scope, bindings)
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn explicit_ids_are_scoped_and_stable_when_siblings_reorder() {
        let nodes = crepuscularity_core::parser::parse_template("button #save").unwrap();
        let ctx = TemplateContext::new();
        let first = node_id("instance:a", 0, &nodes[0], &ctx);
        assert_eq!(first, node_id("instance:a", 9, &nodes[0], &ctx));
        assert_ne!(first, node_id("instance:b", 0, &nodes[0], &ctx));
        assert_ne!(
            scoped_id("root", "a/b"),
            scoped_id(&scoped_id("root", "a"), "b")
        );
    }
}

#[cfg(test)]
mod image_mapping_tests {
    use super::*;

    #[test]
    fn image_dimensions_move_to_the_real_image_without_forcing_intrinsic_axes() {
        let mut wrapper = gpui::div()
            .id("wrapper")
            .w(gpui::px(24.))
            .max_h(gpui::px(48.));
        let expected_width = wrapper.style().size.width.clone();
        let expected_max_height = wrapper.style().max_size.height.clone();
        let mut image = gpui::img("embedded.png").id("image");
        transfer_image_size(&mut wrapper, &mut image);
        assert_eq!(image.style().size.width, expected_width);
        assert_eq!(image.style().max_size.height, expected_max_height);
        assert!(image.style().size.height.is_none());
        assert!(wrapper.style().size.width.is_none());
        assert!(gpui::Element::id(&image).is_some());
    }
}
