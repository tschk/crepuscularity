//! Thin GPUI / Zed integration surface for crepuscularity-components.
//!
//! Path-include or copy this directory into a `crepuscularity-gpui` crate:
//!
//! ```ignore
//! // In your crate root / lib.rs:
//! #[path = "path/to/crepuscularity-components/packages/gpui/mod.rs"]
//! mod crepuscularity_components_gpui;
//! use crepuscularity_components_gpui::{palette, sparkline};
//! ```
//!
//! Or as a module tree:
//!
//! ```ignore
//! // packages/gpui/lib.rs (crate root when published as crepuscularity-gpui)
//! pub mod palette;
//! pub mod sparkline;
//! pub use palette::*;
//! pub use sparkline::{sparkline_alphas, sparkline_alphas_for_size, Variant, BAYER};
//! ```
//!
//! # Consuming themes
//!
//! Theme JSON lives in `catalog/themes/*.json`. GPUI apps should:
//! 1. Load a theme by name (`dither-kit`, `kumo`, `night`, `chalk`, `aurora`,
//!    `dawn`, `zinc`) — typically via `include_str!` or a build-time embed.
//! 2. Map `seeds.<color>.fill` → `gpui::Hsla` / `Rgba` for chart fills.
//! 3. Use [`palette`] constants for the common dither-kit / kumo seeds without
//!    parsing JSON at runtime.
//! 4. Call [`sparkline::sparkline_alphas`] to get Bayer cell coverage, then
//!    paint with `gpui::Canvas` / `PaintQuad` (deferred — not in this stub).

pub mod palette;
pub mod sparkline;

pub use palette::{
    DITHER_BLUE, DITHER_BLUE_FILL, DITHER_GREEN, DITHER_GREY, DITHER_KIT_SEEDS, DITHER_ORANGE,
    DITHER_PINK, DITHER_PURPLE, DITHER_RED, KUMO_BLUE, KUMO_CATEGORICAL, KUMO_ORANGE, KUMO_PINK,
    KUMO_PURPLE, KUMO_TEAL, KUMO_YELLOW, THEME_NAMES, Rgb, Seed,
};
pub use sparkline::{
    backing_size, clamp01, resample, sparkline_alphas, sparkline_alphas_for_size, Variant, BAYER,
    BORDER_ALPHA, CELL, MAX_COLS, MAX_ROWS, OFF_TIER,
};
