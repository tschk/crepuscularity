use gpui::TextTransform;
/// Runtime Tailwind class → GPUI style applicator.
///
/// Supports:
/// - Static Tailwind classes: `flex`, `bg-red-500`, `p-4`
/// - Arbitrary values: `bg-[#ff5733]`, `w-[200px]`, `text-[14px]`
/// - Dynamic context expressions: `bg-{theme.surface}`, `text-{colors.muted}`
///   where the expression evaluates to a hex color string like "#1e1e2e" or "1e1e2e"
use gpui::{
    hsla, point, prelude::*, px, relative, rems, rgb, rgba, AbsoluteLength, AlignItems, BoxShadow,
    DefiniteLength, Div, Length,
};

use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::tailwind::{
    parse_arbitrary_length_token, parse_css_hex_color, parse_length_token,
    parse_opacity_percentage, resolve_css_color, LengthToken,
};

/// Apply a class to a div, optionally resolving `{expr}` placeholders against context.
pub fn apply_class(d: Div, class: &str) -> Div {
    apply_class_with_ctx(d, class, None)
}

/// Apply a runtime utility list to any styled GPUI element.
pub fn apply_class_list<E: gpui::Styled>(mut element: E, classes: impl AsRef<str>) -> E {
    let mut styled = gpui::div();
    *styled.style() = std::mem::take(element.style());
    for class in classes.as_ref().split_whitespace() {
        styled = apply_class(styled, class);
    }
    *element.style() = std::mem::take(styled.style());
    element
}

/// Whether a state list operates on the shared ring/shadow collection.
pub fn class_list_has_state_shadow(classes: &str, state: &str) -> bool {
    classes.split_whitespace().any(|mut class| {
        while let Some(rest) = class
            .strip_prefix("gpui:")
            .or_else(|| class.strip_prefix("gui:"))
        {
            class = rest;
        }
        class.split_once(':').is_some_and(|(prefix, mut utility)| {
            if prefix != state {
                return false;
            }
            while let Some(rest) = utility
                .strip_prefix("gpui:")
                .or_else(|| utility.strip_prefix("gui:"))
            {
                utility = rest;
            }
            is_shadow_class(utility)
        })
    })
}

/// Resolve one state from a runtime utility list for generated GPUI views.
pub fn class_list_state_style(classes: &str, state: &str) -> gpui::StyleRefinement {
    state_style_with_ctx(classes.split_whitespace(), state, None)
}

/// Apply matching state utilities in order, including resets of earlier utilities.
pub fn apply_class_list_state_style(
    style: gpui::StyleRefinement,
    classes: &str,
    state: &str,
) -> gpui::StyleRefinement {
    state_style_with_initial(style, classes.split_whitespace(), state, None, None)
}

pub fn state_style_with_ctx<'a>(
    classes: impl Iterator<Item = &'a str>,
    state: &str,
    ctx: Option<&TemplateContext>,
) -> gpui::StyleRefinement {
    state_style_with_initial(gpui::StyleRefinement::default(), classes, state, ctx, None)
}

pub(crate) fn state_style_with_initial<'a>(
    style: gpui::StyleRefinement,
    classes: impl Iterator<Item = &'a str>,
    state: &str,
    ctx: Option<&TemplateContext>,
    base: Option<&gpui::StyleRefinement>,
) -> gpui::StyleRefinement {
    let mut styled = gpui::div();
    *styled.style() = style;
    for mut class in classes {
        while let Some(rest) = class
            .strip_prefix("gpui:")
            .or_else(|| class.strip_prefix("gui:"))
        {
            class = rest;
        }
        if let Some((prefix, utility)) = class.split_once(':') {
            if prefix == state {
                styled = apply_class_with_ctx_and_base(styled, utility, ctx, base);
            }
        }
    }
    std::mem::take(styled.style())
}

/// Apply a class with optional template context for dynamic value resolution.
pub fn apply_class_with_ctx(d: Div, class: &str, ctx: Option<&TemplateContext>) -> Div {
    apply_class_with_ctx_and_base(d, class, ctx, None)
}

fn apply_class_with_ctx_and_base(
    mut d: Div,
    mut class: &str,
    ctx: Option<&TemplateContext>,
    base: Option<&gpui::StyleRefinement>,
) -> Div {
    while let Some((prefix, rest)) = class.split_once(':') {
        match prefix {
            "gpui" | "gui" => class = rest,
            "tui" | "web" | "ios" | "android" | "macos" | "darwin" | "windows" | "win32"
            | "linux" => return d,
            _ => break,
        }
    }
    if class.starts_with('{') && class.ends_with('}') {
        if let Some(ctx) = ctx {
            if let Ok(classes) = ctx.interpolate(class) {
                return classes.split_whitespace().fold(d, |d, class| {
                    apply_class_with_ctx_and_base(d, class, None, base)
                });
            }
        }
        return d;
    }

    // The renderer applies state variants separately from the base style.
    if class.starts_with("hover:") || class.starts_with("focus:") || class.starts_with("active:") {
        return d;
    }

    // Resolve embedded expressions such as bg-{color} or w-{size}.
    if let Some(ctx) = ctx {
        if class.contains('{') {
            return apply_context_class(d, class, ctx, base);
        }
    }

    if let Some(base) = base {
        if is_shadow_class(class) && d.style().box_shadow.is_none() {
            d.style().box_shadow = base.box_shadow.clone();
        }
    }
    apply_base_class(d, class)
}

pub fn apply_base_class(d: Div, class: &str) -> Div {
    match apply_static(d, class) {
        Ok(d) => d,
        Err(d) => apply_dynamic(d, class),
    }
}

/// Try to resolve a class containing `{expr}` against the template context.
/// Falls through to `apply_base_class` if no context expression matched.
fn apply_context_class(
    d: Div,
    class: &str,
    ctx: &TemplateContext,
    base: Option<&gpui::StyleRefinement>,
) -> Div {
    // Pattern: prefix-{expr} where prefix is bg, text, border, etc.
    for prefix in ["bg-", "text-", "border-"] {
        if let Some(rest) = class.strip_prefix(prefix) {
            if rest.starts_with('{') && rest.ends_with('}') {
                let expr = &rest[1..rest.len() - 1];
                let Ok(val) = crepuscularity_core::eval::eval_expr(expr, ctx) else {
                    continue;
                };
                let color_str = crepuscularity_core::context::value_to_str(&val);
                if let Some(color) = parse_runtime_hex_color(&color_str) {
                    return apply_color(d, prefix, color);
                }
            }
        }
    }

    // bg-{expr}/alpha pattern for RGBA: bg-{expr}/20
    if let Some(rest) = class.strip_prefix("bg-") {
        if let Some((expr_part, alpha_str)) = rest.rsplit_once('/') {
            if expr_part.starts_with('{') && expr_part.ends_with('}') {
                let expr = &expr_part[1..expr_part.len() - 1];
                if let Ok(val) = crepuscularity_core::eval::eval_expr(expr, ctx) {
                    let color_str = crepuscularity_core::context::value_to_str(&val);
                    if let Some(normalized) = parse_css_hex_color(&color_str) {
                        if let (Ok(hex), Some(alpha)) = (
                            u32::from_str_radix(&normalized[1..7], 16),
                            parse_opacity_percentage(alpha_str),
                        ) {
                            let rgba_val = (hex << 8) | (alpha * 255.0) as u32;
                            return apply_rgba(d, "bg-", rgba_val);
                        }
                    }
                }
            }
        }
    }

    // opacity-{expr}
    if let Some(rest) = class.strip_prefix("opacity-") {
        if rest.starts_with('{') && rest.ends_with('}') {
            let expr = &rest[1..rest.len() - 1];
            let Ok(val) = crepuscularity_core::eval::eval_expr(expr, ctx) else {
                return apply_base_class(d, class);
            };
            let s = crepuscularity_core::context::value_to_str(&val);
            if let Some(opacity) = parse_opacity_percentage(&s) {
                return d.opacity(opacity);
            }
        }
    }

    // Resolved values are classes, not another interpolation pass.
    if let Ok(resolved) = ctx.interpolate(class) {
        return resolved.split_whitespace().fold(d, |d, class| {
            apply_class_with_ctx_and_base(d, class, None, base)
        });
    }

    apply_base_class(d, class)
}

