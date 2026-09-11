//! Crate-style entry for a future `crepuscularity-gpui` package.
//!
//! When published, this file is the crate root. Until then, path-include
//! [`mod.rs`](mod.rs) from a Zed/GPUI workspace, or copy `palette.rs` +
//! `sparkline.rs` and re-export them the same way.
//!
//! ```ignore
//! // Cargo.toml
//! // [dependencies]
//! // crepuscularity-components-gpui = { path = "plugins/crepuscularity-components/packages/gpui" }
//!
//! use crepuscularity_components_gpui::{DITHER_BLUE_FILL, sparkline_alphas, Variant};
//!
//! let alphas = sparkline_alphas(&[1.0, 3.0, 2.0], 32, 16, Variant::Gradient, 0.0);
//! // paint alphas onto gpui::Canvas …
//! ```

pub mod palette;
pub mod sparkline;

pub use palette::{
    Rgb, Seed, DITHER_BLUE, DITHER_BLUE_FILL, DITHER_GREEN, DITHER_GREY, DITHER_KIT_SEEDS,
    DITHER_ORANGE, DITHER_PINK, DITHER_PURPLE, DITHER_RED, KUMO_BLUE, KUMO_CATEGORICAL,
    KUMO_ORANGE, KUMO_PINK, KUMO_PURPLE, KUMO_TEAL, KUMO_YELLOW, THEME_NAMES,
};
pub use sparkline::{
    backing_size, clamp01, resample, sparkline_alphas, sparkline_alphas_for_size, Variant, BAYER,
    BORDER_ALPHA, CELL, MAX_COLS, MAX_ROWS, OFF_TIER,
};
