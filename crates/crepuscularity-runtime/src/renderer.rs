/// Runtime GPUI renderer — walks the AST and builds GPUI elements dynamically.
///
/// Supports:
/// - Dynamic theme colors via context expressions in class values
/// - GPUI animations via `animate:property={duration easing}` attributes
/// - All standard Tailwind-like classes mapped to GPUI methods
///
/// Direct public render calls share include snapshots for that call and recheck
/// sources on the next call. HotReloadView supplies worker-prepared snapshots.
/// Props, defaults, slots and animation paths remain per occurrence.
use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    bounce, div, ease_in_out, ease_out_quint, linear, quadratic, rgb, Animation, AnimationExt,
    AnyElement, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};

use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::preprocess::slot_rotate_child_phrases;
use crepuscularity_core::virtual_files::lookup_virtual_file;

mod prepared;
pub(crate) use prepared::{prepare_includes, PreparedIncludes};

use crate::styler::{apply_class_with_ctx, parse_duration_ms, state_style_with_initial};
use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};

fn eval_expr_value(expr: &str, ctx: &TemplateContext) -> TemplateValue {
    crepuscularity_core::eval::eval_expr(expr, ctx).unwrap_or(TemplateValue::Null)
}

fn eval_condition_bool(ctx: &TemplateContext, expr: &str) -> bool {
    ctx.eval_condition(expr).unwrap_or(false)
}

const GPUI_TARGET_KEYS: [&str; 8] = [
    "crepus_target",
    "is_gui",
    "is_tui",
    "is_web",
    "crepus_platform",
    "is_macos",
    "is_windows",
    "is_linux",
];

fn has_gpui_target_defaults(ctx: &TemplateContext) -> bool {
    GPUI_TARGET_KEYS
        .iter()
        .all(|key| ctx.vars.contains_key(*key))
}

fn gpui_target_defaults() -> [(&'static str, TemplateValue); 8] {
    [
        ("crepus_target", TemplateValue::Str("gpui".into())),
        ("is_gui", TemplateValue::Bool(true)),
        ("is_tui", TemplateValue::Bool(false)),
        ("is_web", TemplateValue::Bool(false)),
        (
            "crepus_platform",
            TemplateValue::Str(std::env::consts::OS.into()),
        ),
        ("is_macos", TemplateValue::Bool(cfg!(target_os = "macos"))),
        (
            "is_windows",
            TemplateValue::Bool(cfg!(target_os = "windows")),
        ),
        ("is_linux", TemplateValue::Bool(cfg!(target_os = "linux"))),
    ]
}

fn with_gpui_target(ctx: &TemplateContext) -> Cow<'_, TemplateContext> {
    if has_gpui_target_defaults(ctx) {
        return Cow::Borrowed(ctx);
    }
    let mut ctx = ctx.clone();
    for (key, value) in gpui_target_defaults() {
        ctx.vars.entry(key.into()).or_insert(value);
    }
    Cow::Owned(ctx)
}

pub(crate) fn with_gpui_target_in_place<R>(
    ctx: &mut TemplateContext,
    render: impl FnOnce(&TemplateContext) -> R,
) -> R {
    if has_gpui_target_defaults(ctx) {
        return render(ctx);
    }
    struct Restore<'a> {
        ctx: &'a mut TemplateContext,
        inserted: Vec<&'static str>,
    }
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            for key in self.inserted.drain(..) {
                self.ctx.vars.remove(key);
            }
        }
    }
    let mut guard = Restore {
        ctx,
        inserted: Vec::with_capacity(8),
    };
    for (key, value) in gpui_target_defaults() {
        if let std::collections::hash_map::Entry::Vacant(entry) = guard.ctx.vars.entry(key.into()) {
            entry.insert(value);
            guard.inserted.push(key);
        }
    }
    render(guard.ctx)
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

fn element_id(el: &Element, ctx: &TemplateContext) -> Option<SharedString> {
    let value = if let Some(id) = &el.id {
        id.clone()
    } else {
        let binding = el.bindings.iter().find(|binding| binding.prop == "id")?;
        let value = crepuscularity_core::eval::eval_binding(binding, ctx)
            .ok()
            .flatten()?;
        value_to_str(&value)
    };
    (!value.is_empty()).then(|| SharedString::from(value))
}

fn element_state_style(
    el: &Element,
    ctx: &TemplateContext,
    state: &str,
    base: Option<&gpui::StyleRefinement>,
) -> gpui::StyleRefinement {
    let mut style = gpui::StyleRefinement::default();
    for class in el.classes.iter().map(String::as_str).chain(
        el.conditional_classes
            .iter()
            .filter(|class| eval_condition_bool(ctx, &class.condition))
            .map(|class| class.class.as_str()),
    ) {
        let is_list = class.starts_with('{') && class.ends_with('}');
        let resolved = if is_list {
            ctx.interpolate(class).unwrap_or_default()
        } else {
            class.to_owned()
        };
        let classes = if is_list {
            resolved.split_whitespace().collect::<Vec<_>>()
        } else {
            vec![resolved.as_str()]
        };
        for class in classes {
            style = state_style_with_initial(style, std::iter::once(class), state, Some(ctx), base);
        }
    }
    if style.box_shadow.as_ref() == base.and_then(|base| base.box_shadow.as_ref()) {
        style.box_shadow = None;
    }
    style
}

