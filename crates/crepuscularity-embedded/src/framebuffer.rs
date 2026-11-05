//! Row-major framebuffer buffers and the [`Framebuffer`] draw trait.

use crate::color::{Color, Rgb888};
use crate::screen::ScreenSize;

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

pub trait Framebuffer {
    fn width(&self) -> u16;
    fn height(&self) -> u16;
    fn screen_size(&self) -> ScreenSize {
        ScreenSize::new(self.width(), self.height())
    }
    fn set_pixel(&mut self, x: u16, y: u16, color: Color);
    fn fill(&mut self, color: Color);
    fn as_rgb565_bytes(&self) -> Option<&[u8]> {
        None
    }
}

/// Mutable view over a display's RGB565 framebuffer (row-major `width * height` pixels).
pub struct Rgb565View<'a> {
    size: ScreenSize,
    pixels: &'a mut [u16],
}

pub struct Rgb565Buffer {
    size: ScreenSize,
    pixels: Vec<u16>,
}

pub struct Rgb888Buffer {
    size: ScreenSize,
    pixels: Vec<Rgb888>,
}

impl<'a> Rgb565View<'a> {
    pub fn new(size: ScreenSize, pixels: &'a mut [u16]) -> Result<Self, String> {
        if pixels.len() != size.pixel_count() {
            return Err(format!(
                "framebuffer length {} does not match {}x{} ({} pixels)",
                pixels.len(),
                size.width,
                size.height,
                size.pixel_count()
            ));
        }
        Ok(Self { size, pixels })
    }

    pub fn screen_size(&self) -> ScreenSize {
        self.size
    }

    pub fn pixels_mut(&mut self) -> &mut [u16] {
        self.pixels
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.pixels.as_ptr().cast(), self.pixels.len() * 2) }
    }
}

impl Rgb565Buffer {
    pub fn screen_size(&self) -> ScreenSize {
        self.size
    }

    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u16] {
        &mut self.pixels
    }

    pub fn new(size: ScreenSize, fill: Color) -> Self {
        let v = fill.to_rgb565().0;
        let mut pixels = Vec::with_capacity(size.pixel_count());
        pixels.resize(size.pixel_count(), v);
        Self { size, pixels }
    }

    pub fn clear(&mut self, color: Color) {
        let v = color.to_rgb565().0;
        self.pixels.fill(v);
    }
}

impl Rgb888Buffer {
    /// Return packed RGB bytes in red, green, blue channel order.
    pub fn as_rgb888_bytes(&self) -> &[u8] {
        // SAFETY: repr(C) Rgb888 contains three initialized u8 fields with no padding.
        unsafe { core::slice::from_raw_parts(self.pixels.as_ptr().cast(), self.pixels.len() * 3) }
    }

    pub fn new(size: ScreenSize, fill: Color) -> Self {
        let v = fill.to_rgb888();
        let mut pixels = Vec::with_capacity(size.pixel_count());
        pixels.resize(size.pixel_count(), v);
        Self { size, pixels }
    }

    pub fn clear(&mut self, color: Color) {
        let v = color.to_rgb888();
        self.pixels.fill(v);
    }
}

impl Framebuffer for Rgb565View<'_> {
    fn width(&self) -> u16 {
        self.size.width
    }
    fn height(&self) -> u16 {
        self.size.height
    }
    fn set_pixel(&mut self, x: u16, y: u16, color: Color) {
        if x < self.size.width && y < self.size.height {
            self.pixels[y as usize * self.size.width as usize + x as usize] = color.to_rgb565().0;
        }
    }
    fn fill(&mut self, color: Color) {
        let v = color.to_rgb565().0;
        self.pixels.fill(v);
    }
    fn as_rgb565_bytes(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
}

