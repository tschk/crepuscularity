# Embedded / framebuffer mode

> **UNSTABLE — still in development; largely untested on real hardware.** The `crepuscularity-embedded` crate and `crepus embedded` commands are experimental. Treat snapshots and host simulators as the supported validation path until board bring-up is documented. APIs may change without a semver-major bump. Pin exact crate versions in firmware projects and expect regressions.

**Status:** CI covers parsing, layout, and host-side PPM snapshots. End-to-end SPI/LTDC/ESP-LCD paths are **not** production-ready and have **not** been broadly exercised on physical panels.

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [TUI](tui.md) · [Native shells](native.md)

Render `.crepus` templates into a fixed-size **RGB565 framebuffer** for microcontrollers, SPI displays, and simulators. The primary integration path is Rust: depend on `crepuscularity-embedded`, use [`Ui`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.Ui.html), and push `ui.rgb565()` to your display driver.

## Install

```toml
[dependencies]
crepuscularity-embedded = "0.2.0"
```

Default features include parsing and `include` (via `crepuscularity-core`). For layout/paint only on device, use `default-features = false` and construct trees on the host (advanced).

## Quick start

```rust
use crepuscularity_embedded::ui;

const UI: &str = include_str!("ui/dashboard.crepus");

let mut ui = ui!(UI, 240, 320, "cpu" => 42, "status" => "nominal");

loop {
    ui.render().expect("render");
    panel.flush_rgb565(ui.rgb565()); // your SPI / LTDC / DMA driver
    if let Some(id) = ui.hit(touch_x, touch_y) {
        // handle element #id
    }
}
```

Without the macro:

```rust
use crepuscularity_embedded::Ui;

let mut ui = Ui::new(240, 320, UI)
    .with("cpu", 42)
    .with("status", "nominal");
ui.set("cpu", 88); // in-loop updates
ui.render()?;
```

## Panel drivers (optional features)

Same crate, feature-gated modules under [`panel`](../crates/crepuscularity-embedded/src/panel/). Implements [`Rgb565Display`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/trait.Rgb565Display.html) so you can **`ui.flush(&mut panel)`** without wiring mipidsi / Embassy / ESP-IDF yourself.

| Feature | Target |
| --- | --- |
| `mipidsi` | Raw RGB565 blit into mipidsi `Display` |
| `embassy-stm32` | ILI9341 / ST7789 on STM32 blocking SPI |
| `esp-idf` | `esp_lcd_panel_draw_bitmap` wrapper |

```toml
crepuscularity-embedded = { version = "0.2.0", default-features = false, features = ["embassy-stm32"] }
```

```rust,ignore
use crepuscularity_embedded::panel::embassy_stm32::ili9341_240x320_static;
let panel = ili9341_240x320_static(spi, sck, mosi, miso, cs, dc, rst);
ui.render()?;
ui.flush(panel)?;
```

[`PanelPreset`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/enum.PanelPreset.html) — `Ili9341_240x320`, `St7789_240x320`, etc. — sets byte order and `Ui` size (always available, no extra feature).

Board walkthroughs: [`examples/advanced/embedded-stm32/`](../examples/advanced/embedded-stm32/) · [`examples/advanced/embedded-esp32/`](../examples/advanced/embedded-esp32/).

## Real hardware (STM32, ESP32, RP2040, …)

