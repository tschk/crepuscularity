//! Tailwind CSS v4 colors and CSS color resolution for native View IR.

pub use crepuscularity_core::tailwind::{lookup_color_u32, lookup_named_color};

/// Parse an opacity suffix like `/50` → `0.5`, `/75` → `0.75`.
pub fn parse_opacity_suffix(s: &str) -> Option<f32> {
    let n: u8 = s.parse().ok()?;
    if n > 100 {
        return None;
    }
    Some(n as f32 / 100.0)
}

/// Resolve any CSS-style color string to RGBA float components (0.0–1.0 each).
pub fn resolve_rgba(css: &str) -> Option<[f32; 4]> {
    let lower = css.trim().to_lowercase();
    match lower.as_str() {
        "red" => return Some([1.0, 0.0, 0.0, 1.0]),
        "blue" => return Some([0.0, 0.0, 1.0, 1.0]),
        "green" => return Some([0.0, 0.502, 0.0, 1.0]),
        "white" => return Some([1.0, 1.0, 1.0, 1.0]),
        "black" => return Some([0.0, 0.0, 0.0, 1.0]),
        "gray" | "grey" => return Some([0.502, 0.502, 0.502, 1.0]),
        "clear" | "transparent" => return Some([0.0, 0.0, 0.0, 0.0]),
        "orange" => return Some([1.0, 0.647, 0.0, 1.0]),
        "yellow" => return Some([1.0, 1.0, 0.0, 1.0]),
        "purple" => return Some([0.502, 0.0, 0.502, 1.0]),
        "pink" => return Some([1.0, 0.753, 0.796, 1.0]),
        _ => {}
    }

    if let Some(hex) = lookup_named_color(lower.as_str()) {
        return parse_hex_rgba(hex);
    }

    if lower.starts_with('#') {
        return parse_hex_rgba(lower.as_str());
    }

    None
}

fn parse_hex_rgba(s: &str) -> Option<[f32; 4]> {
    let hex = s.trim_start_matches('#');
    match hex.len() {
        6 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some([
                ((n >> 16) & 0xFF) as f32 / 255.0,
                ((n >> 8) & 0xFF) as f32 / 255.0,
                (n & 0xFF) as f32 / 255.0,
                1.0,
            ])
        }
        8 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some([
                ((n >> 24) & 0xFF) as f32 / 255.0,
                ((n >> 16) & 0xFF) as f32 / 255.0,
                ((n >> 8) & 0xFF) as f32 / 255.0,
                (n & 0xFF) as f32 / 255.0,
            ])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_named() {
        let [r, g, b, a] = resolve_rgba("red").unwrap();
        assert_eq!([r, g, b, a], [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn tailwind_named() {
        let [r, g, b, a] = resolve_rgba("slate-500").unwrap();
        assert!((r - 0x62 as f32 / 255.0).abs() < 0.02);
        assert!((g - 0x74 as f32 / 255.0).abs() < 0.02);
        assert!((b - 0x8e as f32 / 255.0).abs() < 0.02);
        assert_eq!(a, 1.0);
    }

    #[test]
    fn hex6() {
        let [r, g, b, a] = resolve_rgba("#ff0000").unwrap();
        assert_eq!([r, g, b, a], [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn hex8_alpha() {
        let [r, g, b, a] = resolve_rgba("#ff000080").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);
        assert!((a - 0.502).abs() < 0.01);
    }

    #[test]
    fn transparent() {
        let [_, _, _, a] = resolve_rgba("transparent").unwrap();
        assert_eq!(a, 0.0);
    }

    #[test]
    fn unknown_returns_none() {
        assert!(resolve_rgba("primary").is_none());
        assert!(resolve_rgba("chartreuse-deluxe").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(resolve_rgba("RED").is_some());
        assert!(resolve_rgba("Slate-500").is_some());
        assert!(resolve_rgba("#FF0000").is_some());
    }

    #[test]
    fn test_parse_opacity_suffix_valid() {
        assert_eq!(parse_opacity_suffix("0"), Some(0.0));
        assert_eq!(parse_opacity_suffix("50"), Some(0.5));
        assert_eq!(parse_opacity_suffix("100"), Some(1.0));
        assert_eq!(parse_opacity_suffix("75"), Some(0.75));
        assert_eq!(parse_opacity_suffix("33"), Some(0.33));
    }

    #[test]
    fn test_parse_opacity_suffix_invalid() {
        assert_eq!(parse_opacity_suffix("101"), None);
        assert_eq!(parse_opacity_suffix("200"), None);
        assert_eq!(parse_opacity_suffix("255"), None);
        assert_eq!(parse_opacity_suffix("-1"), None);
        assert_eq!(parse_opacity_suffix("abc"), None);
        assert_eq!(parse_opacity_suffix(""), None);
        assert_eq!(parse_opacity_suffix(" "), None);
    }
}
