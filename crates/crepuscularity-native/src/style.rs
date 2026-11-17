//! Maps Tailwind utility classes onto [`ViewStyle`] and stack alignment hints.
//!
//! Coverage targets Tailwind CSS v4. Skipped (no SwiftUI equivalent or irrelevant):
//! breakpoints, dark-mode, hover/focus/active pseudo-classes,
//! grid, transitions, animations.

use crepuscularity_core::context::TemplateContext;

use crepuscularity_core::tailwind::{
    parse_font_size_named, parse_opacity_percentage, parse_size_pt, parse_spacing_pt,
    resolve_arbitrary_css_color, resolve_css_color, SIZE_FIT,
};

use crate::{StackAxis, ViewStyle};

// ── Public re-exports ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub(crate) struct StackLayoutHints {
    pub style: ViewStyle,
    pub align_items: Option<String>,
    pub justify_content: Option<String>,
}

pub(crate) fn extract_stack_hints(
    classes: Vec<String>,
    ctx: Option<&TemplateContext>,
) -> StackLayoutHints {
    let mut hints = StackLayoutHints::default();
    for c in &classes {
        apply_layout_class(&mut hints, c, ctx);
    }
    hints.style.classes = classes;
    hints
}

pub(crate) fn extract_text_style(classes: Vec<String>, ctx: Option<&TemplateContext>) -> ViewStyle {
    extract_stack_hints(classes, ctx).style
}

pub(crate) fn scroll_axis(classes: &[String], layout_axis: StackAxis) -> Option<StackAxis> {
    let horizontal = classes
        .iter()
        .any(|class| matches!(class.as_str(), "overflow-x-scroll" | "overflow-x-auto"));
    let vertical = classes
        .iter()
        .any(|class| matches!(class.as_str(), "overflow-y-scroll" | "overflow-y-auto"));
    let generic = classes
        .iter()
        .any(|class| matches!(class.as_str(), "overflow-scroll" | "overflow-auto"));
    match (horizontal, vertical, generic) {
        (true, false, _) => Some(StackAxis::Row),
        (false, true, _) => Some(StackAxis::Column),
        (false, false, false) => None,
        // shortcut: View IR has one scroll axis; add a second axis when dual-axis native scrolling is supported.
        _ => Some(layout_axis),
    }
}

// ── Scale + colour parsing (canonical implementations live in core) ──────────