impl Framebuffer for Rgb565Buffer {
    fn width(&self) -> u16 {
        self.size.width
    }
    fn height(&self) -> u16 {
        self.size.height
    }
    fn set_pixel(&mut self, x: u16, y: u16, color: Color) {
        if x < self.size.width && y < self.size.height {
            self.pixels[y as usize * self.size.width as usize + x as usize] = color.to_rgb565().0;
        }
    }
    fn fill(&mut self, color: Color) {
        self.clear(color);
    }
    fn as_rgb565_bytes(&self) -> Option<&[u8]> {
        Some(unsafe {
            core::slice::from_raw_parts(self.pixels.as_ptr().cast(), self.pixels.len() * 2)
        })
    }
}

#[cfg(feature = "std")]
impl Rgb888Buffer {
    pub fn write_ppm(&self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        let mut f =
            std::fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
        write!(f, "P6\n{} {}\n255\n", self.size.width, self.size.height)
            .map_err(|e| format!("write: {e}"))?;
        f.write_all(self.as_rgb888_bytes())
            .map_err(|e| format!("write: {e}"))
    }
}

impl Framebuffer for Rgb888Buffer {
    fn width(&self) -> u16 {
        self.size.width
    }
    fn height(&self) -> u16 {
        self.size.height
    }
    fn set_pixel(&mut self, x: u16, y: u16, color: Color) {
        if x < self.size.width && y < self.size.height {
            self.pixels[y as usize * self.size.width as usize + x as usize] = color.to_rgb888();
        }
    }
    fn fill(&mut self, color: Color) {
        self.clear(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_views_validate_lengths_and_clip_pixel_writes() {
        let size = ScreenSize::new(2, 2);
        for len in [0, 1, 3, 5] {
            let mut pixels = [0u16; 5];
            assert!(Rgb565View::new(size, &mut pixels[..len]).is_err());
        }
        let mut pixels = [0u16; 4];
        let mut view = Rgb565View::new(size, &mut pixels).unwrap();
        let mut owned = Rgb565Buffer::new(size, Color::rgb(0, 0, 0));
        let mut rgb888 = Rgb888Buffer::new(size, Color::rgb(0, 0, 0));
        let red = Color::rgb(255, 0, 0);
        for fb in [&mut view as &mut dyn Framebuffer, &mut owned, &mut rgb888] {
            fb.set_pixel(1, 1, red);
            for (x, y) in [(2, 0), (0, 2), (u16::MAX, u16::MAX)] {
                fb.set_pixel(x, y, Color::rgb(255, 255, 255));
            }
        }
        assert_eq!(view.pixels_mut(), &[0, 0, 0, red.to_rgb565().0]);
        assert_eq!(owned.pixels(), &[0, 0, 0, red.to_rgb565().0]);
        assert_eq!(view.as_bytes(), owned.as_rgb565_bytes().unwrap());
        assert_eq!(
            rgb888.as_rgb888_bytes(),
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0]
        );

        for fb in [&mut view as &mut dyn Framebuffer, &mut owned, &mut rgb888] {
            fb.fill(Color::rgb(0, 0, 0));
        }
        assert_eq!(view.pixels_mut(), &[0; 4]);
        assert_eq!(owned.pixels(), &[0; 4]);
        assert_eq!(rgb888.as_rgb888_bytes(), &[0; 12]);

        let mut empty = [];
        let mut view = Rgb565View::new(ScreenSize::new(0, 0), &mut empty).unwrap();
        view.fill(red);
        view.set_pixel(0, 0, red);
        assert!(view.as_bytes().is_empty());
    }

    #[test]
    fn rgb888_byte_export_preserves_channel_and_pixel_order() {
        assert_eq!(core::mem::size_of::<Rgb888>(), 3);
        assert_eq!(core::mem::align_of::<Rgb888>(), 1);
        let mut fb = Rgb888Buffer::new(ScreenSize::new(2, 1), Color::rgb(1, 2, 3));
        fb.set_pixel(1, 0, Color::rgb(4, 5, 6));
        assert_eq!(fb.as_rgb888_bytes(), &[1, 2, 3, 4, 5, 6]);
    }
}