/// Returns Ok(div) if class matched, Err(div) to fall through to dynamic.
#[allow(clippy::result_large_err)]
fn apply_static(d: Div, class: &str) -> Result<Div, Div> {
    apply_layout(d, class)
        .or_else(|d| apply_colors(d, class))
        .or_else(|d| apply_typography(d, class))
        .or_else(|d| apply_borders(d, class))
        .or_else(|d| apply_shadows(d, class))
        .or_else(|d| apply_misc(d, class))
}

#[allow(clippy::result_large_err)]
fn apply_layout(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Display ──
        "block" => d.block(),
        "flex" => d.flex(),
        "grid" => d.grid(),
        "hidden" => d.hidden(),

        // ── Visibility ──
        "visible" => d.visible(),
        "invisible" => d.invisible(),

        // ── Position ──
        "absolute" => d.absolute(),
        "relative" => d.relative(),

        // ── Overflow ──
        "overflow-hidden" => d.overflow_hidden(),
        "overflow-x-hidden" => d.overflow_x_hidden(),
        "overflow-y-hidden" => d.overflow_y_hidden(),
        "overflow-visible" | "overflow-x-visible" | "overflow-y-visible" => {
            let mut d = d;
            if class != "overflow-y-visible" {
                d.style().overflow.x = Some(gpui::Overflow::Visible);
            }
            if class != "overflow-x-visible" {
                d.style().overflow.y = Some(gpui::Overflow::Visible);
            }
            d
        }
        // The renderer gives scrolling elements stable IDs.
        "overflow-scroll" | "overflow-auto" | "overflow-y-scroll" | "overflow-y-auto"
        | "overflow-x-scroll" | "overflow-x-auto" => {
            let mut d = d;
            if !class.starts_with("overflow-y-") {
                d.style().overflow.x = Some(gpui::Overflow::Scroll);
            }
            if !class.starts_with("overflow-x-") {
                d.style().overflow.y = Some(gpui::Overflow::Scroll);
            }
            d
        }

        // ── Flex direction ──
        "flex-row" => d.flex_row(),
        "flex-row-reverse" => d.flex_row_reverse(),
        "flex-col" => d.flex_col(),
        "flex-col-reverse" => d.flex_col_reverse(),

        // ── Flex wrap ──
        "flex-wrap" => d.flex_wrap(),
        "flex-wrap-reverse" => d.flex_wrap_reverse(),
        "flex-nowrap" => d.flex_nowrap(),

        // ── Flex sizing ──
        "flex-1" => d.flex_1(),
        "flex-auto" => d.flex_auto(),
        "flex-initial" => d.flex_initial(),
        "flex-none" => d.flex_none(),
        "grow" => d.flex_grow(1.0),
        "grow-0" => d.flex_none(),
        "shrink" => d.flex_shrink(1.0),
        "shrink-0" => d.flex_shrink_0(),

        // ── Justify content ──
        "justify-start" => d.justify_start(),
        "justify-end" => d.justify_end(),
        "justify-center" => d.justify_center(),
        "justify-between" => d.justify_between(),
        "justify-around" => d.justify_around(),
        "justify-evenly" => d.justify_evenly(),

        // ── Align items ──
        "items-start" => d.items_start(),
        "items-end" => d.items_end(),
        "items-center" => d.items_center(),
        "items-baseline" => d.items_baseline(),
        "items-stretch" => d.items_stretch(),

        // ── Align content ──
        "content-normal" => d.content_normal(),
        "content-center" => d.content_center(),
        "content-start" => d.content_start(),
        "content-end" => d.content_end(),
        "content-between" => d.content_between(),
        "content-around" => d.content_around(),
        "content-evenly" => d.content_evenly(),
        "content-stretch" => d.content_stretch(),

        // ── Sizing (predefined) ──
        "w-0" => d.w(px(0.)),
        "w-px" => d.w_px(),
        "w-full" => d.w_full(),
        "w-auto" => d.w_auto(),
        "h-0" => d.h(px(0.)),
        "h-px" => d.h_px(),
        "h-full" => d.h_full(),
        "h-auto" => d.h_auto(),
        "size-full" => d.size_full(),
        "size-auto" => d.size_auto(),
        "min-w-0" => d.min_w(px(0.)),
        "min-w-full" => d.min_w_full(),
        "min-h-0" => d.min_h(px(0.)),
        "min-h-full" => d.min_h_full(),
        "max-w-full" => d.max_w_full(),
        "max-h-full" => d.max_h_full(),

        // ── Grid ──
        "grid-cols-1" => d.grid_cols(1),
        "grid-cols-2" => d.grid_cols(2),
        "grid-cols-3" => d.grid_cols(3),
        "grid-cols-4" => d.grid_cols(4),
        "grid-cols-5" => d.grid_cols(5),
        "grid-cols-6" => d.grid_cols(6),
        "grid-cols-7" => d.grid_cols(7),
        "grid-cols-8" => d.grid_cols(8),
        "grid-cols-9" => d.grid_cols(9),
        "grid-cols-10" => d.grid_cols(10),
        "grid-cols-11" => d.grid_cols(11),
        "grid-cols-12" => d.grid_cols(12),
        "grid-rows-1" => d.grid_rows(1),
        "grid-rows-2" => d.grid_rows(2),
        "grid-rows-3" => d.grid_rows(3),
        "grid-rows-4" => d.grid_rows(4),
        "grid-rows-5" => d.grid_rows(5),
        "grid-rows-6" => d.grid_rows(6),
        "col-span-full" => d.col_span_full(),
        "col-start-auto" => d.col_start_auto(),
        "col-end-auto" => d.col_end_auto(),
        "row-span-full" => d.row_span_full(),
        "row-start-auto" => d.row_start_auto(),
        "row-end-auto" => d.row_end_auto(),

        // ── Align self ──
        "self-start" => {
            let mut d = d;
            d.style().align_self = Some(AlignItems::Start);
            d
        }
        "self-end" => {
            let mut d = d;
            d.style().align_self = Some(AlignItems::End);
            d
        }
        "self-center" => {
            let mut d = d;
            d.style().align_self = Some(AlignItems::Center);
            d
        }
        "self-stretch" => {
            let mut d = d;
            d.style().align_self = Some(AlignItems::Stretch);
            d
        }
        "self-baseline" => {
            let mut d = d;
            d.style().align_self = Some(AlignItems::Baseline);
            d
        }
        "self-auto" => {
            let mut d = d;
            d.style().align_self = None;
            d
        }

        // ── Aspect ratio ──
        "aspect-square" => {
            let mut d = d;
            d.style().aspect_ratio = Some(1.0);
            d
        }
        "aspect-video" => {
            let mut d = d;
            d.style().aspect_ratio = Some(16.0 / 9.0);
            d
        }
        "aspect-auto" => {
            let mut d = d;
            d.style().aspect_ratio = None;
            d
        }
        _ => return Err(d),
    })
}

