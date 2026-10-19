//! Seed RGB constants for future crepuscularity-gpui dither charts.
//! Values mirror catalog/themes/dither-kit.json and kumo.json.

/// RGB triple in 0–255.
pub type Rgb = [u8; 3];

pub struct Seed {
    pub fill: Rgb,
    pub line: Rgb,
    pub star: Rgb,
}

// --- dither-kit ---

pub const DITHER_GREEN: Seed = Seed {
    fill: [40, 210, 110],
    line: [150, 255, 180],
    star: [200, 255, 220],
};
pub const DITHER_BLUE: Seed = Seed {
    fill: [53, 143, 243],
    line: [150, 200, 255],
    star: [205, 228, 255],
};
pub const DITHER_PURPLE: Seed = Seed {
    fill: [150, 110, 255],
    line: [200, 175, 255],
    star: [225, 210, 255],
};
pub const DITHER_PINK: Seed = Seed {
    fill: [240, 90, 190],
    line: [255, 170, 220],
    star: [255, 205, 235],
};
pub const DITHER_ORANGE: Seed = Seed {
    fill: [255, 150, 50],
    line: [255, 195, 130],
    star: [255, 220, 175],
};
pub const DITHER_RED: Seed = Seed {
    fill: [240, 70, 70],
    line: [255, 150, 140],
    star: [255, 195, 185],
};
pub const DITHER_GREY: Seed = Seed {
    fill: [92, 92, 100],
    line: [140, 140, 150],
    star: [165, 165, 175],
};

pub const DITHER_BLUE_FILL: Rgb = DITHER_BLUE.fill;

/// Ordered dither-kit seeds (green → grey).
pub const DITHER_KIT_SEEDS: &[(&str, Seed)] = &[
    ("green", DITHER_GREEN),
    ("blue", DITHER_BLUE),
    ("purple", DITHER_PURPLE),
    ("pink", DITHER_PINK),
    ("orange", DITHER_ORANGE),
    ("red", DITHER_RED),
    ("grey", DITHER_GREY),
];

// --- Cloudflare Kumo categorical (light) ---

pub const KUMO_BLUE: Rgb = [66, 144, 240];
pub const KUMO_YELLOW: Rgb = [245, 182, 71];
pub const KUMO_PINK: Rgb = [232, 100, 157];
pub const KUMO_PURPLE: Rgb = [141, 88, 238];
pub const KUMO_TEAL: Rgb = [80, 195, 182];
pub const KUMO_ORANGE: Rgb = [211, 117, 54];

pub const KUMO_CATEGORICAL: &[Rgb] = &[
    KUMO_BLUE,
    KUMO_YELLOW,
    KUMO_PINK,
    KUMO_PURPLE,
    KUMO_TEAL,
    KUMO_ORANGE,
];

pub const THEME_NAMES: &[&str] = &[
    "dither-kit",
    "kumo",
    "night",
    "chalk",
    "aurora",
    "dawn",
    "zinc",
];
