//! Tailwind CSS v4 utility classes → [`crate::document::EmbeddedStyle`].
//!
//! Skips responsive/dark/hover variants (`sm:`, `dark:`, etc.). No grid, shadows, or z-index.

use crate::color::Color;
use crate::document::{Align, EmbeddedStyle, FlexDir, Justify, SizeHint};
use crepuscularity_colors::lookup_named_color;

#[cfg(feature = "std")]
use crepuscularity_core::context::TemplateContext;
#[cfg(feature = "std")]
use crepuscularity_core::tailwind::parse::{
    parse_color_rgb, parse_size_width_height, parse_spacing_px, SizeToken,
};

#[cfg(not(feature = "std"))]
mod tailwind_parse {
    use crepuscularity_colors::lookup_named_color;

    pub enum SizeToken {
        Full,
        Auto,
        Px(u16),
        Fraction { num: u16, den: u16 },
        Spacing(u16),
    }

    pub fn parse_spacing_px(rest: &str) -> Option<u16> {
        if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let stripped = inner.strip_suffix("px").unwrap_or(inner);
            return stripped.parse::<u16>().ok();
        }
        match rest {
            "px" => Some(1),
            "0" => Some(0),
            "0.5" => Some(2),
            "1" => Some(4),
            "1.5" => Some(6),
            "2" => Some(8),
            "2.5" => Some(10),
            "3" => Some(12),
            "3.5" => Some(14),
            "4" => Some(16),
            "5" => Some(20),
            "6" => Some(24),
            "8" => Some(32),
            _ => rest.parse::<u16>().ok().map(|n| n.saturating_mul(4)),
        }
    }

    pub fn parse_size_width_height(rest: &str) -> Option<SizeToken> {
        match rest {
            "full" | "screen" => return Some(SizeToken::Full),
            "auto" | "fit" | "min" | "max" => return Some(SizeToken::Auto),
            _ => {}
        }
        if let Some((num, den)) = rest.split_once('/') {
            if let (Ok(n), Ok(d)) = (num.parse::<u16>(), den.parse::<u16>()) {
                if d > 0 {
                    return Some(SizeToken::Fraction { num: n, den: d });
                }
            }
        }
        if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let stripped = inner.strip_suffix("px").unwrap_or(inner);
            if let Ok(n) = stripped.parse::<u16>() {
                return Some(SizeToken::Px(n));
            }
        }
        parse_spacing_px(rest).map(SizeToken::Spacing)
    }

    pub fn parse_color_rgb(name: &str) -> Option<[u8; 3]> {
        let mut name = name.split_once('/').map_or(name, |(color, _)| color).trim();
        while let Some(inner) = name.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            name = inner.trim();
        }
        if let Some(hex) = lookup_named_color(name) {
            return parse_hex_rgb(hex);
        }
        if name.starts_with('#') {
            return parse_hex_rgb(name);
        }
        None
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn deeply_wrapped_colors_are_parsed_without_recursion() {
            let mut bytes = [b'['; 40_007];
            bytes[20_000..20_007].copy_from_slice(b"#123456");
            bytes[20_007..].fill(b']');
            let wrapped = core::str::from_utf8(&bytes).unwrap();
            assert_eq!(parse_color_rgb(wrapped), parse_color_rgb("#123456"));
            assert_eq!(parse_color_rgb("#aé1234"), None);
        }
    }

    fn parse_hex_rgb(s: &str) -> Option<[u8; 3]> {
        let color = crate::color::parse_hex(s)?.to_rgb888();
        Some([color.r, color.g, color.b])
    }
}

#[cfg(not(feature = "std"))]
use tailwind_parse::{parse_color_rgb, parse_size_width_height, parse_spacing_px, SizeToken};

