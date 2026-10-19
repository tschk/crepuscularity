//! Panel flush helpers for common RGB565 display drivers (SPI ILI9341, ST7789, LTDC, …).

use crate::framebuffer::Framebuffer;
use crate::screen::ScreenSize;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

/// How each 16-bit pixel is encoded on the wire (many controllers differ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rgb565ByteOrder {
    /// Standard RGB565: high byte = RRRRRGGG, low byte = GGGGBBBB.
    #[default]
    Rgb,
    /// Swap the two bytes of each pixel (required on some ST7789/ILI9341 modules).
    Bgr,
}

/// Window offset and byte order for a physical panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PanelConfig {
    pub byte_order: Rgb565ByteOrder,
    /// Column offset applied by some 240×320 modules (visible area starts at (x_offset, y_offset)).
    pub x_offset: u16,
    pub y_offset: u16,
}

impl PanelConfig {
    pub const fn ili9341_240x320() -> Self {
        Self {
            byte_order: Rgb565ByteOrder::Rgb,
            x_offset: 0,
            y_offset: 0,
        }
    }

    pub const fn st7789_240x320() -> Self {
        Self {
            byte_order: Rgb565ByteOrder::Bgr,
            x_offset: 0,
            y_offset: 0,
        }
    }
}

/// Flush a row-major RGB565 buffer to a display (SPI burst, `esp_lcd`, LTDC helper, etc.).
pub trait Rgb565Display {
    fn screen_size(&self) -> ScreenSize;

    /// Receive native framebuffer RGB565 bytes instead of panel-encoded wire bytes.
    ///
    /// Semantic display adapters can opt in when they decode pixel values and
    /// handle transport encoding themselves. [`flush_framebuffer`] then skips
    /// [`PanelConfig::byte_order`] conversion; panel offsets still apply.
    /// Raw wire drivers retain panel byte-order conversion by default.
    fn uses_native_rgb565_bytes(&self) -> bool {
        false
    }

    /// Draw `pixels` into the rectangle `(x, y) .. (x + w, y + h)`.
    fn flush_rgb565_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        pixels: &[u8],
    ) -> Result<(), DisplayError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayError {
    Message(String),
}

impl core::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DisplayError::Message(m) => write!(f, "{m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DisplayError {}

/// Swap each RGB565 pixel's two bytes into `scratch` (ST7789 / some ILI9341 modules).
pub fn swap_rgb565_bytes_bgr<'a>(src: &[u8], scratch: &'a mut Vec<u8>) -> &'a [u8] {
    scratch.clear();
    scratch.reserve(src.len());
    let mut i = 0;
    while i + 1 < src.len() {
        scratch.push(src[i + 1]);
        scratch.push(src[i]);
        i += 2;
    }
    scratch.as_slice()
}

