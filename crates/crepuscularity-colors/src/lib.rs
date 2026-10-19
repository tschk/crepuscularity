//! Shared Tailwind palette for host and no-std renderers.
#![no_std]

mod palette;
pub use palette::lookup_named_color;

/// Resolve a palette color (e.g. `"red-500"` or `"white"`) to `0xRRGGBB`.
pub fn lookup_color_u32(name: &str) -> Option<u32> {
    let hex = lookup_named_color(name)?;
    let h = hex.trim_start_matches('#');
    let value = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(value),
        3 => {
            let r = (value >> 8) & 0xf;
            let g = (value >> 4) & 0xf;
            let b = value & 0xf;
            Some(((r * 17) << 16) | ((g * 17) << 8) | (b * 17))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn complete_palette_resolves_to_matching_numeric_colors() {
        for family in [
            "amber", "blue", "cyan", "emerald", "fuchsia", "gray", "green", "indigo", "lime",
            "mauve", "mist", "neutral", "olive", "orange", "pink", "purple", "red", "rose", "sky",
            "slate", "stone", "taupe", "teal", "violet", "yellow", "zinc",
        ] {
            for shade in [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] {
                let name = std::format!("{family}-{shade}");
                let hex = lookup_named_color(&name).expect("palette shade missing");
                let numeric = lookup_color_u32(&name).expect("palette color cannot be decoded");
                assert_eq!(std::format!("#{numeric:06x}"), hex, "{name}");
            }
        }
    }

    #[test]
    fn test_lookup_named_color() {
        // Happy paths
        assert_eq!(lookup_named_color("amber-500"), Some("#fe9a00"));
        assert_eq!(lookup_named_color("blue-50"), Some("#eff6ff"));
        assert_eq!(lookup_named_color("white"), Some("#fff"));
        assert_eq!(lookup_named_color("black"), Some("#000"));
        assert_eq!(lookup_named_color("zinc-950"), Some("#09090b"));

        // Edge cases and error conditions
        assert_eq!(lookup_named_color(""), None);
        assert_eq!(lookup_named_color("nonexistent-color"), None);
        assert_eq!(lookup_named_color("AMBER-500"), None); // case-sensitive
        assert_eq!(lookup_named_color("amber-501"), None);
    }

    #[test]
    fn test_lookup_color_u32() {
        // Happy paths for 6-digit hex
        assert_eq!(lookup_color_u32("amber-500"), Some(0xfe9a00));
        assert_eq!(lookup_color_u32("zinc-950"), Some(0x09090b));
        assert_eq!(lookup_color_u32("blue-50"), Some(0xeff6ff));

        assert_eq!(lookup_color_u32("white"), Some(0xffffff));
        assert_eq!(lookup_color_u32("black"), Some(0));

        // Invalid names
        assert_eq!(lookup_color_u32("nonexistent-color"), None);
        assert_eq!(lookup_color_u32(""), None);
    }
}