#[allow(clippy::result_large_err)]
fn apply_colors(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Named colors — background ──
        "bg-black" => d.bg(gpui::black()),
        "bg-white" => d.bg(gpui::white()),
        "bg-transparent" => d.bg(gpui::transparent_black()),
        "bg-red" => d.bg(gpui::red()),
        "bg-green" => d.bg(gpui::green()),
        "bg-blue" => d.bg(gpui::blue()),
        "bg-yellow" => d.bg(gpui::yellow()),

        // ── Named colors — text ──
        "text-white" => d.text_color(gpui::white()),
        "text-black" => d.text_color(gpui::black()),
        "text-transparent" => d.text_color(gpui::transparent_black()),
        "text-red" => d.text_color(gpui::red()),
        "text-green" => d.text_color(gpui::green()),
        "text-blue" => d.text_color(gpui::blue()),
        "text-yellow" => d.text_color(gpui::yellow()),

        // ── Named colors — border ──
        "border-white" => d.border_color(gpui::white()),
        "border-black" => d.border_color(gpui::black()),
        "border-transparent" => d.border_color(gpui::transparent_black()),
        _ => return Err(d),
    })
}

#[allow(clippy::result_large_err)]
fn apply_typography(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Typography — weight ──
        "font-thin" => d.font_weight(gpui::FontWeight::THIN),
        "font-light" => d.font_weight(gpui::FontWeight::LIGHT),
        "font-normal" => d.font_weight(gpui::FontWeight::NORMAL),
        "font-medium" => d.font_weight(gpui::FontWeight::MEDIUM),
        "font-semibold" => d.font_weight(gpui::FontWeight::SEMIBOLD),
        "font-bold" => d.font_weight(gpui::FontWeight::BOLD),
        "font-extrabold" => d.font_weight(gpui::FontWeight::EXTRA_BOLD),
        "font-black" => d.font_weight(gpui::FontWeight::BLACK),

        // ── Typography — style ──
        "italic" | "font-italic" => d.italic(),
        "not-italic" => d.not_italic(),

        // ── Typography — size ──
        "text-xs" => d.text_xs(),
        "text-sm" => d.text_sm(),
        "text-base" => d.text_base(),
        "text-lg" => d.text_lg(),
        "text-xl" => d.text_xl(),
        "text-2xl" => d.text_2xl(),
        "text-3xl" => d.text_3xl(),
        // 4xl-9xl: GPUI only has up to text_3xl, use text_size for larger
        "text-4xl" => d.text_size(rems(2.25)),
        "text-5xl" => d.text_size(rems(3.)),
        "text-6xl" => d.text_size(rems(3.75)),
        "text-7xl" => d.text_size(rems(4.5)),
        "text-8xl" => d.text_size(rems(6.)),
        "text-9xl" => d.text_size(rems(8.)),

        // ── Typography — alignment ──
        "text-left" => d.text_left(),
        "text-center" => d.text_center(),
        "text-right" => d.text_right(),

        // ── Typography — decoration ──
        "underline" => d.underline(),
        "line-through" => d.line_through(),
        "no-underline" => d.text_decoration_none(),
        "decoration-solid" => d.text_decoration_solid(),
        "decoration-wavy" => d.text_decoration_wavy(),
        "decoration-0" => d.text_decoration_0(),
        "decoration-1" => d.text_decoration_1(),
        "decoration-2" => d.text_decoration_2(),
        "decoration-4" => d.text_decoration_4(),
        "decoration-8" => d.text_decoration_8(),

        // ── Typography — line height ──
        "leading-none" => d.line_height(relative(1.)),
        "leading-tight" => d.line_height(relative(1.25)),
        "leading-snug" => d.line_height(relative(1.375)),
        "leading-normal" => d.line_height(relative(1.5)),
        "leading-relaxed" => d.line_height(relative(1.625)),
        "leading-loose" => d.line_height(relative(2.)),

        // ── Typography — overflow ──
        "truncate" => d.truncate(),
        "text-ellipsis" => d.text_ellipsis(),
        "whitespace-nowrap" => d.whitespace_nowrap(),
        "whitespace-normal" => d.whitespace_normal(),

        // ── Text transform (gpui-ce provides TextTransform natively) ──
        "uppercase" => d.text_transform(TextTransform::Uppercase),
        "lowercase" => d.text_transform(TextTransform::Lowercase),
        "capitalize" => d.text_transform(TextTransform::Capitalize),
        "normal-case" => d.text_transform(TextTransform::None),
        _ => return Err(d),
    })
}

