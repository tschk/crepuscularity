//! Bitmap font metrics, glyph lookup, and framebuffer text drawing.

use crate::color::Color;
use crate::document::Rect;
use crate::framebuffer::Framebuffer;

/// Cell metrics for the software renderer's fixed 8×8 bitmap font.
/// Smaller cells clip glyphs; larger cells add spacing without scaling them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    pub cell_width: u8,
    pub cell_height: u8,
    pub baseline: u8,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self::builtin_8x8()
    }
}

impl FontMetrics {
    pub const fn builtin_8x8() -> Self {
        Self {
            cell_width: 8,
            cell_height: 8,
            baseline: 7,
        }
    }

    pub fn line_height(&self) -> u16 {
        self.cell_height as u16
    }

    pub fn measure_text(&self, text: &str) -> (u16, u16) {
        let lines = if text.is_empty() {
            1
        } else {
            text.lines().count()
        };
        let max_cols = text
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        (
            (max_cols.min(u16::MAX as usize) as u16).saturating_mul(self.cell_width as u16),
            (lines.min(u16::MAX as usize) as u16).saturating_mul(self.line_height()),
        )
    }

    pub fn glyph_rows(&self, ch: char) -> [u8; 8] {
        let _ = self;
        glyph_bitmap(ch)
    }

    pub fn is_supported(&self, ch: char) -> bool {
        let _ = self;
        (32..127).contains(&(ch as u32))
    }
}

pub fn measure_text(text: &str) -> (u16, u16) {
    FontMetrics::default().measure_text(text)
}

pub fn char_width() -> u16 {
    FontMetrics::default().cell_width as u16
}

pub fn char_height() -> u16 {
    FontMetrics::default().cell_height as u16
}

pub fn draw_text(
    fb: &mut impl Framebuffer,
    bounds: Rect,
    text: &str,
    color: Color,
    metrics: &FontMetrics,
) {
    if metrics.cell_width == 0 || metrics.cell_height == 0 || bounds.w == 0 || bounds.h == 0 {
        return;
    }
    let mut y = bounds.y;
    for line in text.lines() {
        let remaining_height = bounds.y.saturating_add(bounds.h).saturating_sub(y);
        if remaining_height == 0 {
            break;
        }
        let mut x = bounds.x;
        for ch in line.chars() {
            if x.saturating_add(metrics.cell_width as u16) > bounds.x.saturating_add(bounds.w) {
                break;
            }
            if metrics.is_supported(ch) {
                let mut rows = metrics.glyph_rows(ch);
                for row in rows
                    .iter_mut()
                    .skip(remaining_height.min(metrics.line_height()) as usize)
                {
                    *row = 0;
                }
                draw_glyph(fb, x, y, rows, color, metrics.cell_width as u16);
            } else {
                draw_replacement_box(
                    fb,
                    x,
                    y,
                    metrics.cell_width as u16,
                    (metrics.cell_height as u16).min(remaining_height),
                    color,
                );
            }
            x = x.saturating_add(metrics.cell_width as u16);
        }
        y = y.saturating_add(metrics.line_height());
        if y >= bounds.y.saturating_add(bounds.h) {
            break;
        }
    }
}

fn draw_glyph(
    fb: &mut impl Framebuffer,
    x: u16,
    y: u16,
    rows: [u8; 8],
    color: Color,
    cell_width: u16,
) {
    for (ri, row) in rows
        .iter()
        .take(fb.height().saturating_sub(y) as usize)
        .enumerate()
    {
        for col in 0..8u16.min(cell_width).min(fb.width().saturating_sub(x)) {
            if row & (1 << (7 - col)) != 0 {
                fb.set_pixel(x + col, y + ri as u16, color);
            }
        }
    }
}

fn draw_replacement_box(fb: &mut impl Framebuffer, x: u16, y: u16, w: u16, h: u16, color: Color) {
    for dy in 0..h.min(fb.height().saturating_sub(y)) {
        for dx in 0..w.min(fb.width().saturating_sub(x)) {
            fb.set_pixel(x + dx, y + dy, color);
        }
    }
}

fn glyph_bitmap(ch: char) -> [u8; 8] {
    let idx = ch as u32;
    if (32..127).contains(&idx) {
        GLYPHS[(idx - 32) as usize]
    } else {
        [0; 8]
    }
}

use crate::font_data::GLYPHS;