/// A spacing utility: its class prefix and the [`ViewStyle`] field it sets.
type SpacingArm = (&'static str, fn(&mut ViewStyle, f32));

/// Parse a spacing prefix: `"p-"` from `"p-4"` → `Some(16.0)`.
fn parse_prefix_spacing(class: &str, prefix: &str) -> Option<f32> {
    parse_spacing_pt(class.strip_prefix(prefix)?)
}

fn parse_braced_expr(s: &str) -> (bool, &str) {
    if s.starts_with('{') && s.ends_with('}') {
        return (true, s[1..s.len() - 1].trim());
    }
    (false, "")
}

// ── Main class appliers ──────────────────────────────────────────────────────

fn apply_layout_class(hints: &mut StackLayoutHints, class: &str, ctx: Option<&TemplateContext>) {
    // Skip pseudo-class variants
    if class.contains(':') {
        return;
    }

    let s = &mut hints.style;

    // Padding and margin: every arm differs only in (prefix, field).
    const SPACING: &[SpacingArm] = &[
        ("p-", |s, v| s.padding = Some(v)),
        ("px-", |s, v| s.padding_horizontal = Some(v)),
        ("py-", |s, v| s.padding_vertical = Some(v)),
        ("pt-", |s, v| s.padding_top = Some(v)),
        ("pb-", |s, v| s.padding_bottom = Some(v)),
        ("pl-", |s, v| s.padding_left = Some(v)),
        ("pr-", |s, v| s.padding_right = Some(v)),
        ("m-", |s, v| s.margin = Some(v)),
        ("mx-", |s, v| s.margin_horizontal = Some(v)),
        ("my-", |s, v| s.margin_vertical = Some(v)),
        ("mt-", |s, v| s.margin_top = Some(v)),
        ("mb-", |s, v| s.margin_bottom = Some(v)),
        ("ml-", |s, v| s.margin_left = Some(v)),
        ("mr-", |s, v| s.margin_right = Some(v)),
    ];
    for (prefix, set) in SPACING {
        let spacing_class = if prefix.starts_with('m') {
            class.strip_prefix('-').unwrap_or(class)
        } else {
            class
        };
        if let Some(v) = parse_prefix_spacing(spacing_class, prefix) {
            let v = if spacing_class != class { -v } else { v };
            if prefix.starts_with('p') && v < 0.0 {
                return;
            }
            set(s, v);
            return;
        }
    }

    // Width / height / size
    if let Some(rest) = class.strip_prefix("w-") {
        if let Some(v) = parse_size_pt(rest) {
            s.width = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("h-") {
        if let Some(v) = parse_size_pt(rest) {
            s.height = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("size-") {
        if let Some(v) = parse_size_pt(rest) {
            s.width = Some(v);
            s.height = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("min-w-") {
        if let Some(v) = parse_size_pt(rest) {
            s.min_width = Some(v.max(0.0));
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("max-w-") {
        if rest == "none" {
            s.max_width = Some(SIZE_FIT);
            return;
        }
        if let Some(v) = parse_size_pt(rest) {
            s.max_width = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("min-h-") {
        if let Some(v) = parse_size_pt(rest) {
            s.min_height = Some(v.max(0.0));
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("max-h-") {
        if let Some(v) = parse_size_pt(rest) {
            s.max_height = Some(v);
            return;
        }
    }

    // Aspect ratio
    match class {
        "aspect-square" => {
            s.aspect_ratio = Some(1.0);
            return;
        }
        "aspect-video" => {
            s.aspect_ratio = Some(16.0 / 9.0);
            return;
        }
        _ => {}
    }

    // Flex
    match class {
        "flex-1" => {
            s.flex_grow = Some(1.0);
            s.flex_shrink = Some(1.0);
            return;
        }
        "flex-auto" => {
            s.flex_grow = Some(1.0);
            s.flex_shrink = Some(1.0);
            return;
        }
        "flex-none" => {
            s.flex_grow = Some(0.0);
            s.flex_shrink = Some(0.0);
            return;
        }
        "flex-wrap" => {
            s.flex_wrap = Some(true);
            return;
        }
        "flex-nowrap" => {
            s.flex_wrap = Some(false);
            return;
        }
        "flex-row" => {
            s.flex_direction = Some("row".into());
            return;
        }
        "flex-col" => {
            s.flex_direction = Some("column".into());
            return;
        }
        "grow" | "flex-grow" => {
            s.flex_grow = Some(1.0);
            return;
        }
        "grow-0" => {
            s.flex_grow = Some(0.0);
            return;
        }
        "shrink" | "flex-shrink" => {
            s.flex_shrink = Some(1.0);
            return;
        }
        "shrink-0" => {
            s.flex_shrink = Some(0.0);
            return;
        }
        _ => {}
    }

    // Align self
    match class {
        "self-auto" => {
            s.align_self = Some("auto".into());
            return;
        }
        "self-start" => {
            s.align_self = Some("start".into());
            return;
        }
        "self-end" => {
            s.align_self = Some("end".into());
            return;
        }
        "self-center" => {
            s.align_self = Some("center".into());
            return;
        }
        "self-stretch" => {
            s.align_self = Some("stretch".into());
            return;
        }
        "self-baseline" => {
            s.align_self = Some("baseline".into());
            return;
        }
        _ => {}
    }

    // Alignment
    match class {
        "items-start" => {
            hints.align_items = Some("start".into());
            return;
        }
        "items-end" => {
            hints.align_items = Some("end".into());
            return;
        }
        "items-center" => {
            hints.align_items = Some("center".into());
            return;
        }
        "items-stretch" => {
            hints.align_items = Some("stretch".into());
            return;
        }
        "items-baseline" => {
            hints.align_items = Some("baseline".into());
            return;
        }
        "justify-start" => {
            hints.justify_content = Some("start".into());
            return;
        }
        "justify-end" => {
            hints.justify_content = Some("end".into());
            return;
        }
        "justify-center" => {
            hints.justify_content = Some("center".into());
            return;
        }
        "justify-between" => {
            hints.justify_content = Some("between".into());
            return;
        }
        "justify-around" => {
            hints.justify_content = Some("around".into());
            return;
        }
        "justify-evenly" => {
            hints.justify_content = Some("evenly".into());
            return;
        }
        _ => {}
    }

    // Overflow / visibility
    match class {
        "overflow-hidden" => {
            s.overflow_hidden = Some(true);
            return;
        }
        "overflow-visible" => {
            s.overflow_hidden = Some(false);
            return;
        }
        "hidden" => {
            s.hidden = Some(true);
            return;
        }
        "invisible" => {
            s.opacity = Some(0.0);
            return;
        } // invisible ≈ opacity 0
        _ => {}
    }

    // Border
    match class {
        "border" => {
            s.border_width = Some(1.0);
            return;
        }
        "border-0" => {
            s.border_width = Some(0.0);
            return;
        }
        "border-2" => {
            s.border_width = Some(2.0);
            return;
        }
        "border-4" => {
            s.border_width = Some(4.0);
            return;
        }
        "border-8" => {
            s.border_width = Some(8.0);
            return;
        }
        _ => {}
    }
    // border-[N]
    if let Some(rest) = class.strip_prefix("border-") {
        let width = if rest.starts_with('[') {
            parse_spacing_pt(rest)
        } else {
            rest.parse::<f32>().ok()
        };
        if let Some(width) = width.filter(|width| width.is_finite() && *width >= 0.0) {
            s.border_width = Some(width);
            return;
        }
        // Try as color
        if let Some(hex) = resolve_css_color(rest).or_else(|| resolve_arbitrary_css_color(rest)) {
            s.border_color = Some(hex);
            return;
        }
    }

    // Dynamic color via context expression
    if let Some(ctx) = ctx {
        if class.contains('{') {
            if let Some(rest) = class.strip_prefix("bg-") {
                let (ok, expr) = parse_braced_expr(rest);
                if ok {
                    let Ok(v) = crepuscularity_core::eval::eval_expr(expr, ctx) else {
                        return;
                    };
                    let color_str = crepuscularity_core::context::value_to_str(&v);
                    if let Some(hex) = resolve_css_color(&color_str) {
                        s.background_color = Some(hex);
                        return;
                    }
                }
            }
            if let Some(rest) = class.strip_prefix("text-") {
                let (ok, expr) = parse_braced_expr(rest);
                if ok {
                    let Ok(v) = crepuscularity_core::eval::eval_expr(expr, ctx) else {
                        return;
                    };
                    let color_str = crepuscularity_core::context::value_to_str(&v);
                    if let Some(hex) = resolve_css_color(&color_str) {
                        s.foreground_color = Some(hex);
                        return;
                    }
                }
            }
        }
    }

    // Background color
    if let Some(rest) = class.strip_prefix("bg-") {
        if let Some(hex) = resolve_css_color(rest).or_else(|| resolve_arbitrary_css_color(rest)) {
            s.background_color = Some(hex);
            return;
        }
    }

    // Fall through to text/typography (text-color, font-size, etc.)
    apply_presentation_class(s, class, ctx);
}

/// Class families that apply to any element. Both entry points route through
/// here so a family added for one cannot silently go missing from the other.
fn apply_presentation_class(s: &mut ViewStyle, class: &str, ctx: Option<&TemplateContext>) {
    if let Some(opacity) = class
        .strip_prefix("opacity-")
        .and_then(parse_opacity_percentage)
    {
        s.opacity = Some(opacity);
        return;
    }
    apply_text_class(s, class, ctx);
    apply_position_class(s, class);
    apply_transform_class(s, class);
    apply_shadow_class(s, class);
    apply_text_layout_class(s, class);
    apply_cursor_class(s, class);
    apply_user_select_class(s, class);
    apply_gradient_class(s, class);
    apply_object_class(s, class);
}

fn apply_text_class(s: &mut ViewStyle, class: &str, ctx: Option<&TemplateContext>) {
    if class.contains(':') {
        return;
    }

    if let Some(rest) = class.strip_prefix("text-") {
        if apply_text_color(s, rest, ctx) {
            return;
        }
        let size = parse_font_size_named(rest).or_else(|| {
            rest.starts_with('[')
                .then(|| parse_spacing_pt(rest))
                .flatten()
                .filter(|size| size.is_finite() && *size > 0.0)
        });
        if let Some(size) = size {
            s.font_size = Some(size);
            return;
        }
        if let Some(align) = parse_text_align(rest) {
            s.text_align = Some(align);
            return;
        }
    }

    if let Some(w) = parse_font_weight(class) {
        s.font_weight = Some(w);
        return;
    }

    if let Some(f) = parse_font_family(class) {
        s.font_family = Some(f);
        return;
    }

    if let Some(rest) = class.strip_prefix("leading-") {
        if let Some(lh) = parse_line_height(rest) {
            s.line_height = Some(lh);
            return;
        }
    }

    if let Some(rest) = class.strip_prefix("tracking-") {
        if let Some(ls) = parse_letter_spacing(rest) {
            s.letter_spacing = Some(ls);
            return;
        }
    }

    if let Some(tt) = parse_text_transform(class) {
        s.text_transform = Some(tt);
        return;
    }

    if apply_text_decoration(s, class) {
        return;
    }

    if let Some(r) = parse_border_radius(class) {
        s.corner_radius = Some(r);
    }
}

#[inline]
fn apply_text_color(s: &mut ViewStyle, rest: &str, ctx: Option<&TemplateContext>) -> bool {
    if let Some(hex) = resolve_css_color(rest).or_else(|| resolve_arbitrary_css_color(rest)) {
        s.foreground_color = Some(hex);
        return true;
    }
    if let Some(ctx) = ctx {
        let (ok, expr) = parse_braced_expr(rest);
        if ok {
            let Ok(v) = crepuscularity_core::eval::eval_expr(expr, ctx) else {
                return true;
            };
            let color_str = crepuscularity_core::context::value_to_str(&v);
            if let Some(hex) = resolve_css_color(&color_str) {
                s.foreground_color = Some(hex);
                return true;
            }
        }
    }
    false
}

#[inline]
fn parse_text_align(rest: &str) -> Option<String> {
    match rest {
        "left" => Some("leading".into()),
        "center" => Some("center".into()),
        "right" => Some("trailing".into()),
        _ => None,
    }
}

#[inline]
fn parse_font_weight(class: &str) -> Option<u16> {
    match class {
        "font-thin" => Some(100),
        "font-extralight" => Some(200),
        "font-light" => Some(300),
        "font-normal" => Some(400),
        "font-medium" => Some(500),
        "font-semibold" => Some(600),
        "font-bold" => Some(700),
        "font-extrabold" => Some(800),
        "font-black" => Some(900),
        _ => None,
    }
}

#[inline]
fn parse_font_family(class: &str) -> Option<String> {
    match class {
        "font-sans" => Some("sans".into()),
        "font-serif" => Some("serif".into()),
        "font-mono" => Some("mono".into()),
        _ => None,
    }
}

#[inline]
fn parse_line_height(rest: &str) -> Option<f32> {
    match rest {
        "none" => Some(1.0),
        "tight" => Some(1.25),
        "snug" => Some(1.375),
        "normal" => Some(1.5),
        "relaxed" => Some(1.625),
        "loose" => Some(2.0),
        _ => {
            if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let stripped = inner.strip_suffix("px").unwrap_or(inner);
                stripped
                    .parse::<f32>()
                    .ok()
                    .filter(|value| value.is_finite())
            } else {
                parse_spacing_pt(rest)
            }
        }
    }
}

#[inline]
fn parse_letter_spacing(rest: &str) -> Option<f32> {
    match rest {
        "tighter" => Some(-0.8),
        "tight" => Some(-0.4),
        "normal" => Some(0.0),
        "wide" => Some(0.4),
        "wider" => Some(0.8),
        "widest" => Some(1.6),
        _ => {
            if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let stripped = inner.strip_suffix("px").unwrap_or(inner);
                stripped
                    .parse::<f32>()
                    .ok()
                    .filter(|value| value.is_finite())
            } else {
                None
            }
        }
    }
}

#[inline]
fn parse_text_transform(class: &str) -> Option<String> {
    match class {
        "uppercase" => Some("uppercase".into()),
        "lowercase" => Some("lowercase".into()),
        "capitalize" => Some("capitalize".into()),
        "normal-case" => Some("normal".into()),
        _ => None,
    }
}

#[inline]
fn apply_text_decoration(s: &mut ViewStyle, class: &str) -> bool {
    match class {
        "italic" | "font-italic" => {
            s.italic = Some(true);
            true
        }
        "not-italic" => {
            s.italic = Some(false);
            true
        }
        "underline" => {
            s.underline = Some(true);
            true
        }
        "no-underline" => {
            s.underline = Some(false);
            true
        }
        "line-through" => {
            s.strikethrough = Some(true);
            true
        }
        "no-line-through" => {
            s.strikethrough = Some(false);
            true
        }
        _ => false,
    }
}

#[inline]
fn parse_border_radius(class: &str) -> Option<f32> {
    match class {
        "rounded-none" => Some(0.0),
        "rounded-sm" => Some(2.0),
        "rounded" | "rounded-md" => Some(6.0),
        "rounded-lg" => Some(8.0),
        "rounded-xl" => Some(12.0),
        "rounded-2xl" => Some(16.0),
        "rounded-3xl" => Some(24.0),
        "rounded-full" => Some(9999.0),
        _ => {
            if let Some(inner) = class
                .strip_prefix("rounded-[")
                .and_then(|s| s.strip_suffix(']'))
            {
                let stripped = inner.strip_suffix("px").unwrap_or(inner);
                stripped
                    .parse::<f32>()
                    .ok()
                    .filter(|value| value.is_finite())
            } else {
                None
            }
        }
    }
}

// ── New CSS property parsers ─────────────────────────────────────

// Position & layering
fn apply_position_class(s: &mut ViewStyle, class: &str) {
    match class {
        "absolute" => {
            s.position = Some("absolute".into());
            return;
        }
        "relative" => {
            s.position = Some("relative".into());
            return;
        }
        "fixed" => {
            s.position = Some("fixed".into());
            return;
        }
        "static" => {
            s.position = Some("static".into());
            return;
        }
        _ => {}
    }
    if let Some(rest) = class.strip_prefix("z-") {
        if let Ok(z) = rest.parse::<i32>() {
            s.z_index = Some(z);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("top-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.top = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("right-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.right = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("bottom-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.bottom = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("left-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.left = Some(v);
        }
    }
}

// Transform
fn apply_transform_class(s: &mut ViewStyle, class: &str) {
    if let Some(rest) = class.strip_prefix("translate-x-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.translate_x = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("translate-y-") {
        if let Some(v) = parse_spacing_pt(rest) {
            s.translate_y = Some(v);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("scale-") {
        if let Some(v) = rest.parse::<f32>().ok().filter(|value| value.is_finite()) {
            let value = if v > 10.0 { v / 100.0 } else { v };
            s.scale_x = Some(value);
            s.scale_y = Some(value);
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("scale-x-") {
        if let Some(v) = rest.parse::<f32>().ok().filter(|value| value.is_finite()) {
            s.scale_x = Some(if v > 10.0 { v / 100.0 } else { v });
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("scale-y-") {
        if let Some(v) = rest.parse::<f32>().ok().filter(|value| value.is_finite()) {
            s.scale_y = Some(if v > 10.0 { v / 100.0 } else { v });
            return;
        }
    }
    if let Some(rest) = class.strip_prefix("rotate-") {
        if let Some(v) = rest.parse::<f32>().ok().filter(|value| value.is_finite()) {
            s.rotate = Some(v);
        }
    }
}

// Shadow
fn apply_shadow_class(s: &mut ViewStyle, class: &str) {
    match class {
        "shadow-sm" => {
            s.shadow_radius = Some(2.0);
            s.shadow_offset_y = Some(1.0);
            return;
        }
        "shadow" => {
            s.shadow_radius = Some(4.0);
            s.shadow_offset_y = Some(2.0);
            return;
        }
        "shadow-md" => {
            s.shadow_radius = Some(6.0);
            s.shadow_offset_y = Some(3.0);
            return;
        }
        "shadow-lg" => {
            s.shadow_radius = Some(10.0);
            s.shadow_offset_y = Some(4.0);
            return;
        }
        "shadow-xl" => {
            s.shadow_radius = Some(15.0);
            s.shadow_offset_y = Some(6.0);
            return;
        }
        "shadow-2xl" => {
            s.shadow_radius = Some(25.0);
            s.shadow_offset_y = Some(8.0);
            return;
        }
        "shadow-none" => {
            s.shadow_radius = Some(0.0);
            return;
        }
        _ => {}
    }
    if let Some(rest) = class.strip_prefix("shadow-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(hex) = resolve_arbitrary_css_color(inner) {
                s.shadow_color = Some(hex);
            }
        }
    }
}

// Text overflow & white-space
fn apply_text_layout_class(s: &mut ViewStyle, class: &str) {
    match class {
        "truncate" => {
            s.text_overflow = Some("truncate".into());
            return;
        }
        "text-ellipsis" => {
            s.text_overflow = Some("ellipsis".into());
            return;
        }
        "text-clip" => {
            s.text_overflow = Some("clip".into());
            return;
        }
        "whitespace-normal" => {
            s.white_space = Some("normal".into());
            return;
        }
        "whitespace-nowrap" => {
            s.white_space = Some("nowrap".into());
            return;
        }
        "whitespace-pre" => {
            s.white_space = Some("pre".into());
            return;
        }
        "whitespace-pre-wrap" => {
            s.white_space = Some("pre-wrap".into());
            return;
        }
        _ => {}
    }
    if let Some(rest) = class.strip_prefix("line-clamp-") {
        if let Some(n) = rest.parse::<i32>().ok().filter(|n| *n > 0) {
            s.line_clamp = Some(n);
        }
    }
}

// Cursor
fn apply_cursor_class(s: &mut ViewStyle, class: &str) {
    match class {
        "cursor-auto" => {
            s.cursor = Some("auto".into());
        }
        "cursor-default" => {
            s.cursor = Some("default".into());
        }
        "cursor-pointer" => {
            s.cursor = Some("pointer".into());
        }
        "cursor-text" => {
            s.cursor = Some("text".into());
        }
        "cursor-move" => {
            s.cursor = Some("move".into());
        }
        "cursor-wait" => {
            s.cursor = Some("wait".into());
        }
        _ => {}
    }
}

// User select
fn apply_user_select_class(s: &mut ViewStyle, class: &str) {
    match class {
        "select-none" => {
            s.user_select = Some("none".into());
        }
        "select-text" => {
            s.user_select = Some("text".into());
        }
        "select-all" => {
            s.user_select = Some("all".into());
        }
        "select-auto" => {
            s.user_select = Some("auto".into());
        }
        _ => {}
    }
}

// Gradients
fn apply_gradient_class(s: &mut ViewStyle, class: &str) {
    if let Some(rest) = class.strip_prefix("bg-gradient-to-") {
        if matches!(rest, "r" | "l" | "t" | "b" | "tr" | "tl" | "br" | "bl") {
            s.background_gradient_direction = Some(format!("to-{rest}"));
            return;
        }
    }

    if let Some(rest) = class.strip_prefix("from-") {
        if let Some(color) = resolve_css_color(rest).or_else(|| resolve_arbitrary_css_color(rest)) {
            s.background_gradient_from = Some(color);
            return;
        }
    }

    if let Some(rest) = class.strip_prefix("to-") {
        if let Some(color) = resolve_css_color(rest).or_else(|| resolve_arbitrary_css_color(rest)) {
            s.background_gradient_to = Some(color);
        }
    }
}

// Object fit / object position
fn apply_object_class(s: &mut ViewStyle, class: &str) {
    match class {
        "object-contain" => {
            s.object_fit = Some("contain".into());
            return;
        }
        "object-cover" => {
            s.object_fit = Some("cover".into());
            return;
        }
        "object-fill" => {
            s.object_fit = Some("fill".into());
            return;
        }
        "object-none" => {
            s.object_fit = Some("none".into());
            return;
        }
        "object-scale-down" => {
            s.object_fit = Some("scale-down".into());
            return;
        }
        _ => {}
    }

    match class {
        "object-center" => s.object_position = Some("center".into()),
        "object-top" => s.object_position = Some("top".into()),
        "object-bottom" => s.object_position = Some("bottom".into()),
        "object-left" => s.object_position = Some("left".into()),
        "object-right" => s.object_position = Some("right".into()),
        "object-left-top" => s.object_position = Some("left-top".into()),
        "object-left-bottom" => s.object_position = Some("left-bottom".into()),
        "object-right-top" => s.object_position = Some("right-top".into()),
        "object-right-bottom" => s.object_position = Some("right-bottom".into()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_core::tailwind::SIZE_FILL;

    fn layout(class: &str) -> ViewStyle {
        extract_stack_hints(vec![class.to_string()], None).style
    }

    #[test]
    fn scroll_direction_is_independent_of_layout_and_generic_overflow_keeps_the_fallback() {
        for fallback in [StackAxis::Row, StackAxis::Column] {
            for (classes, expected) in [
                ("overflow-x-auto", Some(StackAxis::Row)),
                ("overflow-x-scroll", Some(StackAxis::Row)),
                ("overflow-y-auto", Some(StackAxis::Column)),
                ("overflow-y-scroll", Some(StackAxis::Column)),
                ("overflow-auto", Some(fallback)),
                ("overflow-scroll", Some(fallback)),
                ("overflow-x-auto overflow-y-auto", Some(fallback)),
                ("overflow-auto overflow-y-scroll", Some(StackAxis::Column)),
                ("overflow-hidden", None),
                ("overflow-visible", None),
                ("md:overflow-auto", None),
                ("", None),
            ] {
                let classes = classes
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                assert_eq!(scroll_axis(&classes, fallback), expected, "{classes:?}");
            }
        }
    }

    #[test]
    fn spacing_table_covers_every_padding_and_margin_arm() {
        assert_eq!(layout("p-4").padding, Some(16.0));
        assert_eq!(layout("px-2").padding_horizontal, Some(8.0));
        assert_eq!(layout("py-2").padding_vertical, Some(8.0));
        assert_eq!(layout("pt-1").padding_top, Some(4.0));
        assert_eq!(layout("pb-1").padding_bottom, Some(4.0));
        assert_eq!(layout("pl-1").padding_left, Some(4.0));
        assert_eq!(layout("pr-1").padding_right, Some(4.0));
        assert_eq!(layout("m-4").margin, Some(16.0));
        assert_eq!(layout("mx-2").margin_horizontal, Some(8.0));
        assert_eq!(layout("my-2").margin_vertical, Some(8.0));
        assert_eq!(layout("mt-1").margin_top, Some(4.0));
        assert_eq!(layout("mb-1").margin_bottom, Some(4.0));
        assert_eq!(layout("ml-1").margin_left, Some(4.0));
        assert_eq!(layout("mr-1").margin_right, Some(4.0));
    }

    #[test]
    fn spacing_accepts_half_steps_and_arbitrary_values() {
        assert_eq!(layout("p-0").padding, Some(0.0));
        assert_eq!(layout("p-0.5").padding, Some(2.0));
        assert_eq!(layout("p-1.5").padding, Some(6.0));
        assert_eq!(layout("p-0.25").padding, Some(1.0));
        assert_eq!(layout("p-4.5").padding, Some(18.0));
        assert_eq!(layout("w-4.75").width, Some(19.0));
        assert_eq!(layout("-m-4.5").margin, Some(-18.0));
        for class in ["p-0.3", "p-01", "p-1.50", "p--2"] {
            assert_eq!(layout(class).padding, None, "{class}");
        }
        assert_eq!(layout("p-px").padding, Some(1.0));
        assert_eq!(layout("p-[20px]").padding, Some(20.0));
        assert_eq!(layout("p-[20]").padding, Some(20.0));
    }

    #[test]
    fn size_sentinels_are_stable() {
        assert_eq!(layout("w-full").width, Some(SIZE_FILL));
        assert_eq!(layout("w-screen").width, Some(SIZE_FILL));
        assert_eq!(layout("h-full").height, Some(SIZE_FILL));
        for fit in ["w-fit", "w-auto", "w-min", "w-max"] {
            assert_eq!(layout(fit).width, Some(SIZE_FIT), "{fit}");
        }
        assert_eq!(layout("max-w-none").max_width, Some(SIZE_FIT));
        // A fractional width has no point value and must not land in the same
        // field as the sentinels; web targets read it from the class token.
        assert_eq!(layout("w-1/2").width, None);
        assert_eq!(layout("max-w-1/1").max_width, None);
        assert_eq!(layout("w-8").width, Some(32.0));
        assert_eq!(layout("size-8").width, Some(32.0));
        assert_eq!(layout("size-8").height, Some(32.0));
    }

    #[test]
    fn colors_resolve_through_core() {
        assert_eq!(
            layout("bg-red-500").background_color.as_deref(),
            Some("#fb2c36")
        );
        assert_eq!(
            layout("bg-white").background_color.as_deref(),
            Some("#ffffff")
        );
        assert_eq!(
            layout("bg-transparent").background_color.as_deref(),
            Some("#00000000")
        );
        assert_eq!(
            layout("bg-red-500/50").background_color.as_deref(),
            Some("#fb2c3680")
        );
        assert_eq!(
            layout("bg-[#123456]").background_color.as_deref(),
            Some("#123456")
        );
        assert_eq!(
            layout("text-slate-900").foreground_color.as_deref(),
            Some("#0f172b")
        );
    }

    #[test]
    fn font_size_ramp_stays_strict() {
        assert_eq!(layout("text-xs").font_size, Some(12.0));
        assert_eq!(layout("text-9xl").font_size, Some(128.0));
        // Bare scale numbers are not font sizes; bracketed pixels are explicit.
        assert_eq!(layout("text-4").font_size, None);
        assert_eq!(layout("text-[22px]").font_size, Some(22.0));
        // text-left/center/right must not be swallowed by the size ramp.
        assert_eq!(layout("text-center").text_align.as_deref(), Some("center"));
    }
}

#[cfg(test)]
mod opacity_parity_tests {
    use super::*;
    #[test]
    fn text_and_stack_opacity_share_finite_clamped_percentage_semantics() {
        for (token, expected) in [
            ("opacity-12.5", Some(0.125)),
            ("opacity--20", Some(0.0)),
            ("opacity-120", Some(1.0)),
            ("opacity-NaN", None),
            ("opacity-inf", None),
        ] {
            let classes = vec![token.to_owned()];
            assert_eq!(extract_text_style(classes.clone(), None).opacity, expected);
            assert_eq!(extract_stack_hints(classes, None).style.opacity, expected);
        }
    }
}

#[cfg(test)]
mod text_box_style_tests {
    use super::*;
    #[test]
    fn text_preserves_common_box_styles_and_class_order() {
        let classes = [
            "p-1.5",
            "px-2",
            "bg-red-500",
            "bg-blue-500",
            "border-2",
            "border-red-500",
            "rounded-lg",
            "opacity-25",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let text = extract_text_style(classes.clone(), None);
        let stack = extract_stack_hints(classes.clone(), None).style;
        assert_eq!(text.padding, Some(6.0));
        assert_eq!(text.padding_horizontal, Some(8.0));
        assert_eq!(text.background_color, stack.background_color);
        assert_eq!(text.background_color, resolve_css_color("blue-500"));
        assert_eq!(text.border_width, Some(2.0));
        assert_eq!(text.border_color, resolve_css_color("red-500"));
        assert_eq!(text.corner_radius, stack.corner_radius);
        assert_eq!(text.opacity, Some(0.25));
        assert_eq!(text.classes, classes);
    }
}
