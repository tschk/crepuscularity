//! Tailwind spacing / sizing / color token parsing (backend-agnostic).

use super::colors::lookup_named_color;

/// Tailwind spacing scale: `1` → 4 px unless arbitrary `[Npx]`.
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
        "7" => Some(28),
        "8" => Some(32),
        "9" => Some(36),
        "10" => Some(40),
        "11" => Some(44),
        "12" => Some(48),
        "14" => Some(56),
        "16" => Some(64),
        "20" => Some(80),
        "24" => Some(96),
        "32" => Some(128),
        "36" => Some(144),
        "40" => Some(160),
        "44" => Some(176),
        "48" => Some(192),
        "52" => Some(208),
        "56" => Some(224),
        "60" => Some(240),
        "64" => Some(256),
        "72" => Some(288),
        "80" => Some(320),
        "96" => Some(384),
        _ => rest.parse::<u16>().ok().map(|n| n.saturating_mul(4)),
    }
}

/// Width/height token after `w-` / `h-` prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeToken {
    Full,
    Auto,
    Px(u16),
    Fraction { num: u16, den: u16 },
    Spacing(u16),
}

pub fn parse_size_width_height(rest: &str) -> Option<SizeToken> {
    match rest {
        "full" | "screen" => return Some(SizeToken::Full),
        "auto" | "fit" | "min" | "max" => return Some(SizeToken::Auto),
        "px" => return Some(SizeToken::Px(1)),
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

pub fn parse_radius_px(rest: &str) -> Option<u16> {
    if rest.is_empty() {
        return Some(4);
    }
    if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let stripped = inner.strip_suffix("px").unwrap_or(inner);
        return stripped.parse::<u16>().ok();
    }
    match rest {
        "none" => Some(0),
        "sm" => Some(2),
        "md" => Some(6),
        "lg" => Some(8),
        "xl" => Some(12),
        "2xl" => Some(16),
        "3xl" => Some(24),
        "full" => Some(999),
        _ => rest.parse::<u16>().ok(),
    }
}

pub fn parse_text_size_px(rest: &str) -> Option<u16> {
    if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let stripped = inner.strip_suffix("px").unwrap_or(inner);
        return stripped.parse::<u16>().ok();
    }
    if let Some(v) = parse_font_size_named(rest) {
        return Some(v as u16);
    }
    rest.parse::<u16>().ok()
}

/// The named `text-*` font-size ramp only — no arbitrary values, no bare numbers.
///
/// This is the canonical ramp; [`parse_text_size_px`] layers the permissive
/// fallbacks on top for backends that want them.
pub fn parse_font_size_named(rest: &str) -> Option<f32> {
    Some(match rest {
        "xs" => 12.0,
        "sm" => 14.0,
        "base" => 16.0,
        "lg" => 18.0,
        "xl" => 20.0,
        "2xl" => 24.0,
        "3xl" => 30.0,
        "4xl" => 36.0,
        "5xl" => 48.0,
        "6xl" => 60.0,
        "7xl" => 72.0,
        "8xl" => 96.0,
        "9xl" => 128.0,
        _ => return None,
    })
}

// ── Point-valued scale (View IR canonical) ───────────────────────────────────

/// `width`/`height` sentinel meaning "fill the parent" (`w-full`, `w-screen`).
pub const SIZE_FILL: f32 = -1.0;
/// `width`/`height` sentinel meaning "size to content" (`w-fit`, `w-auto`).
pub const SIZE_FIT: f32 = -2.0;

/// Tailwind spacing scale in points: 1 unit = 4 pt.
///
/// Accepts arbitrary `[N]` / `[Npx]` and any decimal step (`0.5` → 2.0).
pub fn parse_spacing_pt(rest: &str) -> Option<f32> {
    if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let stripped = inner.strip_suffix("px").unwrap_or(inner);
        return stripped.parse::<f32>().ok();
    }
    match rest {
        "px" => return Some(1.0),
        "0" => return Some(0.0),
        _ => {}
    }
    rest.parse::<f32>().ok().map(|f| f * 4.0)
}