fn finish_element(mut d: gpui::Div, el: &Element, ctx: &TemplateContext, path: &str) -> AnyElement {
    let hover = element_state_style(el, ctx, "hover", Some(d.style()));
    let focus = element_state_style(el, ctx, "focus", Some(d.style()));
    let active = element_state_style(el, ctx, "active", Some(d.style()));
    let focusable = focus != gpui::StyleRefinement::default();
    let scrolling = [d.style(), &hover, &focus, &active].iter().any(|style| {
        style.overflow.x == Some(gpui::Overflow::Scroll)
            || style.overflow.y == Some(gpui::Overflow::Scroll)
    });
    let needs_id = scrolling || focusable || active != gpui::StyleRefinement::default();
    let d = d.hover(|_| hover).focus(|_| focus);
    let id = element_id(el, ctx)
        .or_else(|| needs_id.then(|| SharedString::from(format!("crepus-node-{path}"))));
    if let Some(id) = id {
        let mut d = d.id(id.clone()).active(|_| active);
        if focusable {
            d = d.focusable().tab_index(0);
        }
        if !el.animations.is_empty() {
            render_with_animations(d, &el.animations, path, Some(id))
        } else {
            d.into_any_element()
        }
    } else if !el.animations.is_empty() {
        render_with_animations(d, &el.animations, path, None)
    } else {
        d.into_any_element()
    }
}

fn animation_id(path: &str, explicit_id: Option<SharedString>) -> SharedString {
    explicit_id.unwrap_or_else(|| SharedString::from(format!("crepus-anim-{path}")))
}

