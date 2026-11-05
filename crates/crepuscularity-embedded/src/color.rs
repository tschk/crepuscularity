//! Packed pixel colors for embedded framebuffers.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Rgb565(Rgb565),
    Rgb888(Rgb888),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb565(pub u16);

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
    let t = s.trim().trim_start_matches('#').trim_start_matches("0x");
    if t.len() != 6 {
        return None;
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
