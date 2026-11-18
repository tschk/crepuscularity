//! Exercise the actual adapter against a fake ESP-IDF FFI boundary on the host.
extern crate alloc;
extern crate self as esp_idf_sys;

pub use crepuscularity_embedded::{DisplayError, PanelConfig, Rgb565Display, ScreenSize};
pub mod panel {
    pub use crepuscularity_embedded::panel::preset;
}

#[allow(non_camel_case_types)]
pub type esp_lcd_panel_handle_t = *mut core::ffi::c_void;
pub const ESP_OK: u32 = 0;

#[derive(Default)]
struct Call {
    rect: Option<(i32, i32, i32, i32)>,
    result: i32,
}

/// The adapter's handle points to the test's live `Call` record.
///
/// # Safety
/// `handle` must point to a uniquely borrowed `Call` for this call.
pub unsafe fn esp_lcd_panel_draw_bitmap(
    handle: esp_lcd_panel_handle_t,
    x: i32,
    y: i32,
    x_end: i32,
    y_end: i32,
    _: *const core::ffi::c_void,
) -> i32 {
    let call = unsafe { &mut *handle.cast::<Call>() };
    call.rect = Some((x, y, x_end, y_end));
    call.result
}

#[path = "../src/panel/esp_idf.rs"]
mod adapter;

#[test]
fn esp_adapter_validates_buffers_and_preserves_rectangle_endpoints() {
    let mut call = Call::default();
    let mut display = unsafe {
        adapter::EspIdfLcdPanel::new(
            (&mut call as *mut Call).cast(),
            panel::preset::PanelPreset::Ili9341_240x320,
        )
    };
    assert!(display.flush_rgb565_rect(0, 0, 2, 1, &[0]).is_err());
    assert!(display
        .flush_rgb565_rect(0, 0, u16::MAX, u16::MAX, &[])
        .is_err());
    assert_eq!(call.rect, None);
    display
        .flush_rgb565_rect(u16::MAX, u16::MAX, 2, 1, &[0; 4])
        .unwrap();
    assert_eq!(call.rect, Some((65535, 65535, 65537, 65536)));
    call.result = -1;
    assert!(display.flush_rgb565_rect(0, 0, 1, 1, &[0; 2]).is_err());
    assert_eq!(call.result, -1);
    assert_eq!(display.screen_size(), ScreenSize::new(240, 320));
    assert_eq!(display.config(), PanelConfig::ili9341_240x320());
    assert!(!display.handle().is_null());
}