#[allow(clippy::result_large_err)]
fn apply_borders(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Border style ──
        "border-dashed" => d.border_dashed(),

        // ── Border width ──
        "border" => d.border_1(),
        "border-0" => d.border_0(),
        "border-2" => d.border_2(),
        "border-4" => d.border_4(),
        "border-8" => d.border_8(),
        // Per-side border width
        "border-t" => d.border_t_1(),
        "border-t-0" => d.border_t_0(),
        "border-t-2" => d.border_t_2(),
        "border-t-4" => d.border_t_4(),
        "border-t-8" => d.border_t_8(),
        "border-b" => d.border_b_1(),
        "border-b-0" => d.border_b_0(),
        "border-b-2" => d.border_b_2(),
        "border-b-4" => d.border_b_4(),
        "border-b-8" => d.border_b_8(),
        "border-l" => d.border_l_1(),
        "border-l-0" => d.border_l_0(),
        "border-l-2" => d.border_l_2(),
        "border-l-4" => d.border_l_4(),
        "border-l-8" => d.border_l_8(),
        "border-r" => d.border_r_1(),
        "border-r-0" => d.border_r_0(),
        "border-r-2" => d.border_r_2(),
        "border-r-4" => d.border_r_4(),
        "border-r-8" => d.border_r_8(),

        // ── Border radius ──
        "rounded-none" => d.rounded_none(),
        "rounded-sm" => d.rounded_sm(),
        "rounded" | "rounded-md" => d.rounded_md(),
        "rounded-lg" => d.rounded_lg(),
        "rounded-xl" => d.rounded_xl(),
        "rounded-2xl" => d.rounded_2xl(),
        "rounded-3xl" => d.rounded_3xl(),
        "rounded-full" => d.rounded_full(),
        // Per-side radius
        "rounded-t-none" => d.rounded_t_none(),
        "rounded-t-sm" => d.rounded_t_sm(),
        "rounded-t" | "rounded-t-md" => d.rounded_t_md(),
        "rounded-t-lg" => d.rounded_t_lg(),
        "rounded-t-xl" => d.rounded_t_xl(),
        "rounded-t-2xl" => d.rounded_t_2xl(),
        "rounded-t-3xl" => d.rounded_t_3xl(),
        "rounded-t-full" => d.rounded_t_full(),
        "rounded-b-none" => d.rounded_b_none(),
        "rounded-b-sm" => d.rounded_b_sm(),
        "rounded-b" | "rounded-b-md" => d.rounded_b_md(),
        "rounded-b-lg" => d.rounded_b_lg(),
        "rounded-b-xl" => d.rounded_b_xl(),
        "rounded-b-2xl" => d.rounded_b_2xl(),
        "rounded-b-3xl" => d.rounded_b_3xl(),
        "rounded-b-full" => d.rounded_b_full(),
        "rounded-l-none" => d.rounded_l_none(),
        "rounded-l-sm" => d.rounded_l_sm(),
        "rounded-l" | "rounded-l-md" => d.rounded_l_md(),
        "rounded-l-lg" => d.rounded_l_lg(),
        "rounded-l-xl" => d.rounded_l_xl(),
        "rounded-l-2xl" => d.rounded_l_2xl(),
        "rounded-l-3xl" => d.rounded_l_3xl(),
        "rounded-l-full" => d.rounded_l_full(),
        "rounded-r-none" => d.rounded_r_none(),
        "rounded-r-sm" => d.rounded_r_sm(),
        "rounded-r" | "rounded-r-md" => d.rounded_r_md(),
        "rounded-r-lg" => d.rounded_r_lg(),
        "rounded-r-xl" => d.rounded_r_xl(),
        "rounded-r-2xl" => d.rounded_r_2xl(),
        "rounded-r-3xl" => d.rounded_r_3xl(),
        "rounded-r-full" => d.rounded_r_full(),
        // Per-corner radius
        "rounded-tl-none" => d.rounded_tl_none(),
        "rounded-tl-sm" => d.rounded_tl_sm(),
        "rounded-tl" | "rounded-tl-md" => d.rounded_tl_md(),
        "rounded-tl-lg" => d.rounded_tl_lg(),
        "rounded-tl-xl" => d.rounded_tl_xl(),
        "rounded-tl-2xl" => d.rounded_tl_2xl(),
        "rounded-tl-3xl" => d.rounded_tl_3xl(),
        "rounded-tl-full" => d.rounded_tl_full(),
        "rounded-tr-none" => d.rounded_tr_none(),
        "rounded-tr-sm" => d.rounded_tr_sm(),
        "rounded-tr" | "rounded-tr-md" => d.rounded_tr_md(),
        "rounded-tr-lg" => d.rounded_tr_lg(),
        "rounded-tr-xl" => d.rounded_tr_xl(),
        "rounded-tr-2xl" => d.rounded_tr_2xl(),
        "rounded-tr-3xl" => d.rounded_tr_3xl(),
        "rounded-tr-full" => d.rounded_tr_full(),
        "rounded-bl-none" => d.rounded_bl_none(),
        "rounded-bl-sm" => d.rounded_bl_sm(),
        "rounded-bl" | "rounded-bl-md" => d.rounded_bl_md(),
        "rounded-bl-lg" => d.rounded_bl_lg(),
        "rounded-bl-xl" => d.rounded_bl_xl(),
        "rounded-bl-2xl" => d.rounded_bl_2xl(),
        "rounded-bl-3xl" => d.rounded_bl_3xl(),
        "rounded-bl-full" => d.rounded_bl_full(),
        "rounded-br-none" => d.rounded_br_none(),
        "rounded-br-sm" => d.rounded_br_sm(),
        "rounded-br" | "rounded-br-md" => d.rounded_br_md(),
        "rounded-br-lg" => d.rounded_br_lg(),
        "rounded-br-xl" => d.rounded_br_xl(),
        "rounded-br-2xl" => d.rounded_br_2xl(),
        "rounded-br-3xl" => d.rounded_br_3xl(),
        "rounded-br-full" => d.rounded_br_full(),

        _ => return Err(d),
    })
}

