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

// The std build uses crepuscularity-core's parsers. This copy serves no_std and is also
// compiled into std tests, which check it against core.
#[cfg(any(not(feature = "std"), test))]
#[cfg_attr(feature = "std", allow(dead_code))]
mod tailwind_parse {
    use crepuscularity_colors::lookup_color_u32;

    pub enum SizeToken {
        Full,
        Auto,
        Px(u16),
        Fraction { num: u16, den: u16 },
        Spacing(u16),
    }

    /// Mirrors `crepuscularity_core::tailwind::parse::parse_spacing_multiplier_px`:
    /// canonical non-negative multiples of 0.25 (Tailwind v4), 1 unit = 4 px.
    fn parse_spacing_multiplier_px(rest: &str) -> Option<f32> {
        let (whole, fraction) = rest.split_once('.').unwrap_or((rest, ""));
        let canonical_whole = whole == "0" || (!whole.starts_with('0') && !whole.is_empty());
        if !canonical_whole || !whole.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let quarter = match (rest.contains('.'), fraction) {
            (false, _) => 0,
            (true, "25") => 1,
            (true, "5") => 2,
            (true, "75") => 3,
            _ => return None,
        };
        let px = whole.parse::<f32>().ok()? * 4.0 + quarter as f32;
        px.is_finite().then_some(px)
    }

    /// Mirrors core's `parse_spacing_px`: whole, non-negative px that fit in `u16`.
    pub fn parse_spacing_px(rest: &str) -> Option<u16> {
        let px = if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            inner
                .strip_suffix("px")
                .unwrap_or(inner)
                .parse::<f32>()
                .ok()?
        } else if rest == "px" {
            1.0
        } else {
            parse_spacing_multiplier_px(rest)?
        };
        // `f32::fract` needs std; a round trip through u16 checks whole and in range.
        (px >= 0.0 && px <= f32::from(u16::MAX) && f32::from(px as u16) == px).then_some(px as u16)
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
        let name = name.trim();
        if let Some((color_part, _)) = name.split_once('/') {
            return parse_color_rgb(color_part);
        }
        if let Some(inner) = name.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return parse_color_rgb(inner);
        }
        // Numeric lookup expands 3-digit palette entries such as `white` (`#fff`), as std does.
        if let Some(rgb) = lookup_color_u32(name) {
            return Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
        }
        if name.starts_with('#') {
            return parse_hex_rgb(name);
        }
        None
    }

    fn parse_hex_rgb(s: &str) -> Option<[u8; 3]> {
        let t = s.trim().trim_start_matches('#');
        if t.len() < 6 {
            return None;
        }
        let t = &t[..6];
        Some([
            u8::from_str_radix(&t[0..2], 16).ok()?,
            u8::from_str_radix(&t[2..4], 16).ok()?,
            u8::from_str_radix(&t[4..6], 16).ok()?,
        ])
    }
}

#[cfg(not(feature = "std"))]
use tailwind_parse::{parse_color_rgb, parse_size_width_height, parse_spacing_px, SizeToken};

#[cfg(feature = "std")]
pub fn apply_classes(
    classes: &[impl AsRef<str>],
    s: &mut EmbeddedStyle,
    ctx: Option<&TemplateContext>,
) {
    for c in classes {
        apply_class(c.as_ref(), s, ctx);
    }
}

#[cfg(not(feature = "std"))]
pub fn apply_classes(classes: &[impl AsRef<str>], s: &mut EmbeddedStyle, ctx: Option<&()>) {
    for c in classes {
        apply_class(c.as_ref(), s, ctx);
    }
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
        "flex-none" | "grow-0" => {}
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
        "invisible" => s.opacity_percent = Some(0),
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
            if s.border_width == 0 {
                s.border_width = 1;
            }
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

    /// Every canonical multiple of 0.25 (Tailwind v4) is 4 px per unit in both builds;
    /// anything else leaves the padding unset.
    #[test]
    fn spacing_accepts_tailwind_v4_multipliers_in_both_builds() {
        for quarters in 0..=400u16 {
            let token = match quarters % 4 {
                0 => format!("{}", quarters / 4),
                1 => format!("{}.25", quarters / 4),
                2 => format!("{}.5", quarters / 4),
                _ => format!("{}.75", quarters / 4),
            };
            assert_eq!(parse_spacing_px(&token), Some(quarters), "{token}");
            let mut s = EmbeddedStyle::default();
            apply_class(&format!("p-{token}"), &mut s, None);
            assert_eq!(s.padding, quarters, "p-{token}");
        }
        for token in ["-2", "0.3", "1.50", "01", ".5", "+4", "1e1", "16384"] {
            assert_eq!(parse_spacing_px(token), None, "{token}");
            let mut s = EmbeddedStyle::default();
            apply_class(&format!("p-{token}"), &mut s, None);
            assert_eq!(s.padding, 0, "p-{token}");
        }
    }

    /// The no_std copy must agree with core, which the std build uses.
    #[cfg(feature = "std")]
    #[test]
    fn no_std_spacing_parser_matches_core() {
        let mut tokens: Vec<String> = (0..=2000u32)
            .flat_map(|n| [format!("{}", n / 4), format!("{}.{}", n / 4, n % 10)])
            .collect();
        tokens.extend(
            [
                "px", "0.25", "4.75", "-2", "01", ".5", "+4", "1e1", "16384", "16383.75", "[20px]",
                "[20]", "[20.5px]", "[20.0px]", "[-2px]", "[65535]", "[65536]", "[1e1px]", "[inf]",
                "[NaN]", "[]", "full", "",
            ]
            .map(String::from),
        );
        for token in &tokens {
            assert_eq!(
                super::tailwind_parse::parse_spacing_px(token),
                crepuscularity_core::tailwind::parse::parse_spacing_px(token),
                "{token}"
            );
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