// GPUI retains the sequence index and start time under this ID. Configuration
// edits must start fresh, especially when a shorter sequence replaces an old one.
fn animation_state_id(
    path: &str,
    explicit_id: Option<SharedString>,
    animations: &[AnimationSpec],
) -> SharedString {
    let mut config = DefaultHasher::new();
    animations.len().hash(&mut config);
    for spec in animations {
        spec.property.hash(&mut config);
        parse_duration_ms(&spec.duration_expr)
            .unwrap_or(300)
            .hash(&mut config);
        spec.easing.hash(&mut config);
        spec.repeat.hash(&mut config);
    }
    SharedString::from(format!(
        "{}-{:016x}",
        animation_id(path, explicit_id),
        config.finish()
    ))
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

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn text_and_unsupported_island_diagnostics_are_painted() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct TextView(Vec<Node>);
        impl gpui::Render for TextView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                div()
                    .size_full()
                    .bg(rgb(0xffffff))
                    .text_size(gpui::px(16.0))
                    .line_height(gpui::px(20.0))
                    .font_family("Helvetica")
                    .child(render_nodes(&self.0, &TemplateContext::new()))
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for (source, expected) in [
            ("span text-[#ff4444] \"FONT CHECK\"", true),
            ("embed widget.js adapter=\"react\"", true),
            ("div\n  embed widget.js adapter=\"react\"", true),
            ("if is_gui\n  embed widget.js adapter=\"react\"", true),
            ("slot-rotate\n  embed widget.js adapter=\"react\"", true),
            ("if is_web\n  embed widget.js adapter=\"react\"", false),
        ] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let window = cx
                .open_window(
                    gpui::size(gpui::px(900.0), gpui::px(100.0)),
                    move |_, app| app.new(|_| TextView(nodes)),
                )
                .unwrap()
                .into();
            cx.update_window(window, |_, window, app| {
                window.draw(app).clear(app);
            })
            .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let has_red = image.pixels().any(|pixel| {
                let [r, g, b, a] = pixel.0;
                r > 180 && g < 130 && b < 130 && a > 0
            });
            assert_eq!(has_red, expected, "{source}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn worker_prepared_nested_includes_paint_frozen_and_refreshed_snapshots() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct PreparedView(Vec<Node>, TemplateContext, Arc<PreparedIncludes>);
        impl gpui::Render for PreparedView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                div()
                    .size_full()
                    .bg(rgb(0xffffff))
                    .font_family("Helvetica")
                    .child(render_nodes_prepared(&self.0, &self.1, Arc::clone(&self.2)))
            }
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        let ordinary = dir.path().join("ordinary.crepus");
        let named = dir.path().join("nested/named.crepus");
        std::fs::write(&ordinary, "include nested/named.crepus#Card").unwrap();
        let source = |color| format!("--- Card\ndiv w-[40px] h-[40px] bg-[{color}]");
        std::fs::write(&named, source("#ff0000")).unwrap();
        let nodes = crepuscularity_core::parse_template("include ordinary.crepus").unwrap();
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(dir.path().into());
        let caller = std::thread::current().id();
        let prepare = || {
            let nodes = nodes.clone();
            let ctx = ctx.clone();
            let (prepared, worker) = std::thread::spawn(move || {
                (prepare_includes(&nodes, &ctx), std::thread::current().id())
            })
            .join()
            .unwrap();
            assert_ne!(worker, caller);
            prepared
        };
        let red = prepare();
        std::fs::write(&named, source("#0000ff")).unwrap();
        let blue = prepare();
        std::fs::remove_file(&ordinary).unwrap();
        std::fs::remove_file(&named).unwrap();
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for (prepared, expected) in [(red, [255, 0, 0, 255]), (blue, [0, 0, 255, 255])] {
            let nodes = nodes.clone();
            let ctx = ctx.clone();
            let window = cx
                .open_window(gpui::size(gpui::px(60.0), gpui::px(60.0)), move |_, app| {
                    app.new(|_| PreparedView(nodes, ctx, prepared))
                })
                .unwrap()
                .into();
            cx.update_window(window, |_, window, app| {
                window.draw(app).clear(app);
            })
            .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 60;
            assert_eq!(image.get_pixel(10 * scale, 10 * scale).0, expected);
            assert!(
                image.pixels().filter(|pixel| pixel.0 == expected).count()
                    >= (20 * scale * 20 * scale) as usize
            );
        }
    }

    #[cfg(target_os = "macos")]
    // Concurrent headless Metal setup aborts; serialize the GPU fixtures.
    pub(super) static METAL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn arbitrary_color_opacity_paints_over_the_parent_background() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct OpacityView(Vec<Node>);
        impl gpui::Render for OpacityView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &TemplateContext::new())
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for (color, expected) in [
            ("[#ff0000]/50", [255, 128, 128, 255]),
            ("transparent/50", [255, 255, 255, 255]),
            ("clear/100", [255, 255, 255, 255]),
        ] {
            let nodes = crepuscularity_core::parse_template(&format!(
                "div w-[100px] h-[40px] bg-white\n  div size-full bg-{color}"
            ))
            .unwrap();
            let window = cx
                .open_window(
                    gpui::size(gpui::px(120.0), gpui::px(60.0)),
                    move |_, app| app.new(|_| OpacityView(nodes)),
                )
                .unwrap()
                .into();
            cx.update_window(window, |_, window, app| {
                window.draw(app).clear(app);
            })
            .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 120;
            assert_eq!(image.get_pixel(10 * scale, 10 * scale).0, expected);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn grid_and_flex_utilities_preserve_the_expected_painted_layout() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct GridView(Vec<Node>);
        impl gpui::Render for GridView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &TemplateContext::new())
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for (source, samples) in [
            ("div grid grid-cols-2 grid-cols-0 grid-rows-2 grid-rows-0 w-[100px] h-[80px]\n  div bg-[#ff0000]\n  div bg-[#0000ff] col-start-2 col-start-0 col-end-3 col-end-0 row-start-1 row-start-0 row-end-2 row-end-0\n  div bg-[#00ff00] row-start-2 col-start-1\n  div bg-[#ffff00] row-start-2 col-start-2 col-span-1 col-span-0 row-span-1 row-span-0", [(10, 10, [255, 0, 0, 255]), (75, 10, [0, 0, 255, 255]), (10, 60, [0, 255, 0, 255]), (75, 60, [255, 255, 0, 255])]),
            ("div flex flex-col items-center items-stretch justify-between justify-evenly w-[100px] h-[100px] bg-white\n  div h-[20px] bg-[#ff0000]\n  div h-[20px] bg-[#0000ff]", [(10, 25, [255, 0, 0, 255]), (10, 65, [0, 0, 255, 255]), (10, 5, [255, 255, 255, 255]), (10, 45, [255, 255, 255, 255])]),
        ] {
        let nodes = crepuscularity_core::parse_template(source).unwrap();
        let window = cx
            .open_window(
                gpui::size(gpui::px(120.0), gpui::px(120.0)),
                move |_, app| app.new(|_| GridView(nodes)),
            )
            .unwrap()
            .into();
        cx.update_window(window, |_, window, app| {
            window.draw(app).clear(app);
        })
        .unwrap();
        let image = cx.capture_screenshot(window).unwrap();
        let scale = image.width() / 120;
        for (x, y, expected) in samples {
            assert_eq!(image.get_pixel(x * scale, y * scale).0, expected);
        }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn interaction_styles_paint_active_and_focused_colors() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::{AppContext, InputEvent};
        struct PaintView(Vec<Node>, TemplateContext);
        impl gpui::Render for PaintView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &self.1)
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for source in [
            "div flex flex-col\n  button w-[100px] h-[40px] bg-[#ff0000] active:bg-[#0000ff] focus:bg-[#00ff00]\n  button w-[100px] h-[40px] bg-[#ff0000] active:bg-[#0000ff] focus:bg-[#00ff00]",
            "div flex flex-col\n  for item in items\n    button w-[100px] h-[40px] bg-[#ff0000] active:bg-[#0000ff] focus:bg-[#00ff00]",
        ] {
        let nodes = crepuscularity_core::parse_template(source).unwrap();
        let mut template_context = TemplateContext::new();
        template_context.set("items", TemplateValue::List(vec![TemplateContext::new(), TemplateContext::new()]));
        let window = cx
            .open_window(
                gpui::size(gpui::px(120.0), gpui::px(100.0)),
                move |_, app| app.new(|_| PaintView(nodes, template_context)),
            )
            .unwrap()
            .into();
        let pixel = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window, |_, window, app| {
                window.draw(app).clear(app);
            })
            .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 120;
            [image.get_pixel(10 * scale, 10 * scale).0, image.get_pixel(10 * scale, 50 * scale).0]
        };
        assert_eq!(pixel(&mut cx), [[255, 0, 0, 255], [255, 0, 0, 255]], "{source}");
        cx.update_window(window, |_, window, app| {
            window.dispatch_event(
                gpui::MouseDownEvent {
                    position: gpui::point(gpui::px(10.0), gpui::px(10.0)),
                    button: gpui::MouseButton::Left,
                    ..Default::default()
                }
                .to_platform_input(),
                app,
            );
        })
        .unwrap();
        assert_eq!(pixel(&mut cx), [[0, 0, 255, 255], [255, 0, 0, 255]], "{source}");
        cx.update_window(window, |_, window, app| {
            window.dispatch_event(
                gpui::MouseUpEvent {
                    position: gpui::point(gpui::px(10.0), gpui::px(10.0)),
                    button: gpui::MouseButton::Left,
                    ..Default::default()
                }
                .to_platform_input(),
                app,
            );
        })
        .unwrap();
        assert_eq!(pixel(&mut cx), [[0, 255, 0, 255], [255, 0, 0, 255]], "{source}");
        cx.update_window(window, |_, window, app| window.focus_next(app)).unwrap();
        assert_eq!(pixel(&mut cx), [[255, 0, 0, 255], [0, 255, 0, 255]], "{source}");
        }
    }

    #[gpui::test]
    fn idless_focus_styles_participate_in_window_focus_navigation(cx: &mut gpui::TestAppContext) {
        struct FocusView(Vec<Node>);
        impl gpui::Render for FocusView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &TemplateContext::new())
            }
        }
        let nodes = crepuscularity_core::parse_template("div flex flex-col\n  button w-[100px] h-[40px] focus:bg-red-500\n  button w-[100px] h-[40px] focus:bg-blue-500").unwrap();
        let (_, cx) = cx.add_window_view(move |_, _| FocusView(nodes));
        cx.simulate_click(
            gpui::point(gpui::px(10.0), gpui::px(10.0)),
            gpui::Modifiers::default(),
        );
        let first = cx.update(|window, app| window.focused(app).unwrap());
        cx.simulate_click(
            gpui::point(gpui::px(10.0), gpui::px(50.0)),
            gpui::Modifiers::default(),
        );
        let second = cx.update(|window, app| window.focused(app).unwrap());
        assert_ne!(first, second);
        cx.update(|window, app| window.focus_prev(app));
        assert_eq!(cx.update(|window, app| window.focused(app).unwrap()), first);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn scroll_wheel_moves_painted_content_and_preserves_clipping() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::{AppContext, InputEvent};
        struct ScrollView(Vec<Node>);
        impl gpui::Render for ScrollView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &TemplateContext::new())
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        for (class, horizontal) in [
            ("overflow-auto", false),
            ("overflow-scroll", true),
            ("overflow-x-auto", true),
            ("overflow-x-scroll", true),
            ("overflow-y-auto", false),
            ("overflow-y-scroll", false),
        ] {
            let direction = if horizontal { "flex-row" } else { "flex-col" };
            let source = format!("div w-[120px] h-[100px] bg-white\n  div flex {direction} w-[40px] h-[40px] {class}\n    div shrink-0 w-[40px] h-[40px] bg-[#ff0000]\n    div shrink-0 w-[40px] h-[40px] bg-[#0000ff]");
            let nodes = crepuscularity_core::parse_template(&source).unwrap();
            let window = cx
                .open_window(
                    gpui::size(gpui::px(120.0), gpui::px(100.0)),
                    move |_, app| app.new(|_| ScrollView(nodes)),
                )
                .unwrap()
                .into();
            let pixels = |cx: &mut gpui::HeadlessAppContext| {
                cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                    .unwrap();
                let image = cx.capture_screenshot(window).unwrap();
                let scale = image.width() / 120;
                [
                    image.get_pixel(10 * scale, 10 * scale).0,
                    image
                        .get_pixel(
                            if horizontal { 50 * scale } else { 10 * scale },
                            if horizontal { 10 * scale } else { 50 * scale },
                        )
                        .0,
                ]
            };
            assert_eq!(
                pixels(&mut cx),
                [[255, 0, 0, 255], [255, 255, 255, 255]],
                "{class}"
            );
            cx.update_window(window, |_, window, app| {
                window.dispatch_event(
                    gpui::ScrollWheelEvent {
                        position: gpui::point(gpui::px(10.0), gpui::px(10.0)),
                        delta: gpui::ScrollDelta::Pixels(gpui::point(
                            gpui::px(if horizontal { -40.0 } else { 0.0 }),
                            gpui::px(if horizontal { 0.0 } else { -40.0 }),
                        )),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    app,
                );
            })
            .unwrap();
            assert_eq!(
                pixels(&mut cx),
                [[0, 0, 255, 255], [255, 255, 255, 255]],
                "{class}"
            );
            assert_eq!(
                pixels(&mut cx),
                [[0, 0, 255, 255], [255, 255, 255, 255]],
                "scroll position persists for {class}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn runtime_state_shadow_resets_preserve_the_other_shadow_kind() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::{AppContext, InputEvent};
        struct StateShadowView(Vec<Node>, TemplateContext);
        impl gpui::Render for StateShadowView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &self.1)
            }
        }
        let nodes = crepuscularity_core::parse_template("div flex flex-row gap-[20px] bg-white\n  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 hover:ring-0\n  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class={state_classes}").unwrap();
        let mut template_context = TemplateContext::new();
        template_context.set("state_classes", "hover:shadow-none");
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        let window = cx
            .open_window(
                gpui::size(gpui::px(160.0), gpui::px(80.0)),
                move |_, app| app.new(|_| StateShadowView(nodes, template_context)),
            )
            .unwrap()
            .into();
        let pixels = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 160;
            [6, 9, 82, 85].map(|x| image.get_pixel(x * scale, 28 * scale).0)
        };
        let initial = pixels(&mut cx);
        for x in [28.0, 104.0] {
            cx.update_window(window, |_, window, app| {
                window.dispatch_event(
                    gpui::MouseMoveEvent {
                        position: gpui::point(gpui::px(x), gpui::px(28.0)),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    app,
                );
            })
            .unwrap();
            let actual = pixels(&mut cx);
            if x == 28.0 {
                assert_eq!(actual[0], initial[0]);
                assert_eq!(actual[1], [255, 255, 255, 255]);
            } else {
                assert_eq!(actual[1], initial[1]);
                assert_eq!(actual[2], [255, 255, 255, 255]);
                assert_eq!(actual[3], initial[3]);
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn dynamic_state_shadow_resets_preserve_the_other_shadow_kind() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::{AppContext, InputEvent};
        struct StateShadowView(Vec<Node>, TemplateContext);
        impl gpui::Render for StateShadowView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &self.1)
            }
        }
        let nodes = crepuscularity_core::parse_template("div flex flex-row gap-[20px] bg-white\n  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class=\"hover:{first}\"\n  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class=\"hover:p-{second}\"").unwrap();
        let mut template_context = TemplateContext::new();
        template_context.set("first", "ring-0");
        template_context.set("second", "0 shadow-none");
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        let window = cx
            .open_window(
                gpui::size(gpui::px(160.0), gpui::px(80.0)),
                move |_, app| app.new(|_| StateShadowView(nodes, template_context)),
            )
            .unwrap()
            .into();
        let pixels = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 160;
            [6, 9, 82, 85].map(|x| image.get_pixel(x * scale, 28 * scale).0)
        };
        let initial = pixels(&mut cx);
        for x in [28.0, 104.0] {
            cx.update_window(window, |_, window, app| {
                window.dispatch_event(
                    gpui::MouseMoveEvent {
                        position: gpui::point(gpui::px(x), gpui::px(28.0)),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    app,
                );
            })
            .unwrap();
            let actual = pixels(&mut cx);
            if x == 28.0 {
                assert_eq!(actual[0], initial[0]);
                assert_eq!(actual[1], [255, 255, 255, 255]);
            } else {
                assert_eq!(actual[1], initial[1]);
                assert_eq!(actual[2], [255, 255, 255, 255]);
                assert_eq!(actual[3], initial[3]);
            }
        }
    }

    #[test]
    fn state_shadow_resets_compose_with_base_rings_and_shadows() {
        let nodes = crepuscularity_core::parse_template(
            "div hover:ring-0\ndiv hover:shadow-none\ndiv hover:ring-inset",
        )
        .unwrap();
        let mut base = crate::styler::apply_class_list(gpui::div(), "shadow-md ring-2");
        for (index, node) in nodes.iter().enumerate() {
            let Node::Element(element) = node else {
                panic!("expected element")
            };
            let actual = element_state_style(
                element,
                &TemplateContext::new(),
                "hover",
                Some(base.style()),
            );
            let class = ["ring-0", "shadow-none", "ring-inset"][index];
            let mut expected = gpui::div();
            expected.style().box_shadow = base.style().box_shadow.clone();
            let mut expected = crate::styler::apply_class(expected, class);
            assert_eq!(actual.box_shadow, expected.style().box_shadow);
        }
    }

    #[test]
    fn scrolling_elements_have_stable_distinct_ids_and_axis_styles() {
        let nodes = crepuscularity_core::parse_template("div overflow-auto\ndiv overflow-x-scroll\ndiv hover:overflow-y-auto\ndiv overflow-scroll overflow-visible\ndiv #explicit overflow-y-scroll").unwrap();
        let ids = || {
            let mut rendered = render_siblings_with_ctx(
                &nodes,
                &TemplateContext::new(),
                "root",
                None,
                &mut IncludeCache::default(),
            );
            let mut ids = Vec::new();
            for index in [0, 1, 2, 4] {
                let element = rendered[index]
                    .downcast_mut::<gpui::Stateful<gpui::Div>>()
                    .unwrap();
                ids.push(gpui::Element::id(element).unwrap());
                if index == 0 {
                    assert_eq!(element.style().overflow.x, Some(gpui::Overflow::Scroll));
                    assert_eq!(element.style().overflow.y, Some(gpui::Overflow::Scroll));
                } else if index == 1 {
                    assert_eq!(element.style().overflow.x, Some(gpui::Overflow::Scroll));
                    assert_eq!(element.style().overflow.y, None);
                }
            }
            assert!(rendered[3].downcast_mut::<gpui::Div>().is_some());
            ids
        };
        let first = ids();
        assert_eq!(first, ids());
        assert_ne!(first[0], first[1]);
        assert_ne!(first[1], first[2]);
        assert_eq!(first[3], ElementId::Name("explicit".into()));
    }

    #[test]
    fn interaction_styles_give_idless_elements_distinct_gpui_identities() {
        let nodes = crepuscularity_core::parse_template("button active:bg-red-500\nbutton focus:bg-blue-500\ndiv\nbutton #explicit active:bg-black").unwrap();
        let mut rendered = render_siblings_with_ctx(
            &nodes,
            &TemplateContext::new(),
            "root",
            None,
            &mut IncludeCache::default(),
        );
        let mut ids = Vec::new();
        for index in [0, 1, 3] {
            let element = rendered[index]
                .downcast_mut::<gpui::Stateful<gpui::Div>>()
                .unwrap();
            ids.push(gpui::Element::id(element).unwrap());
        }
        assert_ne!(ids[0], ids[1]);
        assert_eq!(ids[2], ElementId::Name("explicit".into()));
        assert!(rendered[2].downcast_mut::<gpui::Div>().is_some());
    }

    #[test]
    fn idless_sibling_animations_have_distinct_stable_gpui_ids() {
        let nodes = crepuscularity_core::parse_template(
            "div animate:opacity={100ms linear}\ndiv animate:opacity={200ms linear}",
        )
        .unwrap();
        let ids = || {
            render_siblings_with_ctx(
                &nodes,
                &TemplateContext::new(),
                "root",
                None,
                &mut IncludeCache::default(),
            )
            .iter_mut()
            .map(|element| {
                let animation = element
                    .downcast_mut::<gpui::AnimationElement<gpui::Div>>()
                    .unwrap();
                gpui::Element::id(animation).unwrap()
            })
            .collect::<Vec<_>>()
        };
        let first = ids();
        assert_ne!(first[0], first[1]);
        assert_eq!(first, ids());
        assert_eq!(
            animation_id("root/0", Some("explicit".into())).as_ref(),
            "explicit"
        );
    }

    #[test]
    fn animation_state_identity_changes_with_ordered_configuration() {
        let spec = AnimationSpec {
            property: "opacity".into(),
            duration_expr: "100ms".into(),
            easing: "linear".into(),
            repeat: false,
        };
        let id =
            |specs: &[AnimationSpec]| animation_state_id("root/0", Some("explicit".into()), specs);
        let original = id(std::slice::from_ref(&spec));
        assert_eq!(original, id(std::slice::from_ref(&spec)));
        for changed in [
            AnimationSpec {
                property: "scale".into(),
                ..spec.clone()
            },
            AnimationSpec {
                duration_expr: "200ms".into(),
                ..spec.clone()
            },
            AnimationSpec {
                easing: "bounce".into(),
                ..spec.clone()
            },
            AnimationSpec {
                repeat: true,
                ..spec.clone()
            },
        ] {
            assert_ne!(original, id(std::slice::from_ref(&changed)));
            assert_ne!(
                id(&[spec.clone(), changed.clone()]),
                id(&[changed, spec.clone()])
            );
        }
        assert_ne!(original, id(&[spec.clone(), spec]));
        assert_eq!(
            animation_id("root/0", Some("explicit".into())).as_ref(),
            "explicit"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn quoted_dimension_expressions_repaint_when_context_changes() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct DimensionView(Vec<Node>, TemplateContext);
        impl gpui::Render for DimensionView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                render_nodes(&self.0, &self.1)
            }
        }
        let nodes = crepuscularity_core::parse_template(
            "div size-full bg-white\n  div class=\"w-[{width + 0}px] h-[{height + 0}px] bg-red-500\"",
        )
        .unwrap();
        let mut context = TemplateContext::new();
        context.set("width", 30_i64);
        context.set("height", 20_i64);
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        let entity = std::rc::Rc::new(std::cell::RefCell::new(None));
        let captured = entity.clone();
        let window = cx
            .open_window(
                gpui::size(gpui::px(100.0), gpui::px(100.0)),
                move |_, app| {
                    let view = app.new(|_| DimensionView(nodes, context));
                    *captured.borrow_mut() = Some(view.clone());
                    view
                },
            )
            .unwrap()
            .into();
        let assert_bounds = |cx: &mut gpui::HeadlessAppContext, width: u32, height: u32| {
            cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                .unwrap();
            let image = cx.capture_screenshot(window).unwrap();
            let scale = image.width() / 100;
            for y in 0..100 {
                for x in 0..100 {
                    let pixel = image.get_pixel(x * scale, y * scale).0;
                    if x < width && y < height {
                        assert!(
                            pixel[0] > 150 && pixel[1] < 130 && pixel[2] < 130,
                            "expected red at ({x}, {y}), got {pixel:?}"
                        );
                    } else {
                        assert_eq!(pixel, [255, 255, 255, 255], "({x}, {y})");
                    }
                }
            }
        };
        assert_bounds(&mut cx, 30, 20);
        cx.update_window(window, |_, _, app| {
            entity.borrow().as_ref().unwrap().update(app, |view, cx| {
                view.1.set("width", 60_i64);
                view.1.set("height", 40_i64);
                cx.notify();
            });
        })
        .unwrap();
        assert_bounds(&mut cx, 60, 40);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a Metal device"]
    fn hot_reload_can_shrink_an_active_animation_sequence() {
        let _guard = METAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        use gpui::AppContext;
        struct AnimatedView(std::rc::Rc<std::cell::Cell<bool>>);
        impl gpui::Render for AnimatedView {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                let source = if self.0.get() {
                    "div size-full bg-white\n  div #animated size-full bg-black animate:opacity={1ms linear}"
                } else {
                    "div size-full bg-white\n  div #animated size-full bg-black animate:opacity={1ms linear} animate:fade={100s linear}"
                };
                render_nodes(
                    &crepuscularity_core::parse_template(source).unwrap(),
                    &TemplateContext::new(),
                )
            }
        }
        let platform = gpui_platform::current_platform(true);
        let mut cx = gpui::HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        let shrink = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = shrink.clone();
        let window = cx
            .open_window(
                gpui::size(gpui::px(100.0), gpui::px(100.0)),
                move |_, app| app.new(|_| AnimatedView(flag)),
            )
            .unwrap()
            .into();
        let draw = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                .unwrap();
        };
        draw(&mut cx);
        std::thread::sleep(Duration::from_millis(15));
        draw(&mut cx); // Finishes the first stage and advances the retained index.
        draw(&mut cx); // Renders stage 1, whose fade starts almost transparent.
        let image = cx.capture_screenshot(window).unwrap();
        assert!(image.get_pixel(image.width() / 2, image.height() / 2).0[0] > 200);
        shrink.set(true);
        draw(&mut cx); // Previously indexed stage 1 in a length-one animation list.
        std::thread::sleep(Duration::from_millis(15));
        draw(&mut cx);
        let image = cx.capture_screenshot(window).unwrap();
        assert!(image.get_pixel(image.width() / 2, image.height() / 2).0[0] < 20);
    }

    #[test]
    fn nested_declarations_style_later_siblings_without_leaking_to_parent() {
        let mut ctx = TemplateContext::new();
        ctx.set("color", "#00ff00");
        let nodes = crepuscularity_core::parse_template("div\n  div bg-{color}\n  $: default color = \"#ff0000\"\n  div bg-{color}\n  $: let color = \"#ff0000\"\n  div bg-{color}\n  $: let color = \"#0000ff\"\n  div bg-{color}").unwrap();
        let Node::Element(parent) = &nodes[0] else {
            panic!("expected element");
        };
        let mut children = render_siblings_with_ctx(
            &parent.children,
            &ctx,
            "root",
            None,
            &mut IncludeCache::default(),
        );
        assert_eq!(children.len(), 4);
        for (child, color) in children
            .iter_mut()
            .zip([0x00ff00, 0x00ff00, 0xff0000, 0x0000ff])
        {
            let actual = child.downcast_mut::<gpui::Div>().unwrap();
            let mut expected = div().bg(gpui::rgb(color));
            assert_eq!(
                format!("{:?}", actual.style().background),
                format!("{:?}", expected.style().background)
            );
        }
        assert_eq!(value_to_str(ctx.get("color").unwrap()), "#00ff00");
        let _ = render_element(parent, &ctx, "root", None, &mut IncludeCache::default());
    }

    #[test]
    fn explicit_and_bound_ids_survive_runtime_resolution() {
        let mut ctx = TemplateContext::new();
        ctx.set("name", "dynamic");
        for (source, expected) in [
            ("div #static", Some("static")),
            (
                "div #animated animate:opacity={100ms linear}",
                Some("animated"),
            ),
            ("slot-rotate #rotating\n  \"hello\"", Some("rotating")),
            ("div id={name}", Some("dynamic")),
            ("div id={1 +}", None),
            ("div", None),
        ] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert_eq!(element_id(element, &ctx).as_deref(), expected);
            let _ = render_element(element, &ctx, "root", None, &mut IncludeCache::default());
        }
    }
}