#[cfg(test)]
mod tests {
    #[test]
    fn asymmetric_glyphs_preserve_left_to_right_bitmap_order() {
        use crate::{Rgb565Buffer, ScreenSize};
        for (ch, rows) in [
            ("K", [0x66u8, 0x6c, 0x78, 0x70, 0x78, 0x6c, 0x66, 0]),
            ("L", [0x60u8, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7e, 0]),
        ] {
            let mut fb = Rgb565Buffer::new(ScreenSize::new(8, 8), Color::rgb(0, 0, 0));
            draw_text(
                &mut fb,
                Rect::new(0, 0, 8, 8),
                ch,
                Color::rgb(255, 255, 255),
                &FontMetrics::default(),
            );
            for (actual, bits) in fb.pixels().chunks(8).zip(rows) {
                let expected: [u16; 8] =
                    core::array::from_fn(|x| if bits & (0x80 >> x) != 0 { u16::MAX } else { 0 });
                assert_eq!(actual, expected, "glyph {ch}");
            }
        }
    }

    #[test]
    fn measured_text_bounds_fit_complete_glyphs() {
        use crate::{framebuffer::Rgb565Buffer, screen::ScreenSize};
        let metrics = FontMetrics::default();
        for text in ["OK", "A\nB", "界A"] {
            let mut measured = Rgb565Buffer::new(ScreenSize::new(32, 24), Color::rgb(0, 0, 0));
            let mut roomy = Rgb565Buffer::new(ScreenSize::new(32, 24), Color::rgb(0, 0, 0));
            let (w, h) = metrics.measure_text(text);
            let white = Color::rgb(255, 255, 255);
            draw_text(&mut measured, Rect::new(0, 0, w, h), text, white, &metrics);
            draw_text(&mut roomy, Rect::new(0, 0, 32, 24), text, white, &metrics);
            assert_eq!(measured.pixels(), roomy.pixels(), "{text}");
            assert!(measured.pixels().iter().any(|pixel| *pixel != 0));
        }
    }

    use super::*;

    #[test]
    fn text_does_not_paint_below_its_bounds() {
        use crate::{Rgb565Buffer, ScreenSize};
        for text in ["A", "界", "A\n界"] {
            for height in [0, 1, 2, 5, 11] {
                let mut fb = Rgb565Buffer::new(ScreenSize::new(20, 20), Color::rgb(0, 0, 0));
                draw_text(
                    &mut fb,
                    Rect::new(0, 0, 20, height),
                    text,
                    Color::rgb(255, 255, 255),
                    &FontMetrics::default(),
                );
                assert!(fb.pixels()[height as usize * 20..]
                    .iter()
                    .all(|pixel| *pixel == 0));
            }
        }
    }

    #[test]
    fn offscreen_glyph_coordinates_do_not_wrap() {
        use crate::framebuffer::Rgb565Buffer;
        use crate::screen::ScreenSize;
        let mut fb = Rgb565Buffer::new(ScreenSize::new(8, 8), Color::rgb(0, 0, 0));
        for text in ["A", "界"] {
            draw_text(
                &mut fb,
                Rect::new(u16::MAX - 5, u16::MAX - 5, 20, 20),
                text,
                Color::rgb(255, 255, 255),
                &FontMetrics::default(),
            );
        }
        assert!(fb.pixels().iter().all(|pixel| *pixel == 0));
        let mut edge = Rgb565Buffer::new(ScreenSize::new(u16::MAX, 8), Color::rgb(0, 0, 0));
        draw_replacement_box(&mut edge, u16::MAX - 3, 0, 8, 8, Color::rgb(255, 255, 255));
        for row in edge.pixels().chunks(u16::MAX as usize) {
            assert!(row[..row.len() - 3].iter().all(|pixel| *pixel == 0));
            assert!(row[row.len() - 3..].iter().all(|pixel| *pixel == u16::MAX));
        }
    }

    #[test]
    fn text_measurement_saturates_before_count_conversion() {
        let metrics = FontMetrics::default();
        assert_eq!(metrics.measure_text(&"a".repeat(65536)), (u16::MAX, 8));
        assert_eq!(metrics.measure_text(&"a\n".repeat(65536)), (8, u16::MAX));
    }

    #[test]
    fn unicode_cannot_alias_an_ascii_glyph() {
        let metrics = FontMetrics::default();
        assert!(metrics.is_supported('A'));
        assert!(!metrics.is_supported('Ł'));
        assert_eq!(metrics.glyph_rows('Ł'), [0; 8]);
    }
}
