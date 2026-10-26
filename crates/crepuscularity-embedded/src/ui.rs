//! All-in-one UI: template + context + RGB565 framebuffer.

use std::path::Path;

#[cfg(feature = "std")]
use std::vec::Vec;

use crepuscularity_core::context::TemplateValue;

use crate::display::{
    flush_framebuffer, DisplayError, PanelConfig, Rgb565ByteOrder, Rgb565Display,
};
use crate::document::EmbeddedDocument;
use crate::framebuffer::{Framebuffer, Rgb565Buffer};
use crate::screen::ScreenSize;
use crate::template::Template;
use crate::DEFAULT_BG;

/// Owns a `.crepus` template, variable context, and RGB565 framebuffer.
pub struct Ui {
    template: Template,
    buffer: Rgb565Buffer,
    panel: PanelConfig,
    #[cfg(feature = "std")]
    panel_scratch: Vec<u8>,
}

impl Ui {
    pub fn new(width: u16, height: u16, source: &str) -> Self {
        let screen = ScreenSize::new(width, height);
        Self {
            template: Template::from_source(source, screen),
            buffer: Rgb565Buffer::new(screen, DEFAULT_BG),
            panel: PanelConfig::default(),
            #[cfg(feature = "std")]
            panel_scratch: Vec::new(),
        }
    }

    pub fn from_path(width: u16, height: u16, path: impl AsRef<Path>) -> Result<Self, String> {
        let screen = ScreenSize::new(width, height);
        Ok(Self {
            template: Template::from_path(path, screen)?,
            buffer: Rgb565Buffer::new(screen, DEFAULT_BG),
            panel: PanelConfig::default(),
            #[cfg(feature = "std")]
            panel_scratch: Vec::new(),
        })
    }

    pub fn panel(mut self, config: PanelConfig) -> Self {
        self.panel = config;
        self
    }

    pub fn screen(&self) -> ScreenSize {
        self.template.screen()
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<TemplateValue>) -> &mut Self {
        self.template.set(key, value);
        self
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<TemplateValue>) -> Self {
        self.template.set(key, value);
        self
    }

    pub fn set_component(&mut self, name: impl Into<String>) -> &mut Self {
        self.template.set_component(name);
        self
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.template.reload()
    }

    /// Layout and paint into the internal framebuffer (re-parses only when the template source changes).
    pub fn render(&mut self) -> Result<&EmbeddedDocument, String> {
        self.template.draw(&mut self.buffer)
    }

    /// RGB565 pixel bytes in **internal** byte order (before [`PanelConfig::byte_order`]).
    pub fn rgb565(&self) -> &[u8] {
        self.buffer
            .as_rgb565_bytes()
            .expect("Rgb565Buffer always exposes bytes")
    }

    /// RGB565 bytes after applying [`self.panel`](Self::panel) byte-order encoding.
    #[cfg(feature = "std")]
    pub fn rgb565_for_panel(&mut self) -> &[u8] {
        match self.panel.byte_order {
            Rgb565ByteOrder::Rgb => self.rgb565(),
            Rgb565ByteOrder::Bgr => {
                let src = self
                    .buffer
                    .as_rgb565_bytes()
                    .expect("Rgb565Buffer always exposes bytes");
                crate::display::swap_rgb565_bytes_bgr(src, &mut self.panel_scratch)
            }
        }
    }

    pub fn pixels(&self) -> &[u16] {
        self.buffer.pixels()
    }

    pub fn document(&self) -> Option<&EmbeddedDocument> {
        self.template.document()
    }

    pub fn hit(&self, x: u16, y: u16) -> Option<&str> {
        self.template.document()?.node_at(x, y)
    }

    pub fn template(&self) -> &Template {
        &self.template
    }

    pub fn template_mut(&mut self) -> &mut Template {
        &mut self.template
    }

    pub fn buffer(&self) -> &Rgb565Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Rgb565Buffer {
        &mut self.buffer
    }

    pub fn render_into(&mut self, fb: &mut impl Framebuffer) -> Result<&EmbeddedDocument, String> {
        self.template.draw(fb)
    }

    /// Render, then push pixels to a display driver (SPI ILI9341, ST7789, …).
    #[cfg(feature = "std")]
    pub fn flush<D: Rgb565Display + ?Sized>(
        &mut self,
        display: &mut D,
    ) -> Result<&EmbeddedDocument, DisplayError> {
        self.render().map_err(DisplayError::Message)?;
        flush_framebuffer(display, &self.buffer, self.panel, &mut self.panel_scratch)?;
        Ok(self.document().expect("render stores document"))
    }
}