#[cfg(test)]
mod state_style_tests {
    use super::*;

    #[test]
    fn runtime_state_styles_merge_classes_context_and_conditions() {
        let mut ctx = TemplateContext::new();
        ctx.set("theme", "hover:bg-red-500 hover:p-2");
        ctx.set("color", "#ff0000");
        ctx.set("enabled", true);
        let nodes = crepuscularity_core::parse_template("div #state class={theme} hover:bg-{ color } class:hover:p-4={enabled} active:bg-black animate:opacity={100ms linear}").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        let actual = element_state_style(element, &ctx, "hover", None);
        let mut expected = div().bg(rgb(0xff0000)).p(gpui::rems(1.0));
        assert_eq!(format!("{actual:?}"), format!("{:?}", expected.style()));
        let _ = render_element(element, &ctx, "root", None, &mut IncludeCache::default());
    }

    #[test]
    fn interpolated_state_shadow_utilities_preserve_the_other_base_shadow_kind() {
        let nodes = crepuscularity_core::parse_template("div class=\"hover:{variant}\"").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element");
        };
        let mut base = crate::styler::apply_class_list(gpui::div(), "shadow-md ring-2");
        let mut ctx = TemplateContext::new();
        for variant in [
            "ring-0",
            "shadow-none",
            "ring-inset",
            "ring-0 shadow-none",
            "gpui:ring-0",
            "gpui:shadow-none",
            "gpui:ring-inset",
            "gpui:ring-0 gui:shadow-none",
        ] {
            ctx.set("variant", variant);
            let actual = element_state_style(element, &ctx, "hover", Some(base.style()));
            let mut expected = gpui::div();
            expected.style().box_shadow = base.style().box_shadow.clone();
            let mut expected = crate::styler::apply_class_list(expected, variant);
            assert_eq!(actual.box_shadow, expected.style().box_shadow, "{variant}");
        }
        for (source, variant, utilities) in [
            ("div class=\"hover:p-{variant}\"", "2 ring-0", "p-2 ring-0"),
            (
                "div class=\"hover:p-{variant}\"",
                "2 shadow-none",
                "p-2 shadow-none",
            ),
            ("div class=\"hover:gpui:{variant}\"", "ring-0", "ring-0"),
            (
                "div class=\"hover:gpui:{variant}\"",
                "shadow-none",
                "shadow-none",
            ),
        ] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element");
            };
            ctx.set("variant", variant);
            let actual = element_state_style(element, &ctx, "hover", Some(base.style()));
            let mut expected = gpui::div();
            expected.style().box_shadow = base.style().box_shadow.clone();
            let mut expected = crate::styler::apply_class_list(expected, utilities);
            assert_eq!(
                actual.box_shadow,
                expected.style().box_shadow,
                "{source}: {variant}"
            );
            assert_eq!(
                actual.padding,
                expected.style().padding,
                "{source}: {variant}"
            );
        }
    }

    #[test]
    fn unmatched_and_nonrecursive_state_utilities_do_not_inherit_base_shadows() {
        let mut base = crate::styler::apply_class_list(gpui::div(), "shadow-md ring-2");
        let mut ctx = TemplateContext::new();
        ctx.set("nested", "ring-0");
        for (source, variant) in [
            ("div class=\"hover:{variant}\"", "p-2"),
            ("div class=\"hover:{variant}\"", "unknown-utility"),
            ("div class=\"hover:{variant}\"", ""),
            ("div class=\"web:hover:{variant}\"", "ring-0"),
            ("div class=\"hover:{variant}\"", "web:ring-0"),
            ("div class=\"focus:{variant}\"", "ring-0"),
            ("div class=\"hover:{variant}\"", "{nested}"),
        ] {
            let nodes = crepuscularity_core::parse_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element");
            };
            ctx.set("variant", variant);
            let actual = element_state_style(element, &ctx, "hover", Some(base.style()));
            assert!(
                actual.box_shadow.is_none(),
                "{source}: {variant}: {:?}",
                actual.box_shadow
            );
        }
    }

    #[test]
    fn state_shadow_empty_reset_is_not_seeded_again_before_a_later_ring() {
        let nodes = crepuscularity_core::parse_template("div class=\"hover:{variant}\"").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element");
        };
        let mut base = crate::styler::apply_class_list(gpui::div(), "shadow-md");
        let mut ctx = TemplateContext::new();
        ctx.set("variant", "shadow-none ring-4");
        let actual = element_state_style(element, &ctx, "hover", Some(base.style()));
        let mut expected = gpui::div();
        expected.style().box_shadow = base.style().box_shadow.clone();
        let mut expected = crate::styler::apply_class_list(expected, "shadow-none ring-4");
        assert_eq!(actual.box_shadow, expected.style().box_shadow);
    }
}

