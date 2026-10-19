//! Crepuscularity compiled templates with GPUI Kit's GPUI types.
//!
//! This is a separate backend from `crepuscularity-gpui` (GPUI CE). It does not
//! re-export the CE runtime or convert entities, elements, windows, or contexts
//! between the two frameworks.
//!
//! The shared macros emit `::gpui` paths. Name this dependency `gpui` in the
//! consuming application's Cargo.toml, or declare
//! `extern crate crepuscularity_gpui_kit as gpui;` at its crate root.
//!
//! ```no_run
//! extern crate crepuscularity_gpui_kit as gpui;
//! use gpui::prelude::*;
//!
//! struct Hello;
//! impl Render for Hello {
//!     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
//!         view!(r#"
//! div flex flex-col gap-2 p-4
//!   "Hello from Crepuscularity and GPUI Kit"
//! "#)
//!     }
//! }
//!
//! fn main() {
//!     gpui::application().run(|cx| {
//!         gpui::init(cx);
//!         gpui::open_window(gpui::WindowOptions::default(), cx, |_, cx| {
//!             cx.new(|_| Hello)
//!         }).expect("open window");
//!     });
//! }
//! ```
//!
//! Construct native Kit controls in Rust and insert them with `{control}` in a
//! template. Lowercase `button`, `input`, and `img` tags do not become Kit
//! controls. Runtime rendering and hot reload are not provided by this adapter.

pub use crepuscularity_core::build;
pub use crepuscularity_macros::{view, view_file};
pub use gpui_kit as kit;
pub use gpui_kit::*;

/// GPUI traits, common types, Kit lifecycle entry points, and template macros.
pub mod prelude {
    pub use crate::{view, view_file};
    pub use gpui_kit::prelude::*;
    pub use gpui_kit::{
        div, init, open_window, px, rgb, App, AppContext, Context, Entity, IntoElement, Render,
        SharedString, Window, WindowOptions,
    };
}