pub fn apply_classes(classes: &[impl AsRef<str>], s: &mut EmbeddedStyle, ctx: Ctx<'_>) {
    let mut explicit = [false; 6];
    for c in classes {
        let class = c.as_ref();
        apply_class(class, s, ctx);
        if class.contains(':') {
            continue;
        }
        let Some((prefix, value)) = class.split_once('-') else {
            continue;
        };
        let index = match prefix {
            "px" => 0,
            "py" => 1,
            "pt" => 2,
            "pb" => 3,
            "pl" => 4,
            "pr" => 5,
            _ => continue,
        };
        if parse_spacing_px(value).is_some() {
            explicit[index] = true;
        }
    }
    let values = [
        s.padding_x,
        s.padding_y,
        s.padding_t,
        s.padding_b,
        s.padding_l,
        s.padding_r,
    ];
    if !explicit
        .iter()
        .zip(values)
        .any(|(set, value)| *set && value == 0)
    {
        return;
    }
    // Public fields use zero for unset; resolve explicit zeros before discarding fallbacks.
    let edge = |specific, axis, specific_set, axis_set| {
        if specific_set || specific > 0 {
            specific
        } else if axis_set || axis > 0 {
            axis
        } else {
            s.padding
        }
    };
    let [x, y, t, b, l, r] = explicit;
    let left = edge(s.padding_l, s.padding_x, l, x);
    let right = edge(s.padding_r, s.padding_x, r, x);
    let top = edge(s.padding_t, s.padding_y, t, y);
    let bottom = edge(s.padding_b, s.padding_y, b, y);
    s.padding = 0;
    s.padding_x = 0;
    s.padding_y = 0;
    s.padding_l = left;
    s.padding_r = right;
    s.padding_t = top;
    s.padding_b = bottom;
}

#[cfg(feature = "std")]
pub fn apply_class(class: &str, s: &mut EmbeddedStyle, ctx: Option<&TemplateContext>) {
    if class.contains(':') {
        return;
    }
    apply_class_impl(class, s, ctx);
}

#[cfg(not(feature = "std"))]
pub fn apply_class(class: &str, s: &mut EmbeddedStyle, _ctx: Option<&()>) {
    if class.contains(':') {
        return;
    }
    apply_class_impl(class, s, None);
}

#[cfg(feature = "std")]
fn apply_class_impl(class: &str, s: &mut EmbeddedStyle, ctx: Option<&TemplateContext>) {
    apply_class_body(class, s, ctx);
}

#[cfg(not(feature = "std"))]
fn apply_class_impl(class: &str, s: &mut EmbeddedStyle, _ctx: Option<&()>) {
    apply_class_body(class, s, None);
}

#[cfg(feature = "std")]
type Ctx<'a> = Option<&'a TemplateContext>;
#[cfg(not(feature = "std"))]
type Ctx<'a> = Option<&'a ()>;

fn apply_class_body(class: &str, s: &mut EmbeddedStyle, ctx: Ctx<'_>) {
    if apply_spacing(class, s) {
        return;
    }
    if apply_size(class, s) {
        return;
    }
    if apply_flex_and_align(class, s) {
        return;
    }
    if apply_visibility(class, s) {
        return;
    }
    if apply_border(class, s, ctx) {
        return;
    }
    if apply_background(class, s, ctx) {
        return;
    }
    apply_text(class, s, ctx);
}

fn apply_spacing_prefix(class: &str, prefix: &str, set: &mut u16) -> bool {
    if let Some(rest) = class.strip_prefix(prefix) {
        if let Some(px) = parse_spacing_px(rest) {
            *set = px;
            return true;
        }
    }
    false
}

fn apply_spacing(class: &str, s: &mut EmbeddedStyle) -> bool {
    if apply_spacing_prefix(class, "p-", &mut s.padding) {
        return true;
    }
    if apply_spacing_prefix(class, "px-", &mut s.padding_x) {
        return true;
    }
    if apply_spacing_prefix(class, "py-", &mut s.padding_y) {
        return true;
    }
    if apply_spacing_prefix(class, "pt-", &mut s.padding_t) {
        return true;
    }
    if apply_spacing_prefix(class, "pb-", &mut s.padding_b) {
        return true;
    }
    if apply_spacing_prefix(class, "pl-", &mut s.padding_l) {
        return true;
    }
    if apply_spacing_prefix(class, "pr-", &mut s.padding_r) {
        return true;
    }
    if apply_spacing_prefix(class, "m-", &mut s.margin) {
        return true;
    }
    if apply_spacing_prefix(class, "mx-", &mut s.margin_x) {
        return true;
    }
    if apply_spacing_prefix(class, "my-", &mut s.margin_y) {
        return true;
    }
    if apply_spacing_prefix(class, "mt-", &mut s.margin_t) {
        return true;
    }
    if apply_spacing_prefix(class, "mb-", &mut s.margin_b) {
        return true;
    }
    if apply_spacing_prefix(class, "ml-", &mut s.margin_l) {
        return true;
    }
    if apply_spacing_prefix(class, "mr-", &mut s.margin_r) {
        return true;
    }
    if apply_spacing_prefix(class, "gap-", &mut s.gap) {
        return true;
    }
    false
}