/// Parse `"1/2"` → `Some(0.5)`, `"2/3"` → `Some(0.667)`.
pub fn parse_fraction(s: &str) -> Option<f32> {
    let (num, den) = s.split_once('/')?;
    let n: f32 = num.parse().ok()?;
    let d: f32 = den.parse().ok()?;
    if d == 0.0 {
        return None;
    }
    Some(n / d)
}

/// Width/height token in points, or a sentinel.
///
/// - `> 0` — absolute points
/// - [`SIZE_FILL`] — fill parent
/// - [`SIZE_FIT`] — fit content
/// - negative fraction (`-0.5` for `w-1/2`) — proportion of the parent
pub fn parse_size_pt(rest: &str) -> Option<f32> {
    match rest {
        "full" | "screen" => return Some(SIZE_FILL),
        "fit" | "auto" | "min" | "max" => return Some(SIZE_FIT),
        "px" => return Some(1.0),
        _ => {}
    }
    if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let stripped = inner.strip_suffix("px").unwrap_or(inner);
        return stripped.parse::<f32>().ok().map(|v| v.max(0.0));
    }
    // Fractions are deliberately not encoded here. This value shares a field
    // with the SIZE_FILL / SIZE_FIT sentinels, and a fraction has no point
    // value, so `-(n/d)` produced a number no consumer can decode — and
    // `1/1` collided exactly with SIZE_FILL. Callers that can express a
    // fraction use `parse_fraction` directly; web targets get it from the
    // preserved class token.
    if parse_fraction(rest).is_some() {
        return None;
    }
    parse_spacing_pt(rest)
}

// ── CSS colour strings ───────────────────────────────────────────────────────

/// Resolve a colour token to a CSS-ish hex string: named palette, bare hex,
/// `#hex`, or `family-shade/opacity` (encoded as `"{hex}%{alpha:02x}"`).
pub fn resolve_css_color(s: &str) -> Option<String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "black" => return Some("#000000".to_string()),
        "white" => return Some("#ffffff".to_string()),
        "transparent" | "clear" => return Some("#00000000".to_string()),
        _ => {}
    }
    if let Some((color_part, opacity_part)) = s.split_once('/') {
        if let Some(hex) = lookup_named_color(color_part) {
            if let Ok(pct) = opacity_part.parse::<u8>() {
                let alpha = (pct as f32 / 100.0 * 255.0).round() as u8;
                return Some(format!("{}%{:02x}", hex, alpha));
            }
        }
    }
    if let Some(hex) = lookup_named_color(s) {
        return Some(hex.to_string());
    }
    parse_css_hex_color(s)
}

/// Accept a 6- or 8-digit hex colour, with or without a `#` / `0x` prefix.
pub fn parse_css_hex_color(s: &str) -> Option<String> {
    let t = s.trim();
    let hex = t
        .strip_prefix('#')
        .or_else(|| t.strip_prefix("0x"))
        .unwrap_or(t);
    if (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!("#{}", hex));
    }
    None
}

/// Resolve an arbitrary-value bracket colour: `[#0f0f0f]`, `[red-500]`, `[rebeccapurple]`.
pub fn resolve_arbitrary_css_color(rest: &str) -> Option<String> {
    let inner = rest.strip_prefix('[')?.strip_suffix(']')?;
    resolve_css_color(inner).or_else(|| {
        if inner.chars().all(|c| c.is_alphabetic() || c == '-') {
            Some(inner.to_string())
        } else {
            None
        }
    })
}

/// Resolve `red-500`, `#fff`, `bg-[#0f0]`, or `red-500/50` to RGB bytes.
pub fn parse_color_rgb(name: &str) -> Option<[u8; 3]> {
    let name = name.trim();
    if let Some((color_part, _opacity)) = name.split_once('/') {
        return parse_color_rgb(color_part);
    }
    if let Some(inner) = name.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return parse_color_rgb(inner);
    }
    if let Some(hex) = lookup_named_color(name) {
        return parse_named_hex_rgb(hex);
    }
    if name.starts_with('#') || name.starts_with("0x") {
        return parse_hex_rgb(name);
    }
    None
}