#[allow(clippy::result_large_err)]
fn apply_shadows(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Shadow ──
        "shadow-2xs" => d.shadow_2xs(),
        "shadow-xs" => d.shadow_xs(),
        "shadow-sm" => d.shadow_sm(),
        "shadow" | "shadow-md" => d.shadow_md(),
        "shadow-lg" => d.shadow_lg(),
        "shadow-xl" => d.shadow_xl(),
        "shadow-2xl" => d.shadow_2xl(),
        "shadow-none" => d.shadow_none(),

        // ── Ring (focus ring via box-shadow spread) ──
        "ring" => d.shadow(vec![BoxShadow {
            color: hsla(0.603, 0.912, 0.602, 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(3.),
            inset: false,
        }]),
        "ring-0" => d.shadow_none(),
        "ring-1" => d.shadow(vec![BoxShadow {
            color: hsla(0.603, 0.912, 0.602, 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(1.),
            inset: false,
        }]),
        "ring-2" => d.shadow(vec![BoxShadow {
            color: hsla(0.603, 0.912, 0.602, 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(2.),
            inset: false,
        }]),
        "ring-4" => d.shadow(vec![BoxShadow {
            color: hsla(0.603, 0.912, 0.602, 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(4.),
            inset: false,
        }]),
        "ring-8" => d.shadow(vec![BoxShadow {
            color: hsla(0.603, 0.912, 0.602, 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(8.),
            inset: false,
        }]),
        "ring-inset" => d, // GPUI has no inset shadow; accepted silently
        _ => return Err(d),
    })
}

#[allow(clippy::result_large_err)]
fn apply_misc(d: Div, class: &str) -> Result<Div, Div> {
    Ok(match class {
        // ── Cursor ──
        "cursor-default" => d.cursor_default(),
        "cursor-pointer" => d.cursor_pointer(),
        "cursor-text" => d.cursor_text(),
        "cursor-move" => d.cursor_move(),
        "cursor-not-allowed" => d.cursor_not_allowed(),
        "cursor-context-menu" => d.cursor_context_menu(),
        "cursor-crosshair" => d.cursor_crosshair(),
        "cursor-vertical-text" => d.cursor_vertical_text(),
        "cursor-alias" => d.cursor_alias(),
        "cursor-copy" => d.cursor_copy(),
        "cursor-no-drop" => d.cursor_no_drop(),
        "cursor-grab" => d.cursor_grab(),
        "cursor-grabbing" => d.cursor_grabbing(),
        "cursor-ew-resize" => d.cursor_ew_resize(),
        "cursor-ns-resize" => d.cursor_ns_resize(),
        "cursor-nesw-resize" => d.cursor_nesw_resize(),
        "cursor-nwse-resize" => d.cursor_nwse_resize(),
        "cursor-col-resize" => d.cursor_col_resize(),
        "cursor-row-resize" => d.cursor_row_resize(),
        "cursor-n-resize" => d.cursor_n_resize(),
        "cursor-e-resize" => d.cursor_e_resize(),
        "cursor-s-resize" => d.cursor_s_resize(),
        "cursor-w-resize" => d.cursor_w_resize(),

        // ── Transitions & animations — no-op at class level ──
        // Actual animation is handled by animate: attributes in the renderer.
        "transition"
        | "transition-all"
        | "transition-colors"
        | "transition-opacity"
        | "transition-shadow"
        | "transition-transform"
        | "duration-75"
        | "duration-100"
        | "duration-150"
        | "duration-200"
        | "duration-300"
        | "duration-500"
        | "duration-700"
        | "duration-1000"
        | "ease-linear"
        | "ease-in"
        | "ease-out"
        | "ease-in-out"
        | "delay-75"
        | "delay-100"
        | "delay-150"
        | "delay-200"
        | "delay-300"
        | "delay-500"
        | "delay-700"
        | "delay-1000"
        | "animate-none"
        | "animate-spin"
        | "animate-ping"
        | "animate-pulse"
        | "animate-bounce" => d,

        // ── Accepted silently (CSS-only or unsupported in GPUI) ──
        "inline-flex"
        | "inline"
        | "inline-block"
        | "select-none"
        | "pointer-events-none"
        | "whitespace-pre"
        | "sticky"
        | "fixed" => d,

        // ── Debug (only in debug builds) ──
        #[cfg(debug_assertions)]
        "debug" => d.debug(),
        #[cfg(debug_assertions)]
        "debug-below" => d.debug_below(),
        #[cfg(not(debug_assertions))]
        "debug" | "debug-below" => d,

        _ => return Err(d),
    })
}

type LengthPropEntry = (&'static str, fn(Div, Length) -> Div);
type DefiniteLengthPropEntry = (&'static str, fn(Div, DefiniteLength) -> Div);

fn apply_dynamic(d: Div, class: &str) -> Div {
    // ── Length properties (support auto) ──
    const LENGTH_PROPS: &[LengthPropEntry] = &[
        ("w-", |d, v| d.w(v)),
        ("h-", |d, v| d.h(v)),
        ("min-w-", |d, v| d.min_w(v)),
        ("min-h-", |d, v| d.min_h(v)),
        ("max-w-", |d, v| d.max_w(v)),
        ("max-h-", |d, v| d.max_h(v)),
        ("m-", |d, v| d.m(v)),
        ("mx-", |d, v| d.mx(v)),
        ("my-", |d, v| d.my(v)),
        ("mt-", |d, v| d.mt(v)),
        ("mb-", |d, v| d.mb(v)),
        ("ml-", |d, v| d.ml(v)),
        ("mr-", |d, v| d.mr(v)),
        ("top-", |d, v| d.top(v)),
        ("bottom-", |d, v| d.bottom(v)),
        ("left-", |d, v| d.left(v)),
        ("right-", |d, v| d.right(v)),
        ("inset-", |d, v| d.inset(v)),
        ("size-", |d, v| d.size(v)),
        ("basis-", |d, v| d.flex_basis(v)),
    ];

    for (prefix, apply) in LENGTH_PROPS {
        if let Some(rest) = class.strip_prefix(prefix) {
            if let Some(len) = parse_length(rest) {
                return apply(d, len);
            }
        }
    }

    // ── DefiniteLength properties (no auto) ──
    const DEFINITE_PROPS: &[DefiniteLengthPropEntry] = &[
        ("p-", |d, v| d.p(v)),
        ("px-", |d, v| d.px(v)),
        ("py-", |d, v| d.py(v)),
        ("pt-", |d, v| d.pt(v)),
        ("pb-", |d, v| d.pb(v)),
        ("pl-", |d, v| d.pl(v)),
        ("pr-", |d, v| d.pr(v)),
        ("gap-", |d, v| d.gap(v)),
        ("gap-x-", |d, v| d.gap_x(v)),
        ("gap-y-", |d, v| d.gap_y(v)),
    ];

    for (prefix, apply) in DEFINITE_PROPS {
        if let Some(rest) = class.strip_prefix(prefix) {
            if let Some(len) = parse_definite_length(rest) {
                return apply(d, len);
            }
        }
    }

    // ── Arbitrary border radius: rounded-[Npx], rounded-t-[Npx], etc. ──
    for (prefix, apply_fn) in &[
        (
            "rounded-tl-[",
            Div::rounded_tl as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-tr-[",
            Div::rounded_tr as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-bl-[",
            Div::rounded_bl as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-br-[",
            Div::rounded_br as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-t-[",
            Div::rounded_t as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-b-[",
            Div::rounded_b as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-l-[",
            Div::rounded_l as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "rounded-r-[",
            Div::rounded_r as fn(Div, AbsoluteLength) -> Div,
        ),
        ("rounded-[", Div::rounded as fn(Div, AbsoluteLength) -> Div),
    ] {
        if let Some(rest) = class.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix(']') {
                if let Some(abs) = parse_absolute_length(inner) {
                    return apply_fn(d, abs);
                }
            }
        }
    }

    // ── Arbitrary border widths: border-t-[Npx], border-[Npx] etc. ──
    for (prefix, apply_fn) in &[
        (
            "border-t-[",
            Div::border_t as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "border-b-[",
            Div::border_b as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "border-l-[",
            Div::border_l as fn(Div, AbsoluteLength) -> Div,
        ),
        (
            "border-r-[",
            Div::border_r as fn(Div, AbsoluteLength) -> Div,
        ),
        ("border-[", Div::border as fn(Div, AbsoluteLength) -> Div),
    ] {
        if let Some(rest) = class.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix(']') {
                if let Some(abs) = parse_absolute_length(inner) {
                    return apply_fn(d, abs);
                }
            }
        }
    }

    // ── Grid placement: col-span-N, col-start-N, col-end-N, row-span-N, etc. ──
    if let Some(rest) = class.strip_prefix("col-span-") {
        if let Ok(n) = rest.parse::<u16>() {
            return d.col_span(n);
        }
    }
    if let Some(rest) = class.strip_prefix("col-start-") {
        if let Ok(n) = rest.parse::<i16>() {
            return d.col_start(n);
        }
    }
    if let Some(rest) = class.strip_prefix("col-end-") {
        if let Ok(n) = rest.parse::<i16>() {
            return d.col_end(n);
        }
    }
    if let Some(rest) = class.strip_prefix("row-span-") {
        if let Ok(n) = rest.parse::<u16>() {
            return d.row_span(n);
        }
    }
    if let Some(rest) = class.strip_prefix("row-start-") {
        if let Ok(n) = rest.parse::<i16>() {
            return d.row_start(n);
        }
    }
    if let Some(rest) = class.strip_prefix("row-end-") {
        if let Ok(n) = rest.parse::<i16>() {
            return d.row_end(n);
        }
    }
    // Dynamic grid-cols-N and grid-rows-N beyond the static 1-12
    if let Some(rest) = class.strip_prefix("grid-cols-") {
        if let Ok(n) = rest.parse::<u16>() {
            return d.grid_cols(n);
        }
    }
    if let Some(rest) = class.strip_prefix("grid-rows-") {
        if let Ok(n) = rest.parse::<u16>() {
            return d.grid_rows(n);
        }
    }

    // ── tracking-* (letter-spacing) — gpui-ce provides letter_spacing natively ──
    {
        match class {
            "tracking-tighter" => return d.letter_spacing(px(-2.0)),
            "tracking-tight" => return d.letter_spacing(px(-1.0)),
            "tracking-normal" => return d.letter_spacing(px(0.)),
            "tracking-wide" => return d.letter_spacing(px(1.5)),
            "tracking-wider" => return d.letter_spacing(px(3.0)),
            "tracking-widest" => return d.letter_spacing(px(5.0)),
            _ => {}
        }
        if let Some(rest) = class.strip_prefix("tracking-[") {
            if let Some(inner) = rest.strip_suffix(']') {
                if let Some(abs) = parse_absolute_length(inner) {
                    let px_val = match abs {
                        gpui::AbsoluteLength::Pixels(p) => p,
                        gpui::AbsoluteLength::Rems(r) => px(r.0 * 16.0),
                    };
                    return d.letter_spacing(px_val);
                }
            }
        }
    }

    // ── aspect-[N/M] or aspect-[N] — arbitrary aspect ratio ──
    if let Some(rest) = class.strip_prefix("aspect-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            let ratio = if let Some((num, den)) = inner.split_once('/') {
                let w = num.parse::<f32>().ok();
                let h = den.parse::<f32>().ok();
                w.zip(h)
                    .and_then(|(w, h)| if h != 0.0 { Some(w / h) } else { None })
            } else {
                inner.parse::<f32>().ok()
            };
            if let Some(r) = ratio {
                let mut d = d;
                d.style().aspect_ratio = Some(r);
                return d;
            }
        }
    }

    // ── ring-[Npx] — arbitrary ring width ──
    if let Some(rest) = class.strip_prefix("ring-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(abs) = parse_absolute_length(inner) {
                let spread = match abs {
                    gpui::AbsoluteLength::Pixels(p) => p,
                    gpui::AbsoluteLength::Rems(r) => px(r.0 * 16.0),
                };
                return d.shadow(vec![BoxShadow {
                    color: hsla(0.603, 0.912, 0.602, 0.5),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: spread,
                    inset: false,
                }]);
            }
        }
    }

    // ── line-clamp-N ──
    if let Some(rest) = class.strip_prefix("line-clamp-") {
        if let Ok(n) = rest.parse::<usize>() {
            return d.line_clamp(n);
        }
    }

    // ── font-['Family'] or font-[Family] ──
    if let Some(rest) = class.strip_prefix("font-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            let family = inner.trim_matches('\'').trim_matches('"').replace('_', " ");
            return d.font_family(family);
        }
    }

    // ── text-[size] — arbitrary text size ──
    if let Some(rest) = class.strip_prefix("text-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            // Check if it's a color first: text-[#rrggbb]
            if let Some(hex_str) = inner.strip_prefix('#') {
                if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                    return d.text_color(rgb(hex));
                }
            }
            // Otherwise it's a size
            if let Some(len) = parse_absolute_length(inner) {
                return d.text_size(len);
            }
        }
    }

    // ── leading-[value] — arbitrary line height ──
    if let Some(rest) = class.strip_prefix("leading-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(abs) = parse_absolute_length(inner) {
                return d.line_height(abs);
            }
        }
    }

    // ── opacity-N ──
    if let Some(rest) = class.strip_prefix("opacity-") {
        if let Ok(n) = rest.parse::<f32>() {
            return d.opacity(n / 100.0);
        }
    }

    // ── Decoration color: decoration-[#hex] or decoration-family-shade ──
    if let Some(rest) = class.strip_prefix("decoration-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(hex_str) = inner.strip_prefix('#') {
                if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                    return d.text_decoration_color(rgb(hex));
                }
            }
        }
    }
    if let Some(rest) = class.strip_prefix("decoration-") {
        // decoration-family-shade
        if let Some(dash_pos) = rest.rfind('-') {
            let family = &rest[..dash_pos];
            let shade = &rest[dash_pos + 1..];
            if let Some(hex) = tailwind_color(family, shade) {
                return d.text_decoration_color(rgb(hex));
            }
        }
    }

    // ── text-bg: text-bg-[#hex] or text-bg-family-shade ──
    if let Some(rest) = class.strip_prefix("text-bg-[") {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some(hex_str) = inner.strip_prefix('#') {
                if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                    return d.text_bg(rgb(hex));
                }
            }
        }
    }
    if let Some(rest) = class.strip_prefix("text-bg-") {
        if let Some(dash_pos) = rest.rfind('-') {
            let family = &rest[..dash_pos];
            let shade = &rest[dash_pos + 1..];
            if let Some(hex) = tailwind_color(family, shade) {
                return d.text_bg(rgb(hex));
            }
        }
    }

    // ── Color families: bg-{family}-{shade}, text-{family}-{shade}, border-{family}-{shade} ──
    for (prefix, apply_fn) in &[
        ("bg-", apply_bg as fn(Div, u32) -> Div),
        ("text-", apply_text_color as fn(Div, u32) -> Div),
        ("border-", apply_border_color as fn(Div, u32) -> Div),
    ] {
        if let Some(rest) = class.strip_prefix(prefix) {
            // Arbitrary hex: bg-[#rrggbb] or bg-[#rrggbbaa]
            if rest.starts_with('[') && rest.ends_with(']') {
                let inner = &rest[1..rest.len() - 1];
                if let Some(hex_str) = inner.strip_prefix('#') {
                    if hex_str.len() == 8 {
                        // 8-digit hex: RRGGBBAA
                        if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                            return d.bg(rgba(hex));
                        }
                    }
                    if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                        return apply_fn(d, hex);
                    }
                }
                // hsla: bg-[hsla(h,s,l,a)]
                if let Some(hsla_str) = inner.strip_prefix("hsla(") {
                    if let Some(vals) = hsla_str.strip_suffix(')') {
                        let parts: Vec<&str> = vals.split(',').map(|s| s.trim()).collect();
                        if parts.len() == 4 {
                            if let (Ok(h), Ok(s), Ok(l), Ok(a)) = (
                                parts[0].parse::<f32>(),
                                parts[1].trim_end_matches('%').parse::<f32>(),
                                parts[2].trim_end_matches('%').parse::<f32>(),
                                parts[3].parse::<f32>(),
                            ) {
                                let color = hsla(h / 360.0, s / 100.0, l / 100.0, a);
                                if *prefix == "bg-" {
                                    return d.bg(color);
                                }
                                if *prefix == "text-" {
                                    return d.text_color(color);
                                }
                                if *prefix == "border-" {
                                    return d.border_color(color);
                                }
                            }
                        }
                    }
                }
            }
            // bg-family/alpha: bg-red-500/50
            if let Some(slash_pos) = rest.rfind('/') {
                let color_part = &rest[..slash_pos];
                let alpha_str = &rest[slash_pos + 1..];
                if let Ok(alpha_pct) = alpha_str.parse::<u32>() {
                    if let Some(dash_pos) = color_part.rfind('-') {
                        let family = &color_part[..dash_pos];
                        let shade = &color_part[dash_pos + 1..];
                        if let Some(hex) = tailwind_color(family, shade) {
                            let alpha_byte = alpha_pct * 255 / 100;
                            let rgba_val = (hex << 8) | alpha_byte;
                            return d.bg(rgba(rgba_val));
                        }
                    }
                }
            }
            // Named: family-shade
            if let Some(dash_pos) = rest.rfind('-') {
                let family = &rest[..dash_pos];
                let shade = &rest[dash_pos + 1..];
                if let Some(hex) = tailwind_color(family, shade) {
                    return apply_fn(d, hex);
                }
            }
        }
    }

    d
}