#[cfg(test)]
mod target_context_tests {
    use super::*;

    fn assert_same_vars(left: &TemplateContext, right: &TemplateContext) {
        assert_eq!(left.vars.len(), right.vars.len());
        for (key, value) in &left.vars {
            assert_eq!(
                format!("{value:?}"),
                format!("{:?}", right.vars.get(key).unwrap())
            );
        }
    }

    #[test]
    fn gpui_target_defaults_are_available_without_mutating_caller_values() {
        let mut original = TemplateContext::new();
        let defaults = with_gpui_target(&original);
        assert!(eval_condition_bool(&defaults, "is_gui"));
        assert!(!eval_condition_bool(&defaults, "is_tui || is_web"));
        assert_eq!(value_to_str(defaults.get("crepus_target").unwrap()), "gpui");
        assert!(original.get("is_gui").is_none());
        original.set("is_gui", false);
        original.set("crepus_target", "custom");
        let overridden = with_gpui_target(&original);
        assert!(!eval_condition_bool(&overridden, "is_gui"));
        assert_eq!(
            value_to_str(overridden.get("crepus_target").unwrap()),
            "custom"
        );
        let nodes =
            crepuscularity_core::parse_template("if is_gui\n  div #gui\nelse\n  div #other")
                .unwrap();
        let _ = render_nodes(&nodes, &TemplateContext::new());
        let _ = render_node(&nodes[0], &TemplateContext::new());
    }
    #[test]
    fn in_place_defaults_restore_all_caller_data_and_reuse_payloads() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "items",
            TemplateValue::List(vec![TemplateContext::new(); 256]),
        );
        let mut scope = TemplateContext::new();
        scope.set("text", "large".repeat(1024));
        ctx.set("scope", TemplateValue::Scope(scope));
        ctx.set("is_gui", TemplateValue::Null);
        ctx.set("crepus_target", "custom");
        Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "span".into());
        ctx.slot = Some((Vec::new(), Arc::new(TemplateContext::new())));
        let before = ctx.clone();
        let Some(TemplateValue::List(items)) = ctx.get("items") else {
            panic!("items");
        };
        let items_pointer = items.as_ptr();
        let Some(TemplateValue::Scope(scope)) = ctx.get("scope") else {
            panic!("scope");
        };
        let Some(TemplateValue::Str(text)) = scope.get("text") else {
            panic!("text");
        };
        let text_pointer = text.as_ptr();
        for panic in [false, true] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_gpui_target_in_place(&mut ctx, |current| {
                    assert!(has_gpui_target_defaults(current));
                    assert!(matches!(current.get("is_gui"), Some(TemplateValue::Null)));
                    assert_eq!(current.get_str("crepus_target"), "custom");
                    let Some(TemplateValue::List(items)) = current.get("items") else {
                        panic!("items");
                    };
                    assert_eq!(items.as_ptr(), items_pointer);
                    let Some(TemplateValue::Scope(scope)) = current.get("scope") else {
                        panic!("scope");
                    };
                    let Some(TemplateValue::Str(text)) = scope.get("text") else {
                        panic!("text");
                    };
                    assert_eq!(text.as_ptr(), text_pointer);
                    assert!(!eval_condition_bool(current, "is_tui || is_web"));
                    assert!(!panic, "deliberate render panic");
                });
            }));
            assert_eq!(result.is_err(), panic);
            assert_same_vars(&ctx, &before);
            assert_eq!(ctx.base_dir, before.base_dir);
            assert!(Arc::ptr_eq(&ctx.virtual_files, &before.virtual_files));
            assert!(Arc::ptr_eq(
                &ctx.slot.as_ref().unwrap().1,
                &before.slot.as_ref().unwrap().1
            ));
        }
        ctx.set("is_gui", true);
        assert!(with_gpui_target_in_place(&mut ctx, |current| {
            eval_condition_bool(current, "is_gui")
        }));
        ctx.vars.remove("is_gui");
        assert!(with_gpui_target_in_place(&mut ctx, |current| {
            eval_condition_bool(current, "is_gui")
        }));
        assert!(ctx.get("is_gui").is_none());
    }

    #[test]
    fn complete_in_place_context_preserves_every_override_type() {
        let mut ctx = TemplateContext::new();
        for (key, value) in GPUI_TARGET_KEYS.into_iter().zip([
            TemplateValue::Null,
            TemplateValue::Bool(false),
            TemplateValue::Int(42),
            TemplateValue::Float(1.5),
            TemplateValue::Str("custom".into()),
            TemplateValue::List(Vec::new()),
            TemplateValue::Scope(TemplateContext::new()),
            TemplateValue::Bool(true),
        ]) {
            ctx.set(key, value);
        }
        let before = ctx.clone();
        let pointer = &ctx as *const TemplateContext;
        with_gpui_target_in_place(&mut ctx, |current| {
            assert_eq!(current as *const TemplateContext, pointer);
            assert_same_vars(current, &before);
            assert!(matches!(with_gpui_target(current), Cow::Borrowed(_)));
        });
        assert_same_vars(&ctx, &before);
    }
}