`crepuscularity-embedded` fills an **RGB565** buffer; optional [`panel`](#panel-drivers-optional-features) helpers or your own driver push it to the glass.

```
.crepus → Ui::render() → ui.rgb565() → SPI (ILI9341/ST7789) | LTDC framebuffer | esp_lcd → LCD
                              ↑
                    ui.hit(x, y) ← touch controller (FT6336, XPT2046, …)
```

| Setup | MCU examples | Panel IC | Bus | Match `Ui::new(w, h, …)` |
| --- | --- | --- | --- | --- |
| Cheap 2.4″ SPI module | STM32F4, F7, H7 | **ILI9341** | SPI | 240×320 |
| ESP32 dev boards | ESP32-S3 | **ST7789** | SPI / QSPI | 240×320, 240×280, … |
| Pico Display Pack | **RP2040** | ST7789 | SPI | 240×135 |
| Discovery / HMI | STM32F746, H743 | RGB parallel | **LTDC** (+ SDRAM FB) | board resolution (e.g. 480×272) |

**STM32 + ILI9341** — feature `embassy-stm32` or blit a host-built `frame.bin` ([`examples/advanced/embedded-stm32`](../examples/advanced/embedded-stm32/)).

**ESP32-S3 + ST7789** — feature `esp-idf` + [`examples/advanced/embedded-esp32`](../examples/advanced/embedded-esp32/).

**LTDC (framebuffer in RAM)** — avoid a copy; render into scan-out memory:

```rust,ignore
let mut view = Rgb565View::new(screen_size, &mut ltdc_framebuffer[..])?;
ui.render_into(&mut view)?;
```

**Touch:** read `(x, y)` from your controller driver → `ui.hit(x, y)` returns the deepest `#id` (e.g. `#btn-save` in `.crepus`).

**On-device parsing today** needs the default **`std`** feature (heap). Works well on **ESP32** and larger **Cortex-M** parts; validate templates in CI with `crepus embedded check` and `build.rs` (`compile_crepus`).

Step-by-step board notes: [`examples/embedded-dashboard/README.md`](../examples/embedded-dashboard/README.md).

## Example project

[`examples/embedded-dashboard/`](../examples/embedded-dashboard/) — runnable on the host (no hardware required):

```bash
cargo test -p embedded-dashboard
cargo run -p embedded-dashboard
```

`build.rs` validates `ui.crepus` at compile time with `crepuscularity_core::build::compile_crepus`.

## Template variables (target)

Embedded rendering injects the same target flags as other backends:

| Variable | Value |
| --- | --- |
| `crepus_target` | `"embedded"` |
| `is_embedded` | `true` |
| `is_tui` / `is_web` / `is_gui` | `false` |
| `screen_width` / `screen_height` | display size in pixels |

Use in templates: `if {is_embedded && screen_width == 240}`.

## Hit testing

After `render()`, use `ui.hit(x, y)` or `ui.document()` for `node_by_id` and layout bounds. Events are not wired yet; hit-testing is for touch/button layers you add in firmware.

## External display RAM

If the panel buffer is already allocated:

```rust
use crepuscularity_embedded::{Rgb565View, Ui};

let mut ram = [0u16; 240 * 320];
let mut view = Rgb565View::new(screen, &mut ram)?;
ui.render_into(&mut view)?;
// `ram` is ready for DMA
```

## CLI (CI and debug)

| Command | Purpose |
| --- | --- |
| `crepus embedded check ui.crepus` | Parse-only validation for CI / `build.rs` |
| `crepus embedded snapshot ui.crepus --width 240 --height 320 --out preview.ppm` | **Debug only** — RGB888 PPM preview, not for production firmware |

`--ctx` (JSON) and `--var key=value` match `crepus native ir` conventions on snapshot.

## Coverage (v1)

Supported today: column/row flex, sizing (`w-full`, `h-full`, fractions, `w-[Npx]`), spacing (padding, margin, gap), alignment, borders, **Tailwind CSS v4** named colors (`bg-zinc-900`, `text-green-500`, arbitrary `bg-[#…]`), opacity, font-size tokens, ASCII bitmap text, `include` with the same path rules as TUI/native.

Palette is generated from Tailwind v4.3 `theme.css` (`scripts/gen-tailwind-colors.mjs`). Regenerate with: `bun scripts/gen-tailwind-colors.mjs --theme /path/to/theme.css`.

Not in v1: direct LVGL runtime adapter in the framebuffer path, animation, input events, responsive/dark variants, grid/shadows, `no_std` parsing on device. Use the separate [LVGL target](lvgl.md) when you want LVGL Pro XML output.

## Lower-level API

- [`Template`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.Template.html) — file-backed templates, custom framebuffers
- [`render_template_to_framebuffer`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/fn.render_template_to_framebuffer.html) — one-shot render
- [`EmbeddedDocument`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.EmbeddedDocument.html) — retained tree with `by_id` and `node_at`
