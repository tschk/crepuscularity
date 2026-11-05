#![cfg(all(feature = "std", feature = "mipidsi"))]

use std::{cell::RefCell, rc::Rc};

use crepuscularity_embedded::{
    flush_framebuffer, panel::mipidsi_blit, Color, DisplayError, Framebuffer, PanelConfig,
    Rgb565Buffer, Rgb565Display, ScreenSize,
};
use display_interface::{DataFormat, WriteOnlyDataCommand};
use mipidsi::{models::ST7789, Builder, Display, NoResetPin};

struct Interface(Rc<RefCell<Vec<u16>>>);
impl WriteOnlyDataCommand for Interface {
    fn send_commands(&mut self, _: DataFormat<'_>) -> Result<(), display_interface::DisplayError> {
        Ok(())
    }

    fn send_data(&mut self, data: DataFormat<'_>) -> Result<(), display_interface::DisplayError> {
        // Capture the logical RGB565 words the real mipidsi driver sends to its bus.
        // Initialization/address commands use byte formats and are intentionally ignored.
        if let DataFormat::U16BEIter(words) = data {
            self.0.borrow_mut().extend(words);
        }
        Ok(())
    }
}

struct Delay;
impl embedded_hal::delay::DelayNs for Delay {
    fn delay_ns(&mut self, _: u32) {}
}

struct Panel(Display<Interface, ST7789, NoResetPin>);
impl Rgb565Display for Panel {
    fn screen_size(&self) -> ScreenSize {
        ScreenSize::new(3, 1)
    }

    fn uses_native_rgb565_bytes(&self) -> bool {
        true
    }

    fn flush_rgb565_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        pixels: &[u8],
    ) -> Result<(), DisplayError> {
        mipidsi_blit::blit_rect(&mut self.0, x, y, w, h, pixels)
    }
}

fn panel() -> (Panel, Rc<RefCell<Vec<u16>>>) {
    let words = Rc::new(RefCell::new(Vec::new()));
    let display = Builder::new(ST7789, Interface(words.clone()))
        .init(&mut Delay)
        .unwrap();
    words.borrow_mut().clear();
    (Panel(display), words)
}

#[test]
fn st7789_framebuffer_flush_preserves_primary_color_words() {
    for config in [PanelConfig::st7789_240x320(), PanelConfig::default()] {
        let (mut panel, words) = panel();
        let mut framebuffer = Rgb565Buffer::new(ScreenSize::new(3, 1), Color::rgb(255, 0, 0));
        framebuffer.set_pixel(1, 0, Color::rgb(0, 255, 0));
        framebuffer.set_pixel(2, 0, Color::rgb(0, 0, 255));
        flush_framebuffer(&mut panel, &framebuffer, config, &mut Vec::new()).unwrap();
        assert_eq!(*words.borrow(), [0xf800, 0x07e0, 0x001f]);
    }
}

#[test]
fn direct_mipidsi_blit_full_preserves_little_endian_input() {
    let (mut panel, words) = panel();
    mipidsi_blit::blit_full(&mut panel.0, 3, 1, &[0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00]).unwrap();
    assert_eq!(*words.borrow(), [0xf800, 0x07e0, 0x001f]);
}
