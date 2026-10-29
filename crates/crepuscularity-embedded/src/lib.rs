//! **UNSTABLE — still in development; largely untested on real hardware.** APIs may change before 1.0.
//! Validate with host tests and `crepus embedded snapshot` before relying on SPI/LTDC/ESP paths.
//!
//! Framebuffer rendering for `.crepus` on embedded displays and firmware.
//!
//! Same workflow as the `crepuscularity-tui` or `crepuscularity-native` crates: depend on this crate,
//! build a [`TemplateContext`], render a template into your surface, use the retained
//! [`EmbeddedDocument`] for layout bounds and hit-testing.
//!
//! # Quick start (firmware / simulator; requires `std`)
//!
//! ```rust,no_run
//! # #[cfg(feature = "std")]
//! # {
//! use crepuscularity_embedded::Ui;
//!
//! const UI: &str = "div w-full h-full\n  span #temp\n    \"{temp}\"";
//! let mut ui = Ui::new(240, 320, UI).with("temp", 72);
//! ui.render().unwrap();
//! let _bytes = ui.rgb565();
//! let _ = ui.hit(10, 10);
//! # }
//! ```
//!
//! Or with initial variables: `ui!(UI, 240, 320, "cpu" => 42, "status" => "ok")`.
//!
//! Docs: [`embedded.md`](https://github.com/tschk/crepuscularity/blob/main/docs/embedded.md) · Example: `examples/embedded-dashboard`.
//!
//! Lower-level: [`Template`], [`Rgb565View`].
//!
//! # Display RAM you already own
//!
//! ```rust,no_run
//! # use crepuscularity_embedded::{Rgb565View, ScreenSize};
//! # let screen = ScreenSize::new(128, 64);
//! # let mut ram = [0u16; 128 * 64];
//! let mut fb = Rgb565View::new(screen, &mut ram).unwrap();
//! // `Template` parses at runtime, so it needs `std`.
//! # #[cfg(feature = "std")]
//! # {
//! # let mut ui = crepuscularity_embedded::Template::from_source("div w-full h-full\n  \"Hi\"", screen);
//! ui.draw(&mut fb).unwrap();
//! # }
//! // `ram` is ready for DMA
//! ```
//!
//! # `build.rs` / CI
//!
//! Validate templates at compile time with [`crepuscularity_core::build::compile_crepus`],
//! or `crepus embedded check ui.crepus` in CI. Parsing and includes require the **`std`**
//! feature (enabled by default). With `default-features = false` you get `no_std` layout,
//! paint, and [`Framebuffer`] adapters only — use when a host build pre-renders or you
//! construct [`EmbeddedDocument`] trees in Rust.
//!
//! # Dev-only snapshots
//!
//! `crepus embedded snapshot … --out file.ppm` writes a PPM for visual debugging; production
//! firmware should use [`Ui::render`] / [`Ui::rgb565`].

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod color;
pub mod display;
pub mod document;
pub mod font;
pub(crate) mod font_data;
pub mod framebuffer;
pub mod layout;
pub mod paint;

pub mod panel;
pub mod screen;
pub mod style;
mod tailwind_apply;

#[cfg(feature = "std")]
pub mod include_expand;
#[cfg(feature = "std")]
pub mod render;
#[cfg(feature = "std")]
pub mod template;
#[cfg(feature = "std")]
pub mod ui;

#[cfg(all(test, feature = "std"))]
mod tests;

pub use color::{lookup_named_color, parse_hex, Color, Rgb565, Rgb888};
pub use document::{
    Align, EmbeddedDocument, EmbeddedNode, EmbeddedStyle, FlexDir, Justify, Rect, SizeHint,
    DEFAULT_BG, DEFAULT_TEXT,
};
pub use font::{draw_text, measure_text, FontMetrics};
pub use framebuffer::{Framebuffer, Rgb565Buffer, Rgb565View, Rgb888Buffer};
pub use layout::{layout_document, layout_tree};
pub use screen::ScreenSize;
pub use style::{parse_classes, style_from_classes};

#[cfg(feature = "std")]
pub use crepuscularity_core::{
    build, parse_component_file, parse_template, CrepusError, TemplateContext, TemplateValue,
};
#[cfg(feature = "macros")]
pub use crepuscularity_macros::embedded_template;
pub use display::{
    flush_framebuffer, swap_rgb565_bytes_bgr, DisplayError, PanelConfig, Rgb565ByteOrder,
    Rgb565Display,
};
pub use panel::preset::PanelPreset;
#[cfg(feature = "std")]
pub use render::{
    render_component_file_to_framebuffer, render_file_to_framebuffer, render_nodes_to_document,
    render_parsed_nodes_to_framebuffer, render_template_to_framebuffer, with_embedded_target,
};
#[cfg(feature = "std")]
pub use template::{template, Template};
#[cfg(feature = "std")]
pub use ui::Ui;

/// PPM (P6) export — delegates to [`Rgb888Buffer::write_ppm`].
#[cfg(feature = "std")]
pub fn write_ppm(path: &std::path::Path, buffer: &framebuffer::Rgb888Buffer) -> Result<(), String> {
    buffer.write_ppm(path)
}

/// Shorthand for [`Ui::new`] with optional `.set` chain.
///
/// ```rust
/// # use crepuscularity_embedded::ui;
/// let mut ui = ui!(r#"div w-full h-full"#, 128, 64, "name" => "Ada");
/// ```
#[cfg(feature = "std")]
#[macro_export]
macro_rules! ui {
    ($src:expr, $w:expr, $h:expr) => {
        $crate::Ui::new($w, $h, $src)
    };
    ($src:expr, $w:expr, $h:expr, $($key:expr => $val:expr),+ $(,)?) => {
        $crate::Ui::new($w, $h, $src)$(.with($key, $val))+
    };
}