fn apply_bg(d: Div, hex: u32) -> Div {
    d.bg(rgb(hex))
}
fn apply_text_color(d: Div, hex: u32) -> Div {
    d.text_color(rgb(hex))
}
fn apply_border_color(d: Div, hex: u32) -> Div {
    d.border_color(rgb(hex))
}

/// Lower a [`LengthToken`] to an `AbsoluteLength`, rejecting relative units.
fn token_to_absolute(token: LengthToken) -> Option<AbsoluteLength> {
    match token {
        LengthToken::Px(n) => Some(px(n).into()),
        LengthToken::Rems(n) => Some(rems(n).into()),
        LengthToken::Auto | LengthToken::Fraction(_) => None,
    }
}

/// Parse a Tailwind length token into a `Length` (supports auto, full, %, px, rem, number)
fn parse_length(value: &str) -> Option<Length> {
    Some(match parse_length_token(value)? {
        LengthToken::Auto => Length::Auto,
        LengthToken::Fraction(f) => relative(f).into(),
        LengthToken::Px(n) => px(n).into(),
        LengthToken::Rems(n) => rems(n).into(),
    })
}

/// Parse a Tailwind length token into a `DefiniteLength` (no auto, no %)
fn parse_definite_length(value: &str) -> Option<DefiniteLength> {
    if let Some(inner) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return token_to_absolute(parse_arbitrary_length_token(inner)?).map(Into::into);
    }
    if value == "px" {
        return Some(px(1.).into());
    }
    if let Ok(n) = value.parse::<f32>() {
        return Some(rems(n * 0.25).into());
    }
    None
}

