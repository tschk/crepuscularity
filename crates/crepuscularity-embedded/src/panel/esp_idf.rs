//! ESP-IDF [`esp_lcd`](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/peripherals/lcd.html) adapter.

use alloc::format;
use alloc::string::ToString;

use crate::panel::preset::PanelPreset;
use crate::{DisplayError, PanelConfig, Rgb565Display, ScreenSize};

/// RGB565 panel driven by an initialized `esp_lcd` panel handle.
///
/// Create after `esp_lcd_new_panel_*` + `esp_lcd_panel_init` in your board setup.
///
/// # Safety
///
/// `handle` must remain valid for the lifetime of this struct and must not be used by
/// other code while borrowed.
pub struct EspIdfLcdPanel {
    handle: esp_idf_sys::esp_lcd_panel_handle_t,
    size: ScreenSize,
    config: PanelConfig,
}

impl EspIdfLcdPanel {
    /// Wrap an ESP-IDF LCD panel handle with a [`PanelPreset`] (size + byte order metadata).
    ///
    /// # Safety
    ///
    /// See struct-level safety note.
    pub unsafe fn new(handle: esp_idf_sys::esp_lcd_panel_handle_t, preset: PanelPreset) -> Self {
        Self {
            handle,
            size: preset.size(),
            config: preset.config(),
        }
    }

    pub fn handle(&self) -> esp_idf_sys::esp_lcd_panel_handle_t {
        self.handle
    }

    pub fn config(&self) -> PanelConfig {
        self.config
    }
}

impl Rgb565Display for EspIdfLcdPanel {
    fn screen_size(&self) -> ScreenSize {
        self.size
    }

    fn flush_rgb565_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        pixels: &[u8],
    ) -> Result<(), DisplayError> {
        let expected = (w as usize)
            .checked_mul(h as usize)
            .and_then(|pixels| pixels.checked_mul(2))
            .ok_or_else(|| {
                DisplayError::Message("esp_lcd draw: dimensions exceed addressable memory".into())
            })?;
        if pixels.len() != expected {
            return Err(DisplayError::Message(
                "esp_lcd draw: pixel buffer length mismatch".to_string(),
            ));
        }
        let x_end = i32::from(x) + i32::from(w);
        let y_end = i32::from(y) + i32::from(h);
        let err = unsafe {
            esp_idf_sys::esp_lcd_panel_draw_bitmap(
                self.handle,
                x as i32,
                y as i32,
                x_end,
                y_end,
                pixels.as_ptr() as *const core::ffi::c_void,
            )
        };
        if err == esp_idf_sys::ESP_OK as i32 {
            Ok(())
        } else {
            Err(DisplayError::Message(format!(
                "esp_lcd_panel_draw_bitmap failed: {err}"
            )))
        }
    }
}