fn parse_named_hex_rgb(s: &str) -> Option<[u8; 3]> {
    let t = s.trim().trim_start_matches('#').trim_start_matches("0x");
    if t.len() == 3 {
        let r = u8::from_str_radix(&t[0..1], 16).ok()?;
        let g = u8::from_str_radix(&t[1..2], 16).ok()?;
        let b = u8::from_str_radix(&t[2..3], 16).ok()?;
        return Some([r * 17, g * 17, b * 17]);
    }
    parse_hex_rgb(s)
}

fn parse_hex_rgb(s: &str) -> Option<[u8; 3]> {
    let t = s.trim().trim_start_matches('#').trim_start_matches("0x");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_scale() {
        assert_eq!(parse_spacing_px("4"), Some(16));
        assert_eq!(parse_spacing_px("[20px]"), Some(20));
    }

    #[test]
    fn test_parse_size_width_height() {
        // Exact string matches
        assert_eq!(parse_size_width_height("full"), Some(SizeToken::Full));
        assert_eq!(parse_size_width_height("screen"), Some(SizeToken::Full));
        assert_eq!(parse_size_width_height("auto"), Some(SizeToken::Auto));
        assert_eq!(parse_size_width_height("fit"), Some(SizeToken::Auto));
        assert_eq!(parse_size_width_height("min"), Some(SizeToken::Auto));
        assert_eq!(parse_size_width_height("max"), Some(SizeToken::Auto));
        assert_eq!(parse_size_width_height("px"), Some(SizeToken::Px(1)));

        // Fractions
        assert_eq!(
            parse_size_width_height("1/2"),
            Some(SizeToken::Fraction { num: 1, den: 2 })
        );
        assert_eq!(
            parse_size_width_height("3/4"),
            Some(SizeToken::Fraction { num: 3, den: 4 })
        );
        assert_eq!(parse_size_width_height("1/0"), None); // division by zero check if present, actually d > 0 check

        // Arbitrary pixel sizes
        assert_eq!(parse_size_width_height("[10px]"), Some(SizeToken::Px(10)));
        assert_eq!(parse_size_width_height("[42]"), Some(SizeToken::Px(42)));

        // Spacing scale fallbacks
        assert_eq!(parse_size_width_height("4"), Some(SizeToken::Spacing(16)));
        assert_eq!(parse_size_width_height("1.5"), Some(SizeToken::Spacing(6)));

        // Invalid inputs
        assert_eq!(parse_size_width_height("invalid"), None);
        assert_eq!(parse_size_width_height("1/invalid"), None);
        assert_eq!(parse_size_width_height("[invalid]"), None);
        assert_eq!(parse_size_width_height(""), None);
    }

    #[test]
    fn radius_scale() {
        assert_eq!(parse_radius_px(""), Some(4));
        assert_eq!(parse_radius_px("md"), Some(6));
        assert_eq!(parse_radius_px("[14px]"), Some(14));
    }

    #[test]
    fn text_size_scale() {
        assert_eq!(parse_text_size_px("xl"), Some(20));
        assert_eq!(parse_text_size_px("2xl"), Some(24));
        assert_eq!(parse_text_size_px("[22px]"), Some(22));
    }

    #[test]
    fn spacing_pt_scale() {
        assert_eq!(parse_spacing_pt("4"), Some(16.0));
        assert_eq!(parse_spacing_pt("0"), Some(0.0));
        assert_eq!(parse_spacing_pt("px"), Some(1.0));
        assert_eq!(parse_spacing_pt("0.5"), Some(2.0));
        assert_eq!(parse_spacing_pt("1.5"), Some(6.0));
        assert_eq!(parse_spacing_pt("[20px]"), Some(20.0));
        assert_eq!(parse_spacing_pt("[20]"), Some(20.0));
        assert_eq!(parse_spacing_pt("full"), None);
    }

    #[test]
    fn spacing_pt_agrees_with_spacing_px_on_the_named_scale() {
        for token in [
            "0", "0.5", "1", "1.5", "2", "2.5", "3", "3.5", "4", "5", "6", "7", "8", "9", "10",
            "11", "12", "14", "16", "20", "24", "32", "36", "40", "44", "48", "52", "56", "60",
            "64", "72", "80", "96", "px",
        ] {
            let pt = parse_spacing_pt(token).unwrap();
            let px = parse_spacing_px(token).unwrap();
            assert_eq!(pt, px as f32, "spacing scales diverge on {token}");
        }
    }

    #[test]
    fn size_pt_sentinels() {
        assert_eq!(parse_size_pt("full"), Some(SIZE_FILL));
        assert_eq!(parse_size_pt("screen"), Some(SIZE_FILL));
        for fit in ["fit", "auto", "min", "max"] {
            assert_eq!(parse_size_pt(fit), Some(SIZE_FIT));
        }
        assert_eq!(parse_size_pt("px"), Some(1.0));
        assert_eq!(parse_size_pt("4"), Some(16.0));
        assert_eq!(parse_size_pt("[-8px]"), Some(0.0));
        assert_eq!(parse_size_pt("nope"), None);
    }

    #[test]
    fn size_pt_rejects_fractions_rather_than_faking_a_sentinel() {
        // The field these share only decodes points, SIZE_FILL and SIZE_FIT,
        // so a fraction has no representation. Encoding `-(n/d)` produced
        // values no consumer could read, and `1/1` aliased SIZE_FILL exactly.
        for frac in ["1/2", "2/3", "1/1", "3/4"] {
            assert_eq!(parse_size_pt(frac), None);
            assert!(parse_fraction(frac).is_some());
        }
    }

    #[test]
    fn fraction() {
        // Valid cases
        assert_eq!(parse_fraction("1/2"), Some(0.5));
        assert_eq!(parse_fraction("3/4"), Some(0.75));
        assert_eq!(parse_fraction("-1/2"), Some(-0.5));
        assert_eq!(parse_fraction("1/-2"), Some(-0.5));
        assert_eq!(parse_fraction("1.5/3"), Some(0.5));

        // Division by zero
        assert_eq!(parse_fraction("1/0"), None);
        assert_eq!(parse_fraction("-1/0"), None);
        assert_eq!(parse_fraction("0/0"), None);
        assert_eq!(parse_fraction("1/0.0"), None);

        // Invalid formats
        assert_eq!(parse_fraction("4"), None);
        assert_eq!(parse_fraction("1/2/3"), None);
        assert_eq!(parse_fraction("a/b"), None);
        assert_eq!(parse_fraction("1/b"), None);
        assert_eq!(parse_fraction("a/2"), None);
        assert_eq!(parse_fraction(""), None);
        assert_eq!(parse_fraction("/"), None);
        assert_eq!(parse_fraction("1/"), None);
        assert_eq!(parse_fraction("/2"), None);
    }

    #[test]
    fn font_size_named_is_a_strict_ramp() {
        assert_eq!(parse_font_size_named("xs"), Some(12.0));
        assert_eq!(parse_font_size_named("9xl"), Some(128.0));
        assert_eq!(parse_font_size_named("4"), None);
        assert_eq!(parse_font_size_named("[22px]"), None);
    }

    #[test]
    fn css_color_strings() {
        assert_eq!(resolve_css_color("black").as_deref(), Some("#000000"));
        assert_eq!(resolve_css_color("WHITE").as_deref(), Some("#ffffff"));
        assert_eq!(
            resolve_css_color("transparent").as_deref(),
            Some("#00000000")
        );
        assert_eq!(resolve_css_color("red-500").as_deref(), Some("#fb2c36"));
        assert_eq!(
            resolve_css_color("red-500/50").as_deref(),
            Some("#fb2c36%80")
        );
        assert_eq!(resolve_css_color("#aabbcc").as_deref(), Some("#aabbcc"));
        assert_eq!(
            resolve_css_color("0xaabbccdd").as_deref(),
            Some("#aabbccdd")
        );
        assert_eq!(resolve_css_color("#abc"), None);
        assert_eq!(resolve_css_color("nope"), None);
    }

    #[test]
    fn arbitrary_css_colors() {
        assert_eq!(
            resolve_arbitrary_css_color("[#ff0000]").as_deref(),
            Some("#ff0000")
        );
        assert_eq!(
            resolve_arbitrary_css_color("[red-500]").as_deref(),
            Some("#fb2c36")
        );
        assert_eq!(
            resolve_arbitrary_css_color("[rebeccapurple]").as_deref(),
            Some("rebeccapurple")
        );
        assert_eq!(resolve_arbitrary_css_color("[12345]"), None);
        assert_eq!(resolve_arbitrary_css_color("#ff0000"), None);
    }

    #[test]
    fn tailwind_v4_color() {
        let rgb = parse_color_rgb("zinc-900").unwrap();
        assert_eq!(rgb, [0x18, 0x18, 0x1b]);
        assert_eq!(parse_color_rgb("red-500").unwrap(), [0xfb, 0x2c, 0x36]);
    }

    #[test]
    fn tailwind_color_edge_cases() {
        // Opacity syntax should strip the opacity part and parse the color
        assert_eq!(parse_color_rgb("red-500/50"), Some([0xfb, 0x2c, 0x36]));

        // Arbitrary bracket syntax with hex
        assert_eq!(parse_color_rgb("[#ff0000]"), Some([0xff, 0x00, 0x00]));
        assert_eq!(parse_color_rgb("[0x00ff00]"), Some([0x00, 0xff, 0x00]));
        assert_eq!(parse_color_rgb("white"), Some([0xff, 0xff, 0xff]));

        // Direct hex values
        assert_eq!(parse_color_rgb("#0000ff"), Some([0x00, 0x00, 0xff]));
        assert_eq!(parse_color_rgb("0x0000ff"), Some([0x00, 0x00, 0xff]));

        // Invalid inputs
        assert_eq!(parse_color_rgb("unknown-color"), None);
        assert_eq!(parse_color_rgb("#123"), None); // Short hex is not supported
        assert_eq!(parse_color_rgb("[invalid]"), None);
    }

    #[test]
    fn test_parse_color_rgb_bad_hex_formats() {
        // Not enough characters
        assert_eq!(parse_color_rgb("#"), None);
        assert_eq!(parse_color_rgb("#1"), None);
        assert_eq!(parse_color_rgb("#12"), None);
        assert_eq!(parse_color_rgb("#1234"), None);
        assert_eq!(parse_color_rgb("#12345"), None);

        // Invalid hex characters
        assert_eq!(parse_color_rgb("#zzzzzz"), None);
        assert_eq!(parse_color_rgb("#12345g"), None);
        assert_eq!(parse_color_rgb("#-12345"), None);
        assert_eq!(parse_color_rgb("#      "), None);

        // 0x prefix
        assert_eq!(parse_color_rgb("0xzzzzzz"), None);
        assert_eq!(parse_color_rgb("0x123"), None);
    }

    #[test]
    fn test_parse_css_hex_color() {
        // Valid 6-digit hex
        assert_eq!(parse_css_hex_color("#123456").as_deref(), Some("#123456"));
        assert_eq!(parse_css_hex_color("123456").as_deref(), Some("#123456"));
        assert_eq!(parse_css_hex_color("0x123456").as_deref(), Some("#123456"));

        // Valid 8-digit hex
        assert_eq!(
            parse_css_hex_color("#12345678").as_deref(),
            Some("#12345678")
        );
        assert_eq!(
            parse_css_hex_color("12345678").as_deref(),
            Some("#12345678")
        );
        assert_eq!(
            parse_css_hex_color("0x12345678").as_deref(),
            Some("#12345678")
        );

        // Spaces
        assert_eq!(parse_css_hex_color(" #abcdef ").as_deref(), Some("#abcdef"));
        assert_eq!(
            parse_css_hex_color("  0xabcdef  ").as_deref(),
            Some("#abcdef")
        );

        // Invalid length
        assert_eq!(parse_css_hex_color("#123"), None);
        assert_eq!(parse_css_hex_color("12345"), None);
        assert_eq!(parse_css_hex_color("#1234567"), None);
        assert_eq!(parse_css_hex_color("123456789"), None);

        // Invalid characters
        assert_eq!(parse_css_hex_color("#12345g"), None);
        assert_eq!(parse_css_hex_color("12 456"), None);
        assert_eq!(parse_css_hex_color("0x-12345"), None);
    }
}