/// Parse a CSS size string to `AbsoluteLength` (px or rem only)
pub fn parse_absolute_length(inner: &str) -> Option<AbsoluteLength> {
    token_to_absolute(parse_arbitrary_length_token(inner)?)
}

/// Parse a duration string like "300ms", "1s", "0.5s" into milliseconds.
pub fn parse_duration_ms(s: &str) -> Option<u64> {
    if let Some(rest) = s.strip_suffix("ms") {
        return rest.parse::<u64>().ok();
    }
    if let Some(rest) = s.strip_suffix('s') {
        return rest.parse::<f64>().ok().map(|v| (v * 1000.0) as u64);
    }
    s.parse::<u64>().ok()
}

// ============================================================
// Tailwind CSS v4 color palette (via crepuscularity-core)
// ============================================================

pub fn tailwind_color(family: &str, shade: &str) -> Option<u32> {
    crepuscularity_core::tailwind::lookup_color_u32(&format!("{family}-{shade}"))
}

#[cfg(test)]
mod tailwind_color_tests {
    use super::tailwind_color;

    #[test]
    fn v4_red_500() {
        assert_eq!(tailwind_color("red", "500"), Some(0xfb2c36));
    }
}

#[cfg(test)]
mod parse_duration_ms_tests {
    use super::parse_duration_ms;

    #[test]
    fn test_parse_duration_ms() {
        assert_eq!(parse_duration_ms("500ms"), Some(500));
        assert_eq!(parse_duration_ms("300ms"), Some(300));
        assert_eq!(parse_duration_ms("1s"), Some(1000));
        assert_eq!(parse_duration_ms("2s"), Some(2000));
        assert_eq!(parse_duration_ms("0.5s"), Some(500));
        assert_eq!(parse_duration_ms("0.25s"), Some(250));
        assert_eq!(parse_duration_ms("500"), Some(500));
        assert_eq!(parse_duration_ms("1000"), Some(1000));
        assert_eq!(parse_duration_ms("abc"), None);
        assert_eq!(parse_duration_ms("1.5ms"), None);
        assert_eq!(parse_duration_ms("-1s"), Some(0));
        assert_eq!(parse_duration_ms("-100ms"), None);
    }
}

#[cfg(test)]
mod parse_length_tests {
    use super::parse_length;
    use gpui::{px, relative, rems, Length};

    /// The fraction set that used to be spelled out arm-by-arm, now derived.
    #[test]
    fn fractions_match_the_former_hardcoded_table() {
        let expected: &[(&str, f32)] = &[
            ("1/2", 0.5),
            ("1/3", 1.0 / 3.0),
            ("2/3", 2.0 / 3.0),
            ("1/4", 0.25),
            ("2/4", 0.5),
            ("3/4", 0.75),
            ("1/5", 0.2),
            ("2/5", 0.4),
            ("3/5", 0.6),
            ("4/5", 0.8),
            ("1/6", 1.0 / 6.0),
            ("2/6", 2.0 / 6.0),
            ("3/6", 0.5),
            ("4/6", 4.0 / 6.0),
            ("5/6", 5.0 / 6.0),
            ("1/12", 1.0 / 12.0),
            ("2/12", 2.0 / 12.0),
            ("3/12", 0.25),
            ("4/12", 4.0 / 12.0),
            ("5/12", 5.0 / 12.0),
            ("6/12", 0.5),
            ("7/12", 7.0 / 12.0),
            ("8/12", 8.0 / 12.0),
            ("9/12", 9.0 / 12.0),
            ("10/12", 10.0 / 12.0),
            ("11/12", 11.0 / 12.0),
        ];
        for (token, fraction) in expected {
            assert_eq!(
                parse_length(token),
                Some(relative(*fraction).into()),
                "fraction {token} regressed"
            );
        }
    }

    #[test]
    fn keywords_and_scale() {
        assert_eq!(parse_length("full"), Some(relative(1.).into()));
        assert_eq!(parse_length("screen"), Some(relative(1.).into()));
        assert_eq!(parse_length("auto"), Some(Length::Auto));
        assert_eq!(parse_length("px"), Some(px(1.).into()));
        assert_eq!(parse_length("4"), Some(rems(1.).into()));
        assert_eq!(parse_length("[12px]"), Some(px(12.).into()));
        assert_eq!(parse_length("[50%]"), Some(relative(0.5).into()));
        assert_eq!(parse_length("[2rem]"), Some(rems(2.).into()));
        assert_eq!(parse_length("nope"), None);
    }
}

#[cfg(test)]
mod parse_definite_length_tests {
    use super::parse_definite_length;
    use gpui::{px, rems};

    #[test]
    fn stays_definite() {
        assert_eq!(parse_definite_length("4"), Some(rems(1.).into()));
        assert_eq!(parse_definite_length("px"), Some(px(1.).into()));
        assert_eq!(parse_definite_length("[12px]"), Some(px(12.).into()));
        // Relative forms are not definite lengths and must stay rejected.
        assert_eq!(parse_definite_length("full"), None);
        assert_eq!(parse_definite_length("auto"), None);
        assert_eq!(parse_definite_length("1/2"), None);
        assert_eq!(parse_definite_length("[50%]"), None);
    }
}

#[cfg(test)]
mod parse_absolute_length_tests {
    use super::parse_absolute_length;
    use gpui::{px, rems};

    #[test]
    fn test_px_suffix() {
        assert_eq!(parse_absolute_length("10px"), Some(px(10.0).into()));
        assert_eq!(parse_absolute_length("15.5px"), Some(px(15.5).into()));
        assert_eq!(parse_absolute_length("-5px"), Some(px(-5.0).into()));
    }

    #[test]
    fn test_rem_suffix() {
        assert_eq!(parse_absolute_length("2rem"), Some(rems(2.0).into()));
        assert_eq!(parse_absolute_length("1.25rem"), Some(rems(1.25).into()));
        assert_eq!(parse_absolute_length("-0.5rem"), Some(rems(-0.5).into()));
    }