#[cfg(test)]
mod include_target_tests {
    use super::*;

    #[test]
    fn includes_preserve_target_overrides_without_leaking_parent_variables() {
        let mut parent = with_gpui_target(&TemplateContext::new()).into_owned();
        parent.set("is_gui", false);
        parent.set("crepus_target", "custom");
        parent.set("crepus_platform", "preview");
        parent.set("private_parent_value", "secret");
        let child = with_gpui_target(&include_context(
            &parent,
            std::path::Path::new("child.crepus"),
        ))
        .into_owned();
        assert!(!eval_condition_bool(&child, "is_gui"));
        assert_eq!(value_to_str(child.get("crepus_target").unwrap()), "custom");
        assert_eq!(
            value_to_str(child.get("crepus_platform").unwrap()),
            "preview"
        );
        assert!(child.get("private_parent_value").is_none());
        assert_eq!(child.get_bool("is_macos"), parent.get_bool("is_macos"));
    }
}

#[cfg(test)]
mod attribute_id_tests {
    use super::*;

    #[test]
    fn attribute_ids_use_null_omission_and_scalar_string_coercion() {
        let mut ctx = TemplateContext::new();
        ctx.set("title", "Attribute 雪");
        for (expression, expected) in [
            ("null", None),
            ("undefined", None),
            ("false", Some("false")),
            ("42", Some("42")),
            ("title", Some("Attribute 雪")),
            ("\"\"", None),
            ("1 +", None),
        ] {
            let mut nodes = crepuscularity_core::parse_template("div").unwrap();
            let Node::Element(element) = &mut nodes[0] else {
                panic!("expected element")
            };
            element.bindings.push(crepuscularity_core::ast::Binding {
                origin: crepuscularity_core::ast::BindingOrigin::Attribute,
                prop: "id".into(),
                value: expression.into(),
            });
            assert_eq!(
                element_id(element, &ctx).as_deref(),
                expected,
                "{expression}"
            );
        }
    }
}
