//! Embassy **blocking SPI** helpers for STM32 + ILI9341 / ST7789 (mipidsi).

use core::cell::RefCell;

use display_interface_spi::SPIInterface;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::spi::{mode::Master, Config, Spi};
use embassy_stm32::Peri;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::RgbColor;
use mipidsi::models::{ILI9341Rgb565, ST7789};
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use mipidsi::Builder;
use static_cell::StaticCell;

use crate::panel::mipidsi_blit;
use crate::panel::preset::PanelPreset;
use crate::{DisplayError, PanelConfig, Rgb565Display, ScreenSize};

type SpiBus = Spi<'static, embassy_stm32::mode::Blocking, Master>;
type SpiBusMutex = Mutex<CriticalSectionRawMutex, RefCell<SpiBus>>;
type SpiDev = SpiDevice<'static, CriticalSectionRawMutex, SpiBus, Output<'static>>;
type Interface = SPIInterface<SpiDev, Output<'static>>;

/// ILI9341 240×320 over SPI (RGB565).
pub struct Ili9341Spi240x320 {
    display: mipidsi::Display<Interface, ILI9341Rgb565, Output<'static>>,
    size: ScreenSize,
    config: PanelConfig,
}

/// ST7789 240×320 over SPI (RGB565 with controller BGR channel order).
pub struct St7789Spi240x320 {
    display: mipidsi::Display<Interface, ST7789, Output<'static>>,
    size: ScreenSize,
    config: PanelConfig,
}

static SPI_BUS_CELL: StaticCell<SpiBusMutex> = StaticCell::new();
static ILI9341_CELL: StaticCell<Ili9341Spi240x320> = StaticCell::new();
static ST7789_CELL: StaticCell<St7789Spi240x320> = StaticCell::new();

/// ILI9341 on SPI1 (PA5 SCK, PA7 MOSI, PA6 MISO, PA4 CS, PA3 DC, PA2 RST). Returns `'static` panel.
///
/// Only one of [`ili9341_240x320_static`] / [`st7789_240x320_static`] may be used per binary (shared SPI bus cell).
pub fn ili9341_240x320_static(
    spi: Peri<'static, embassy_stm32::peripherals::SPI1>,
    sck: Peri<'static, embassy_stm32::peripherals::PA5>,
    mosi: Peri<'static, embassy_stm32::peripherals::PA7>,
    miso: Peri<'static, embassy_stm32::peripherals::PA6>,
    cs: Peri<'static, embassy_stm32::peripherals::PA4>,
    dc: Peri<'static, embassy_stm32::peripherals::PA3>,
    rst: Peri<'static, embassy_stm32::peripherals::PA2>,
) -> &'static mut Ili9341Spi240x320 {
    let preset = PanelPreset::Ili9341_240x320;
    let spi_bus = Spi::new_blocking(spi, sck, mosi, miso, Config::default());
    let spi_bus = SPI_BUS_CELL.init(Mutex::new(RefCell::new(spi_bus)));
    let cs_out = Output::new(cs, Level::High, Speed::High);
    let dc_out = Output::new(dc, Level::Low, Speed::High);
    let rst_out = Output::new(rst, Level::High, Speed::High);
    let spi_device = SpiDevice::new(spi_bus, cs_out);
    let interface = SPIInterface::new(spi_device, dc_out);
    let mut delay = embassy_time::Delay;
    let mut display = Builder::new(ILI9341Rgb565, interface)
        .reset_pin(rst_out)
        .color_order(ColorOrder::Rgb)
        .orientation(Orientation::new().rotate(Rotation::Deg0))
        .init(&mut delay)
        .expect("ili9341 init");
    let _ = display.clear(Rgb565::BLACK);
    ILI9341_CELL.init(Ili9341Spi240x320 {
        display,
        size: preset.size(),
        config: preset.config(),
    })
}

/// ST7789 on the same default SPI1 pinout as [`ili9341_240x320_static`].
pub fn st7789_240x320_static(
    spi: Peri<'static, embassy_stm32::peripherals::SPI1>,
    sck: Peri<'static, embassy_stm32::peripherals::PA5>,
    mosi: Peri<'static, embassy_stm32::peripherals::PA7>,
    miso: Peri<'static, embassy_stm32::peripherals::PA6>,
    cs: Peri<'static, embassy_stm32::peripherals::PA4>,
    dc: Peri<'static, embassy_stm32::peripherals::PA3>,
    rst: Peri<'static, embassy_stm32::peripherals::PA2>,
) -> &'static mut St7789Spi240x320 {
    let preset = PanelPreset::St7789_240x320;
    let spi_bus = Spi::new_blocking(spi, sck, mosi, miso, Config::default());
    let spi_bus = SPI_BUS_CELL.init(Mutex::new(RefCell::new(spi_bus)));
    let cs_out = Output::new(cs, Level::High, Speed::High);
    let dc_out = Output::new(dc, Level::Low, Speed::High);
    let rst_out = Output::new(rst, Level::High, Speed::High);
    let spi_device = SpiDevice::new(spi_bus, cs_out);
    let interface = SPIInterface::new(spi_device, dc_out);
    let mut delay = embassy_time::Delay;
    let mut display = Builder::new(ST7789, interface)
        .reset_pin(rst_out)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg0))
        .init(&mut delay)
        .expect("st7789 init");
    let _ = display.clear(Rgb565::BLACK);
    ST7789_CELL.init(St7789Spi240x320 {
        display,
        size: preset.size(),
        config: preset.config(),
    })
}

impl Ili9341Spi240x320 {
    pub const WIDTH: u16 = 240;
    pub const HEIGHT: u16 = 320;

    pub fn blit_full(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        mipidsi_blit::blit_full(&mut self.display, Self::WIDTH, Self::HEIGHT, bytes)
    }
}

impl St7789Spi240x320 {
    pub const WIDTH: u16 = 240;
    pub const HEIGHT: u16 = 320;

    pub fn blit_full(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
        mipidsi_blit::blit_full(&mut self.display, Self::WIDTH, Self::HEIGHT, bytes)
    }
}

impl Rgb565Display for Ili9341Spi240x320 {
    fn screen_size(&self) -> ScreenSize {
        self.size
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
        mipidsi_blit::blit_rect(&mut self.display, x, y, w, h, pixels)
    }
}

impl Rgb565Display for St7789Spi240x320 {
    fn screen_size(&self) -> ScreenSize {
        self.size
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
        mipidsi_blit::blit_rect(&mut self.display, x, y, w, h, pixels)
    }
}
