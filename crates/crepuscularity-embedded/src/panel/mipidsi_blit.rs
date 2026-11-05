//! Blit row-major RGB565 bytes into a **mipidsi** display (ILI9341, ST7789, …).

use alloc::format;
use core::convert::Infallible;

use crate::DisplayError;
use display_interface::WriteOnlyDataCommand;
use embedded_graphics::image::{Image, ImageRawLE};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use mipidsi::models::Model;
use mipidsi::Display;

/// Blit a full-screen RGB565 buffer (`width` × `height`, 2 bytes per pixel).
pub fn blit_full<DI, M, RST>(
    display: &mut Display<DI, M, RST>,
    width: u16,
    height: u16,
    bytes: &[u8],
) -> Result<(), DisplayError>
where
    DI: WriteOnlyDataCommand,
    M: Model<ColorFormat = Rgb565>,
    RST: embedded_hal::digital::OutputPin<Error = Infallible>,
{
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| {
            DisplayError::Message("blit_full: dimensions exceed addressable memory".into())
        })?;
    if bytes.len() != expected {
        return Err(DisplayError::Message(format!(
            "blit_full: expected {expected} bytes, got {}",
            bytes.len()
        )));
    }
    blit_rect(display, 0, 0, width, height, bytes)
}

/// Blit RGB565 bytes into rectangle `(x, y)` size `w` × `h`.
pub fn blit_rect<DI, M, RST>(
    display: &mut Display<DI, M, RST>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    pixels: &[u8],
) -> Result<(), DisplayError>
where
    DI: WriteOnlyDataCommand,
    M: Model<ColorFormat = Rgb565>,
    RST: embedded_hal::digital::OutputPin<Error = Infallible>,
{
    let expected = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| {
            DisplayError::Message("blit_rect: dimensions exceed addressable memory".into())
        })?;
    if pixels.len() != expected {
        return Err(DisplayError::Message(format!(
            "blit_rect: expected {expected} bytes, got {}",
            pixels.len()
        )));
    }
    let raw = ImageRawLE::<Rgb565>::new(pixels, w as u32);
    Image::new(&raw, Point::new(x as i32, y as i32))
        .draw(display)
        .map_err(|e| DisplayError::Message(format!("mipidsi draw: {e:?}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Interface;
    impl WriteOnlyDataCommand for Interface {
        fn send_commands(
            &mut self,
            _: display_interface::DataFormat<'_>,
        ) -> Result<(), display_interface::DisplayError> {
            Ok(())
        }
        fn send_data(
            &mut self,
            _: display_interface::DataFormat<'_>,
        ) -> Result<(), display_interface::DisplayError> {
            Ok(())
        }
    }
    struct Delay;
    impl embedded_hal::delay::DelayNs for Delay {
        fn delay_ns(&mut self, _: u32) {}
    }

    #[test]
    fn blits_reject_invalid_lengths_and_large_dimensions() {
        let mut display = mipidsi::Builder::new(mipidsi::models::ILI9341Rgb565, Interface)
            .init(&mut Delay)
            .unwrap();
        for (w, h, bytes) in [
            (2, 1, &[][..]),
            (1, 1, &[0][..]),
            (u16::MAX, u16::MAX, &[][..]),
        ] {
            assert!(blit_full(&mut display, w, h, bytes).is_err());
            assert!(blit_rect(&mut display, 0, 0, w, h, bytes).is_err());
        }
    }
}