fn apply_size_to_hint(token: SizeToken) -> SizeHint {
    match token {
        SizeToken::Full => SizeHint::Fill,
        SizeToken::Auto => SizeHint::Auto,
        SizeToken::Px(n) => SizeHint::Fixed(n),
        SizeToken::Fraction { num, den } => SizeHint::Fraction { num, den },
        SizeToken::Spacing(n) => SizeHint::Fixed(n),
    }
}

fn apply_size_axis(class: &str, prefix: &str, hint: &mut SizeHint) -> bool {
    if let Some(rest) = class.strip_prefix(prefix) {
        if let Some(token) = parse_size_width_height(rest) {
            *hint = apply_size_to_hint(token);
            return true;
        }
    }
    false
}

fn apply_size(class: &str, s: &mut EmbeddedStyle) -> bool {
    if apply_size_axis(class, "w-", &mut s.width) {
        return true;
    }
    if apply_size_axis(class, "h-", &mut s.height) {
        return true;
    }
    if let Some(rest) = class.strip_prefix("size-") {
        if let Some(token) = parse_size_width_height(rest) {
            let h = apply_size_to_hint(token);
            s.width = h;
            s.height = h;
            return true;
        }
    }
    false
}

fn apply_flex_and_align(class: &str, s: &mut EmbeddedStyle) -> bool {
    match class {
        "flex" => {}
        "flex-col" => s.flex_dir = FlexDir::Column,
        "flex-row" => s.flex_dir = FlexDir::Row,
        "flex-1" => {
            s.width = SizeHint::Flex1;
            s.height = SizeHint::Flex1;
        }
        "flex-auto" | "grow" => s.width = SizeHint::Flex1,
        "flex-none" | "grow-0" => {
            for size in [&mut s.width, &mut s.height] {
                if *size == SizeHint::Flex1 {
                    *size = SizeHint::Auto;
                }
            }
        }
        "items-start" => s.align = Align::Start,
        "items-end" => s.align = Align::End,
        "items-center" => s.align = Align::Center,
        "items-stretch" => s.align = Align::Stretch,
        "justify-start" => s.justify = Justify::Start,
        "justify-end" => s.justify = Justify::End,
        "justify-center" => s.justify = Justify::Center,
        "justify-between" => s.justify = Justify::Between,
        "justify-around" => s.justify = Justify::Around,
        "justify-evenly" => s.justify = Justify::Evenly,
        "w-full" => s.width = SizeHint::Fill,
        "h-full" => s.height = SizeHint::Fill,
        _ => return false,
    }
    true
}

fn apply_visibility(class: &str, s: &mut EmbeddedStyle) -> bool {
    match class {
        "hidden" => s.hidden = true,
        "invisible" => s.invisible = true,
        "visible" => s.invisible = false,
        _ => {
            if let Some(rest) = class.strip_prefix("opacity-") {
                if let Ok(n) = rest.parse::<u8>() {
                    s.opacity_percent = Some(n.min(100));
                    return true;
                }
            }
            return false;
        }
    }
    true
}

fn apply_border(class: &str, s: &mut EmbeddedStyle, ctx: Ctx<'_>) -> bool {
    match class {
        "border" => {
            s.border_width = 1;
            return true;
        }
        "border-0" => {
            s.border_width = 0;
            return true;
        }
        "border-2" => {
            s.border_width = 2;
            return true;
        }
        "border-4" => {
            s.border_width = 4;
            return true;
        }
        _ => {}
    }
    if let Some(rest) = class.strip_prefix("border-") {
        if let Some(c) = resolve_color_token(rest, ctx) {
            s.border_color = Some(c);
            return true;
        }
    }
    false
}

