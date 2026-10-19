//! Shared Tailwind CSS v4 utility parsing (colors, spacing, sizing).
//!
//! Used by embedded, native, and other backends for consistent class semantics.

pub mod colors;
pub mod length;
pub mod parse;

pub use colors::{lookup_color_u32, lookup_named_color};
pub use length::{parse_arbitrary_length_token, parse_length_token, LengthToken};
pub use parse::{
    parse_color_rgb, parse_css_hex_color, parse_font_size_named, parse_fraction,
    parse_opacity_percentage, parse_radius_px, parse_size_pt, parse_size_width_height,
    parse_spacing_pt, parse_spacing_px, parse_text_size_px, resolve_arbitrary_css_color,
    resolve_css_color, SizeToken, SIZE_FILL, SIZE_FIT,
};