/// Write every pixel from `fb` into `display` using `panel` offsets and byte order.
pub fn flush_framebuffer<D: Rgb565Display + ?Sized>(
    display: &mut D,
    fb: &impl Framebuffer,
    panel: PanelConfig,
    scratch: &mut Vec<u8>,
) -> Result<(), DisplayError> {
    let size = fb.screen_size();
    let raw = fb
        .as_rgb565_bytes()
        .ok_or_else(|| DisplayError::Message("framebuffer is not RGB565".into()))?;
    if size.pixel_count().checked_mul(2) != Some(raw.len()) {
        return Err(DisplayError::Message(
            "RGB565 byte length does not match framebuffer dimensions".into(),
        ));
    }
    let bytes = if display.uses_native_rgb565_bytes() {
        raw
    } else {
        match panel.byte_order {
            Rgb565ByteOrder::Rgb => raw,
            Rgb565ByteOrder::Bgr => swap_rgb565_bytes_bgr(raw, scratch),
        }
    };
    display.flush_rgb565_rect(
        panel.x_offset,
        panel.y_offset,
        size.width,
        size.height,
        bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    struct InvalidFramebuffer<'a>(&'a [u8]);
    impl Framebuffer for InvalidFramebuffer<'_> {
        fn width(&self) -> u16 {
            2
        }
        fn height(&self) -> u16 {
            1
        }
        fn set_pixel(&mut self, _: u16, _: u16, _: Color) {}
        fn fill(&mut self, _: Color) {}
        fn as_rgb565_bytes(&self) -> Option<&[u8]> {
            Some(self.0)
        }
    }
    struct Display;
    impl Rgb565Display for Display {
        fn screen_size(&self) -> ScreenSize {
            ScreenSize::new(2, 1)
        }
        fn flush_rgb565_rect(
            &mut self,
            _: u16,
            _: u16,
            _: u16,
            _: u16,
            _: &[u8],
        ) -> Result<(), DisplayError> {
            panic!("invalid framebuffer must not reach the display");
        }
    }
    #[test]
    fn invalid_framebuffer_length_is_rejected_before_display_flush() {
        for len in [0, 1, 2, 3, 5] {
            for byte_order in [Rgb565ByteOrder::Rgb, Rgb565ByteOrder::Bgr] {
                let mut scratch = Vec::from([42]);
                assert!(flush_framebuffer(
                    &mut Display,
                    &InvalidFramebuffer(&[0; 5][..len]),
                    PanelConfig {
                        byte_order,
                        ..PanelConfig::default()
                    },
                    &mut scratch
                )
                .is_err());
                assert_eq!(scratch, [42]);
            }
        }
    }
    #[derive(Default)]
    struct Capture {
        rect: Option<(u16, u16, u16, u16)>,
        bytes: Vec<u8>,
    }

    struct NativeDisplay(Capture);
    struct RawDisplay(Capture);

    impl Rgb565Display for NativeDisplay {
        fn uses_native_rgb565_bytes(&self) -> bool {
            true
        }
        fn screen_size(&self) -> ScreenSize {
            ScreenSize::new(16, 16)
        }
        fn flush_rgb565_rect(
            &mut self,
            x: u16,
            y: u16,
            w: u16,
            h: u16,
            pixels: &[u8],
        ) -> Result<(), DisplayError> {
            self.0.rect = Some((x, y, w, h));
            self.0.bytes.extend_from_slice(pixels);
            Ok(())
        }
    }

    impl Rgb565Display for RawDisplay {
        fn screen_size(&self) -> ScreenSize {
            ScreenSize::new(16, 16)
        }
        fn flush_rgb565_rect(
            &mut self,
            x: u16,
            y: u16,
            w: u16,
            h: u16,
            pixels: &[u8],
        ) -> Result<(), DisplayError> {
            self.0.rect = Some((x, y, w, h));
            self.0.bytes.extend_from_slice(pixels);
            Ok(())
        }
    }

    fn primary_buffer() -> crate::Rgb565Buffer {
        let mut fb = crate::Rgb565Buffer::new(ScreenSize::new(3, 1), Color::rgb(0, 0, 0));
        for (x, color) in [
            Color::rgb(255, 0, 0),
            Color::rgb(0, 255, 0),
            Color::rgb(0, 0, 255),
        ]
        .into_iter()
        .enumerate()
        {
            fb.set_pixel(x as u16, 0, color);
        }
        fb
    }

    #[test]
    fn native_display_preserves_primary_colors_and_offsets_without_encoding_scratch() {
        let fb = primary_buffer();
        for byte_order in [Rgb565ByteOrder::Rgb, Rgb565ByteOrder::Bgr] {
            let mut display = NativeDisplay(Capture::default());
            let mut scratch = Vec::from([41, 42, 43]);
            flush_framebuffer(
                &mut display,
                &fb,
                PanelConfig {
                    byte_order,
                    x_offset: 4,
                    y_offset: 7,
                },
                &mut scratch,
            )
            .unwrap();
            assert_eq!(display.0.rect, Some((4, 7, 3, 1)));
            assert_eq!(
                display.0.bytes,
                fb.as_rgb565_bytes().unwrap(),
                "{byte_order:?}"
            );
            let decoded: Vec<_> = display
                .0
                .bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            assert_eq!(decoded, [0xf800, 0x07e0, 0x001f], "{byte_order:?}");
            assert_eq!(
                scratch,
                [41, 42, 43],
                "native driver must not need encoding scratch"
            );
        }
    }

    #[test]
    fn raw_display_keeps_existing_panel_byte_order_and_offsets() {
        let fb = primary_buffer();
        let raw = fb.as_rgb565_bytes().unwrap();
        for byte_order in [Rgb565ByteOrder::Rgb, Rgb565ByteOrder::Bgr] {
            let mut display = RawDisplay(Capture::default());
            let mut scratch = Vec::from([41, 42, 43]);
            flush_framebuffer(
                &mut display,
                &fb,
                PanelConfig {
                    byte_order,
                    x_offset: 4,
                    y_offset: 7,
                },
                &mut scratch,
            )
            .unwrap();
            let expected: Vec<_> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|pair| match byte_order {
                    Rgb565ByteOrder::Rgb => [pair[0], pair[1]],
                    Rgb565ByteOrder::Bgr => [pair[1], pair[0]],
                })
                .collect();
            assert_eq!(display.0.rect, Some((4, 7, 3, 1)));
            assert_eq!(display.0.bytes, expected);
            match byte_order {
                Rgb565ByteOrder::Rgb => assert_eq!(scratch, [41, 42, 43]),
                Rgb565ByteOrder::Bgr => assert_eq!(scratch, expected),
            }
        }
    }
}
