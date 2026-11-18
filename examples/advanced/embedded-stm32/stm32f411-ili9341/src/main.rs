#![no_std]
#![no_main]

use crepuscularity_embedded::compiled::{CompiledTemplate, Value};
use crepuscularity_embedded::paint::paint_document;
use crepuscularity_embedded::panel::embassy_stm32::{ili9341_240x320_static, Ili9341Spi240x320};
use crepuscularity_embedded::{
    embedded_document, layout_document, Color, Framebuffer, Rgb565Display, ScreenSize,
};
use embedded_alloc::TlsfHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

/// Parsed on the build host; the chip only evaluates, lays out, and paints.
static UI: &CompiledTemplate = embedded_document!("../ui.crepus");

const SCREEN: ScreenSize = ScreenSize::new(240, 320);
const BAND_ROWS: u16 = 32;

/// One horizontal strip of the screen. A full 240×320 RGB565 frame (150 KiB) does not fit
/// in the F411's 128 KiB of RAM, so each frame is painted and blitted one band at a time.
struct Band<'a> {
    top: u16,
    pixels: &'a mut [u16],
}

impl Framebuffer for Band<'_> {
    fn width(&self) -> u16 {
        SCREEN.width
    }
    fn height(&self) -> u16 {
        SCREEN.height
    }
    fn set_pixel(&mut self, x: u16, y: u16, color: Color) {
        if x < SCREEN.width && y >= self.top && y < self.top + BAND_ROWS {
            let row = (y - self.top) as usize;
            self.pixels[row * SCREEN.width as usize + x as usize] = color.to_rgb565().0;
        }
    }
    fn fill(&mut self, color: Color) {
        self.pixels.fill(color.to_rgb565().0);
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    unsafe {
        embedded_alloc::init!(HEAP, 32 * 1024);
    }

    let p = embassy_stm32::init(Default::default());
    let _led = Output::new(p.PC13, Level::Low, Speed::Low);

    let panel = ili9341_240x320_static(p.SPI1, p.PA5, p.PA7, p.PA6, p.PA4, p.PA3, p.PA2);
    info!(
        "ili9341 ready {}x{}",
        Ili9341Spi240x320::WIDTH,
        Ili9341Spi240x320::HEIGHT
    );

    let mut pixels = [0u16; SCREEN.width as usize * BAND_ROWS as usize];
    let mut cpu: i64 = 0;
    loop {
        let status = if cpu > 80 { "busy" } else { "ok" };
        let vars = [("cpu", Value::Int(cpu)), ("status", Value::Str(status))];
        let mut doc = UI.document(&vars, SCREEN);
        layout_document(&mut doc);
        let mut top = 0;
        while top < SCREEN.height {
            let mut band = Band {
                top,
                pixels: &mut pixels,
            };
            paint_document(&mut band, &doc);
            let bytes = unsafe {
                core::slice::from_raw_parts(pixels.as_ptr().cast::<u8>(), pixels.len() * 2)
            };
            panel
                .flush_rgb565_rect(0, top, SCREEN.width, BAND_ROWS, bytes)
                .expect("blit");
            top += BAND_ROWS;
        }
        info!("frame rendered cpu={}", cpu);
        cpu = (cpu + 7) % 100;
        Timer::after(Duration::from_secs(1)).await;
    }
}
