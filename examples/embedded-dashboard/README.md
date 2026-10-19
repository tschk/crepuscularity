# embedded-dashboard

> **UNSTABLE** — [`crepuscularity-embedded`](../../crates/crepuscularity-embedded/) is in active development and testing. Pin exact versions; APIs may change before 1.0.

Host-side demo of the firmware loop (no board required):

```bash
cargo test -p embedded-dashboard
cargo run -p embedded-dashboard
```

`build.rs` validates `ui.crepus` at compile time.

Full guide: [`docs/embedded.md`](../../docs/embedded.md).

---

## How this maps to real hardware

Crepuscularity does **not** ship display drivers. It produces a **row-major RGB565 byte buffer** (`ui.rgb565()`). Your existing panel driver (SPI, parallel RGB, LTDC, ESP-LCD, etc.) copies those bytes to the glass.

```
.crepus  →  Ui::render()  →  RGB565 RAM  →  ILI9341 / ST7789 / LTDC / esp_lcd  →  LCD
                ↑                                    ↑
           touch (x,y)  ←  FT6336 / XPT2046 / CST816  ←  user input
```

**Rule:** `Ui::new(width, height, …)` must match the panel’s drawable resolution (e.g. ILI9341 is often **240×320** portrait).

### STM32 + ILI9341 (SPI, 240×320)

Common on Blue Pill shields, 2.4″ SPI modules, and many `stm32f4` / `stm32f7` boards.

| Piece | Typical choice |
| --- | --- |
| MCU | STM32F401, F411, F746, H743 |
| Panel controller | **ILI9341** (RGB565) |
| Bus | **SPI** (+ D/C, reset, optional CS) |
| Rust display stack | [`mipidsi`](https://docs.rs/mipidsi) + [`display-interface-spi`](https://docs.rs/display-interface-spi) + [`embedded-graphics`](https://docs.rs/embedded-graphics) (Embassy or `embedded-hal`) |

After your SPI display init:

```rust,ignore
use crepuscularity_embedded::ui;

const UI: &str = include_str!("ui.crepus");
const W: u16 = 240;
const H: u16 = 320;

// `display` = mipidsi::Display<..., ILI9341, _> (already configured for 240×320 RGB565)
let mut ui = ui!(UI, W, H, "cpu" => 42, "status" => "ok");

loop {
    ui.render().unwrap();
    let bytes = ui.rgb565(); // little-endian RGB565, len = W*H*2

    // Option A: driver-specific RAMWR burst (fastest — check your mipidsi / vendor API)
    display.write_pixels_raw(0, 0, W, H, bytes);

    // Option B: if the panel shares your MCU framebuffer, paint into that slice instead:
    // let mut view = Rgb565View::new(ScreenSize::new(W, H), &mut ltdc_framebuffer)?;
    // ui.render_into(&mut view)?;

    if let Some(id) = ui.hit(touch.x, touch.y) {
        if id == "btn-settings" { /* ... */ }
    }
}
```

Touch (resistive **XPT2046** or capacitive **FT6336** on the same board): read `(x, y)` from your touch driver, pass into `ui.hit(x, y)` — no LVGL required.

### ESP32-S3 + ST7789 (SPI, 240×320 or 240×280)

Typical on TTGO / cheap 1.3″–2″ IPS modules.

| Piece | Typical choice |
| --- | --- |
| MCU | **ESP32-S3** (PSRAM helps for buffer + parse) |
| Panel | **ST7789** |
| Rust | [`esp-idf-sys`](https://docs.rs/esp-idf-sys) + `esp_lcd` panel API, or [`esp-hal`](https://github.com/esp-rs/esp-hal) + `esp_lcd_panel` |

```rust,ignore
// esp-idf style (conceptual)
let mut ui = ui!(include_str!("ui.crepus"), 240, 320, "cpu" => temp_c);

loop {
    ui.render().unwrap();
    // esp_lcd_panel_draw_bitmap(panel, 0, 0, 240, 320, ui.rgb565().as_ptr());
    esp_lcd_panel_draw_bitmap(panel, 0, 0, 240, 320, ui.rgb565())?;
}
```

ESP32 has `std` + heap, so parsing `include_str!` templates **on-device** matches today’s `crepuscularity-embedded` defaults.

### STM32H7 / i.MX RT — parallel RGB (LTDC)

Discovery boards and industrial HMIs often expose a **16-bit RGB parallel** bus. The MCU owns a **framebuffer in SRAM/SDRAM**.

```rust,ignore
use crepuscularity_embedded::{Rgb565View, ScreenSize, Ui};

// Framebuffer already pointed at by LTDC (e.g. 0x24000000, 480×272)
let fb: &mut [u16] = unsafe { core::slice::from_raw_parts_mut(FB_ADDR as *mut u16, 480 * 272) };
let mut view = Rgb565View::new(ScreenSize::new(480, 272), fb).unwrap();

let mut ui = Ui::new(480, 272, include_str!("ui.crepus"));
loop {
    ui.render_into(&mut view).unwrap(); // draws directly into scan-out memory
    // LTDC continuously scans fb; no SPI flush step
}
```

### RP2040 + ST7789 (Pico Display Pack, 240×135)

| Piece | Typical choice |
| --- | --- |
| MCU | **RP2040** |
| Panel | ST7789 (Pimoroni Display Pack 2.x) |
| Rust | Embassy + `mipidsi` |

Same SPI pattern as ILI9341; set `Ui::new(240, 135, …)` to match the Pack resolution.

### What you need in `Cargo.toml` on the firmware crate

```toml
[dependencies]
crepuscularity-embedded = { version = "0.1.9", features = ["std"] }  # parse on device
# plus your HAL / display crates, e.g.:
# mipidsi, display-interface-spi, embedded-graphics, embassy-stm32, esp-idf-svc, ...
```

Use **`default-features = false`** on Crepuscularity only if you pre-build the UI on a host and only run layout/paint on chip (advanced; not covered by this example).

### CI without hardware

```bash
crepus embedded check ui.crepus          # parse gate
crepus embedded snapshot ui.crepus --width 240 --height 320 --out /tmp/preview.ppm  # visual debug
cargo test -p embedded-dashboard
```

### Memory (order of magnitude)

| Resolution | RGB565 buffer |
| --- | --- |
| 128×64 | 16 KiB |
| 240×320 | ~150 KiB |
| 480×272 | ~260 KiB |

Add heap for parse + layout; ESP32-S3 with PSRAM or STM32H7 external RAM is comfortable; tiny AVR targets are not a fit for on-device parse today.

---

This example (`cargo run -p embedded-dashboard`) simulates the **render → flush → hit-test** loop on your PC. Wire `ui.rgb565()` to your chip’s driver using the patterns above.