    #[test]
    fn test_no_suffix_fallback_to_px() {
        assert_eq!(parse_absolute_length("20"), Some(px(20.0).into()));
        assert_eq!(parse_absolute_length("2.5"), Some(px(2.5).into()));
        assert_eq!(parse_absolute_length("-10"), Some(px(-10.0).into()));
    }

    #[test]
    fn test_invalid_strings() {
        assert_eq!(parse_absolute_length(""), None);
        assert_eq!(parse_absolute_length("px"), None);
        assert_eq!(parse_absolute_length("rem"), None);
        assert_eq!(parse_absolute_length("10em"), None);
        assert_eq!(parse_absolute_length("10vh"), None);
        assert_eq!(parse_absolute_length("abc"), None);
        assert_eq!(parse_absolute_length("10.5.5px"), None);
        assert_eq!(parse_absolute_length("10pxrem"), None);
    }
}

#[cfg(test)]
mod class_list_tests {
    use super::*;

    #[test]
    fn dynamic_class_lists_match_individual_utilities() {
        let mut reset = apply_class_list(
            gpui::div().self_center().aspect_square().w_full(),
            "self-auto aspect-auto",
        );
        assert!(reset.style().align_self.is_none());
        assert!(reset.style().aspect_ratio.is_none());
        let mut expected = gpui::div().w_full();
        assert_eq!(
            format!("{:?}", reset.style()),
            format!("{:?}", expected.style())
        );

        let mut ctx = TemplateContext::new();
        ctx.set("theme", "flex flex-row gap-4 p-2 bg-red-500");
        let mut dynamic = apply_class_with_ctx(gpui::div(), "{theme}", Some(&ctx));
        let mut list = apply_class_list(gpui::div().w_full(), "flex flex-row gap-4 p-2 bg-red-500");
        let mut expected_list =
            apply_class_list(gpui::div(), "w-full flex flex-row gap-4 p-2 bg-red-500");
        assert_eq!(
            format!("{:?}", list.style()),
            format!("{:?}", expected_list.style())
        );
        let mut literal = gpui::div();
        for class in ["flex", "flex-row", "gap-4", "p-2", "bg-red-500"] {
            literal = apply_class(literal, class);
        }
        assert_eq!(
            format!("{:?}", dynamic.style()),
            format!("{:?}", literal.style())
        );
    }

    #[test]
    fn target_prefixes_match_macro_filtering() {
        let mut actual = apply_class_list(
            gpui::div(),
            "gpui:flex gui:flex-row gpui:gap-4 web:flex-col tui:p-8 ios:hidden",
        );
        let mut expected = apply_class_list(gpui::div(), "flex flex-row gap-4");
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );

        let mut ctx = TemplateContext::new();
        ctx.set("theme", "gpui:flex gui:flex-row web:hidden");
        let mut actual = apply_class_with_ctx(gpui::div(), "gpui:{theme}", Some(&ctx));
        let mut expected = apply_class_list(gpui::div(), "flex flex-row");
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }

    #[test]
    fn dynamic_class_values_are_not_recursively_evaluated() {
        let mut ctx = TemplateContext::new();
        ctx.set("theme", "{theme}");
        let mut actual = apply_class_with_ctx(gpui::div(), "{theme}", Some(&ctx));
        let mut empty = gpui::div();
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", empty.style())
        );
    }

    #[test]
    fn dynamic_dimensions_match_static_styles_after_context_updates() {
        let mut ctx = TemplateContext::new();
        ctx.set("color", "#12ab34");
        ctx.set("alpha", 50);
        for (size, gap) in [(7, 2), (30, 5)] {
            ctx.set("size", size);
            ctx.set("gap", gap);
            let mut actual = gpui::div();
            for class in [
                "w-{1 + 2}",
                "h-[{size + 2}px]",
                "p-{gap}",
                "bg-{color}",
                "opacity-{alpha}",
            ] {
                actual = apply_class_with_ctx(actual, class, Some(&ctx));
            }
            let mut expected = apply_class_list(
                gpui::div(),
                format!("w-3 h-[{}px] p-{gap} bg-[#12ab34] opacity-50", size + 2),
            );
            assert_eq!(
                format!("{:?}", actual.style()),
                format!("{:?}", expected.style())
            );
        }
    }

    #[test]
    fn state_dimensions_resolve_expressions_without_applying_other_states() {
        let mut ctx = TemplateContext::new();
        for (size, gap) in [(7, 2), (30, 5)] {
            ctx.set("size", size);
            ctx.set("gap", gap);
            let actual = state_style_with_ctx(
                [
                    "gpui:hover:w-{1 + 2}",
                    "hover:h-[{size + 2}px]",
                    "hover:p-{gap}",
                    "focus:w-99",
                ]
                .into_iter(),
                "hover",
                Some(&ctx),
            );
            let mut expected =
                apply_class_list(gpui::div(), format!("w-3 h-[{}px] p-{gap}", size + 2));
            assert_eq!(format!("{actual:?}"), format!("{:?}", expected.style()));
        }
    }

    #[test]
    fn dynamic_dimension_fallbacks_are_noops_and_resolved_lists_are_not_recursive() {
        let mut ctx = TemplateContext::new();
        ctx.set("braced", "{dimension}");
        ctx.set("dimension", 3);
        for class in ["p-{missing}", "w-{1 +}", "p-{braced}"] {
            let mut actual = apply_class_with_ctx(gpui::div(), class, Some(&ctx));
            let mut empty = gpui::div();
            assert_eq!(
                format!("{:?}", actual.style()),
                format!("{:?}", empty.style()),
                "{class}"
            );
        }
        ctx.set("gap", "2 m-4");
        let mut actual = apply_class_with_ctx(gpui::div(), "p-{gap}", Some(&ctx));
        let mut expected = apply_class_list(gpui::div(), "p-2 m-4");
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
}

#[cfg(test)]
mod state_list_tests {
    use super::*;

    #[test]
    fn runtime_state_lists_preserve_utility_order_and_target_filtering() {
        let actual = class_list_state_style(
            "hover:bg-blue-500 gpui:hover:bg-red-500 gui:hover:p-2 web:hover:hidden focus:p-8",
            "hover",
        );
        let mut expected = apply_class_list(gpui::div(), "bg-red-500 p-2");
        assert_eq!(format!("{actual:?}"), format!("{:?}", expected.style()));
        let actual = class_list_state_style("hover:p-2 active:p-4", "active");
        let mut expected = apply_class_list(gpui::div(), "p-4");
        assert_eq!(format!("{actual:?}"), format!("{:?}", expected.style()));
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;
    #[test]
    fn visible_overflow_replaces_clipping_on_the_requested_axes() {
        for (class, x, y) in [
            (
                "overflow-visible",
                gpui::Overflow::Visible,
                gpui::Overflow::Visible,
            ),
            (
                "overflow-x-visible",
                gpui::Overflow::Visible,
                gpui::Overflow::Hidden,
            ),
            (
                "overflow-y-visible",
                gpui::Overflow::Hidden,
                gpui::Overflow::Visible,
            ),
        ] {
            let mut actual = apply_class(gpui::div().overflow_hidden(), class);
            assert_eq!(actual.style().overflow.x, Some(x));
            assert_eq!(actual.style().overflow.y, Some(y));
        }
    }
}
