//! Packed pixel colors for embedded framebuffers.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Rgb565(Rgb565),
    Rgb888(Rgb888),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb565(pub u16);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb888 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for Rgb888 {
    fn default() -> Self {
        Self::BLACK
    }
}

impl Rgb888 {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn to_u32(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

impl From<Rgb565> for Color {
    fn from(c: Rgb565) -> Self {
        Color::Rgb565(c)
    }
}

impl From<Rgb888> for Color {
    fn from(c: Rgb888) -> Self {
        Color::Rgb888(c)
    }
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgb888(Rgb888 { r, g, b })
    }

    pub fn from_rgba_f32(r: f32, g: f32, b: f32, _a: f32) -> Self {
        fn chan(v: f32) -> u8 {
            let v = v.clamp(0.0, 1.0) * 255.0;
            if v >= 255.0 {
                255
            } else {
                v as u8
            }
        }
        Color::rgb(chan(r), chan(g), chan(b))
    }

    pub fn to_rgb565(self) -> Rgb565 {
        match self {
            Color::Rgb565(c) => c,
            Color::Rgb888(c) => c.to_rgb565(),
        }
    }

    pub fn to_rgb888(self) -> Rgb888 {
        match self {
            Color::Rgb565(c) => c.to_rgb888(),
            Color::Rgb888(c) => c,
        }
    }

    pub fn to_u32(self) -> u32 {
        self.to_rgb888().to_u32()
    }
}

impl Rgb888 {
    pub fn to_rgb565(self) -> Rgb565 {
        Rgb565(pack_rgb565(self.r, self.g, self.b))
    }
}

impl Rgb565 {
    pub const fn from_components(r: u8, g: u8, b: u8) -> Self {
        Self(pack_rgb565(r, g, b))
    }

    pub fn to_rgb888(self) -> Rgb888 {
        let v = self.0;
        Rgb888 {
            r: ((((v >> 11) & 0x1F) * 255) / 31) as u8,
            g: ((((v >> 5) & 0x3F) * 255) / 63) as u8,
            b: (((v & 0x1F) * 255) / 31) as u8,
        }
    }
}

pub const fn pack_rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16) >> 3) << 11 | ((g as u16) >> 2) << 5 | (b as u16) >> 3
}

pub fn parse_hex(s: &str) -> Option<Color> {
    let trimmed = s.trim();
    let t = trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("0x"))
        .unwrap_or(trimmed);
    if !matches!(t.len(), 3 | 6) || !t.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    if t.len() == 3 {
        return Some(Color::rgb(
            u8::from_str_radix(&t[0..1], 16).ok()? * 17,
            u8::from_str_radix(&t[1..2], 16).ok()? * 17,
            u8::from_str_radix(&t[2..3], 16).ok()? * 17,
        ));
    }
    Some(Color::rgb(
        u8::from_str_radix(&t[0..2], 16).ok()?,
        u8::from_str_radix(&t[2..4], 16).ok()?,
        u8::from_str_radix(&t[4..6], 16).ok()?,
    ))
}

pub fn lookup_named_color(name: &str) -> Option<Color> {
    crepuscularity_colors::lookup_named_color(name).and_then(parse_hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rgb565_value_round_trips_through_rgb888() {
        for raw in 0..=u16::MAX {
            let packed = Rgb565(raw);
            assert_eq!(packed.to_rgb888().to_rgb565(), packed, "{raw:#06x}");
        }
        for (raw, rgb) in [
            (0xf800, Rgb888::new(255, 0, 0)),
            (0x07e0, Rgb888::new(0, 255, 0)),
            (0x001f, Rgb888::new(0, 0, 255)),
            (0xffff, Rgb888::new(255, 255, 255)),
            (0x0000, Rgb888::BLACK),
        ] {
            assert_eq!(Rgb565(raw).to_rgb888(), rgb);
            assert_eq!(rgb.to_rgb565(), Rgb565(raw));
        }
    }

    #[test]
    fn shared_palette_and_short_hex_work_without_parser_features() {
        assert_eq!(
            lookup_named_color("white").map(Color::to_u32),
            Some(0xffffff)
        );
        assert_eq!(lookup_named_color("black").map(Color::to_u32), Some(0));
        assert_eq!(
            lookup_named_color("blue-500").map(Color::to_u32),
            Some(0x2b7fff)
        );
        assert_eq!(parse_hex("#aB1").map(Color::to_u32), Some(0xaabb11));
    }

    #[test]
    fn malformed_unicode_hex_returns_none() {
        for input in ["界abc", "a界bc", "ab界c", "abc界", "#a界bc", "0xab界c"] {
            assert!(parse_hex(input).is_none(), "{input}");
        }
        assert_eq!(parse_hex("#aBcD12").map(Color::to_u32), Some(0xabcd12));
        assert!(parse_hex("#gg0011").is_none());
        for input in ["##ff0000", "0x0xff0000", "#0xff0000", "###abc"] {
            assert!(parse_hex(input).is_none(), "{input}");
        }
        assert_eq!(parse_hex(" 0xabc ").map(Color::to_u32), Some(0xaabbcc));
        assert_eq!(parse_hex("ff0000").map(Color::to_u32), Some(0xff0000));
    }
}