fn apply_background(class: &str, s: &mut EmbeddedStyle, ctx: Ctx<'_>) -> bool {
    if let Some(rest) = class.strip_prefix("bg-") {
        if let Some(c) = resolve_color_token(rest, ctx) {
            s.bg = Some(c);
            return true;
        }
    }
    false
}

fn apply_text(class: &str, s: &mut EmbeddedStyle, ctx: Ctx<'_>) {
    if let Some(rest) = class.strip_prefix("text-") {
        if let Some(c) = resolve_color_token(rest, ctx) {
            s.text = Some(c);
            return;
        }
        let size = match rest {
            "xs" => 12,
            "sm" => 14,
            "base" => 16,
            "lg" => 18,
            "xl" => 20,
            "2xl" => 24,
            "3xl" => 30,
            "4xl" => 36,
            "5xl" => 48,
            "6xl" => 60,
            "7xl" => 72,
            "8xl" => 96,
            "9xl" => 128,
            _ => 0,
        };
        if size > 0 {
            s.font_size_px = size;
            return;
        }
        match rest {
            "left" | "center" | "right" => {}
            _ => {}
        }
    }
    match class {
        "font-bold" | "font-semibold" => s.font_bold = true,
        "font-normal" => s.font_bold = false,
        "uppercase" | "lowercase" | "capitalize" | "italic" | "underline" => {}
        _ => {}
    }
}

fn resolve_color_token(name: &str, _ctx: Ctx<'_>) -> Option<Color> {
    #[cfg(feature = "std")]
    if let Some(ctx) = _ctx {
        if name.starts_with('{') && name.ends_with('}') {
            use crepuscularity_core::context::value_to_str;
            use crepuscularity_core::eval::eval_expr;
            let expr = &name[1..name.len() - 1];
            let Ok(v) = eval_expr(expr, ctx) else {
                return None;
            };
            let s = value_to_str(&v).trim_matches('"').to_string();
            return color_from_name(&s);
        }
    }
    color_from_name(name)
}

fn color_from_name(name: &str) -> Option<Color> {
    if let Some(rgb) = parse_color_rgb(name) {
        return Some(Color::rgb(rgb[0], rgb[1], rgb[2]));
    }
    lookup_named_color(name).and_then(crate::color::parse_hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Align, EmbeddedStyle, FlexDir, SizeHint};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    #[test]
    fn visibility_and_opacity_are_independent() {
        let mut style = EmbeddedStyle::default();
        apply_class("invisible", &mut style, None);
        assert!(style.invisible);
        assert_eq!(style.opacity_percent, None);
        apply_class("opacity-0", &mut style, None);
        apply_class("visible", &mut style, None);
        assert!(!style.invisible);
        assert_eq!(style.opacity_percent, Some(0));
    }

    #[test]
    fn fractional_spacing_is_consistent_across_host_and_firmware() {
        let mut style = EmbeddedStyle::default();
        for token in ["0.5", "1.5", "2.5", "3.5", "7", "[13px]"] {
            let expected = crepuscularity_core::tailwind::parse::parse_spacing_px(token).unwrap();
            apply_class(&format!("p-{token}"), &mut style, None);
            assert_eq!(style.padding, expected, "{token}");
            apply_class(&format!("w-{token}"), &mut style, None);
            assert_eq!(style.width, SizeHint::Fixed(expected), "{token}");
        }
    }

    #[test]
    fn test_apply_class_spacing() {
        let mut s = EmbeddedStyle::default();
        apply_class("p-4", &mut s, None);
        assert_eq!(s.padding, 16);

        apply_class("px-2", &mut s, None);
        assert_eq!(s.padding_x, 8);
    }

    #[test]
    fn test_apply_class_size() {
        let mut s = EmbeddedStyle::default();
        apply_class("w-full", &mut s, None);
        assert_eq!(s.width, SizeHint::Fill);

        apply_class("h-4", &mut s, None);
        assert_eq!(s.height, SizeHint::Fixed(16));
    }

    #[test]
    fn test_apply_class_flex() {
        let mut s = EmbeddedStyle::default();
        apply_class("flex-col", &mut s, None);
        assert_eq!(s.flex_dir, FlexDir::Column);

        apply_class("items-center", &mut s, None);
        assert_eq!(s.align, Align::Center);
    }
}
