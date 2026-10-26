# Embedded / framebuffer mode

> **UNSTABLE — still in development; largely untested on real hardware.** The `crepuscularity-embedded` crate and `crepus embedded` commands are experimental. Treat snapshots and host simulators as the supported validation path until board bring-up is documented. APIs may change without a semver-major bump. Pin exact crate versions in firmware projects and expect regressions.

**Status:** CI covers parsing, layout, and host-side PPM snapshots. End-to-end SPI/LTDC/ESP-LCD paths are **not** production-ready and have **not** been broadly exercised on physical panels.

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [TUI](tui.md) · [Native shells](native.md)

Render `.crepus` templates into a fixed-size **RGB565 framebuffer** for microcontrollers, SPI displays, and simulators. The primary integration path is Rust: depend on `crepuscularity-embedded`, use [`Ui`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.Ui.html), and push `ui.rgb565()` to your display driver.

## Install

```toml
[dependencies]
crepuscularity-embedded = "0.1.9"
```

Default features include parsing and `include` (via `crepuscularity-core`). For layout/paint only on device, use `default-features = false` and construct trees on the host (advanced).

Within each draw, repeated includes share their source and parsed nodes. Props,
component defaults, slots, and include-depth checks still run for every
occurrence. The cache is discarded after the draw, so disk and virtual-file
changes appear on the next draw without an explicit reload. Separate virtual
bundles keep separate include snapshots even when their paths match.
The std renderer borrows unchanged sibling scopes and supplied slot caller scopes,
avoiding a full parent-scope copy per repeated include. It reuses one scratch
context per loop, restoring row overrides between iterations. Declarations still
clone their local scope when needed. Explicitly owned `TemplateContext.slot` values
remain supported. This reduces context copying without changing framebuffer layout
or painting.

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

`Ui::rgb565_for_panel()` applies the selected panel byte order. RGB output borrows
the framebuffer directly; Bgr byte-swapping reuses scratch storage after its first
allocation, without cloning the source frame. Call `render()` first to update
pixels. `flush()` keeps its existing render-and-send behavior.

## Panel drivers (optional features)

Same crate, feature-gated modules under [`panel`](../crates/crepuscularity-embedded/src/panel/). Implements [`Rgb565Display`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/trait.Rgb565Display.html) so you can **`ui.flush(&mut panel)`** without wiring mipidsi / Embassy / ESP-IDF yourself.

| Feature | Target |
| --- | --- |
| `mipidsi` | Raw RGB565 blit into mipidsi `Display` |
| `embassy-stm32` | ILI9341 / ST7789 on STM32 blocking SPI |
| `esp-idf` | `esp_lcd_panel_draw_bitmap` wrapper |

```toml
crepuscularity-embedded = { version = "0.1.9", default-features = false, features = ["embassy-stm32"] }
```

```rust,ignore
use crepuscularity_embedded::panel::embassy_stm32::ili9341_240x320_static;
let panel = ili9341_240x320_static(spi, sck, mosi, miso, cs, dc, rst);
ui.render()?;
ui.flush(panel)?;
```

[`PanelPreset`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/enum.PanelPreset.html) — `Ili9341_240x320`, `St7789_240x320`, etc. — sets byte order and `Ui` size (always available, no extra feature).

STM32 mipidsi adapters consume native framebuffer RGB565 values and let mipidsi
encode the bus transfer. `flush_framebuffer` skips panel byte swapping for adapters
whose `Rgb565Display::uses_native_rgb565_bytes()` returns `true`; offsets still
apply. The default remains `false` for raw wire drivers, including ESP-IDF.
`Ui::rgb565_for_panel()` still produces panel-encoded bytes, and direct mipidsi
`blit_full`/`blit_rect` helpers still accept little-endian RGB565 input.

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

Ordinary and named includes inherit these reserved values from their rendering
parent after component defaults and props, so inherited environment values take
precedence. Other parent variables still require explicit props. Slots retain
their captured parent context. The low-level `render_nodes_to_document` API uses
the supplied context as-is; call `with_embedded_target` first when target variables
are needed there.

Web `embed` islands are unsupported. An active island returns a render error
before framebuffer painting, including islands reached through includes, slots
or button children. Guard web-only islands with `if {is_web}` and provide an
embedded fallback; skipped conditional branches are not evaluated.

Buttons take their first visible text/span label from evaluated children, including
active control flow, includes and slots. Local declarations use their evaluated
scope. An explicitly empty first label wins over later labels; arbitrary layout
container descendants remain excluded from primary-label selection.

## Hit testing

After `render()`, use `ui.hit(x, y)` or `ui.document()` for `node_by_id` and layout bounds. Events are not wired yet; hit-testing is for touch/button layers you add in firmware.

Disabled buttons are not supported by this renderer. A present `disabled`
attribute returns a render error rather than exposing an enabled click handler
and hit target. Boolean `false` or `null` keeps normal button behavior; a string
such as `"false"` still counts as present. Inactive control-flow branches are not
validated. Handle render errors; this does not implement disabled styling or
disabled hit-test metadata, and existing last-good-frame handling is unchanged.

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
Snapshots select the parser from the input filename and resolve relative includes
beside that file, independently of the command’s working directory.

## Coverage (v1)

Supported today: column/row flex, sizing (`w-full`, `h-full`, fractions, `w-[Npx]`), spacing (padding and gap), cross-axis alignment and main-axis justification (`start`, `end`, `center`, `between`, `around`, `evenly`), borders, **Tailwind CSS v4** named colors (`bg-zinc-900`, `text-green-500`, arbitrary `bg-[#…]`), hidden/invisible elements and zero opacity, fixed 8×8 ASCII bitmap text, `include` with the same path rules as TUI/native.

Border color defaults to the inherited text color. Color tokens such as
`border-red-500` set only the color; add `border`, `border-2`, or `border-4` to
enable the border. `border-0` remains disabled when followed by a color token.

Borders reserve space inside the outer rectangle before padding. Automatic text
sizes include border and padding; explicit sizes describe the outer rectangle.
Padding utilities preserve explicit zero overrides: `p-4 pl-0` keeps the left
edge at zero while retaining 16 pixels on the other edges. Side utilities take
precedence over axis utilities, which take precedence over shorthand padding;
the last utility in the same category wins.

Explicit root widths and heights use the same size hints as child elements,
relative to the screen. Auto-sized root containers retain the screen allocation.
The first visible root background still supplies the canvas clear color.

Auto-sized child containers measure their visible descendants before allocation:
row contents sum widths and take the maximum height; columns do the reverse.
Gaps, border and padding contribute to that size. Non-text containers retain a
minimum content size of 32×16 pixels. Relative and flexible children have no
intrinsic size contribution and resolve within the eventual parent allocation.
Hidden children contribute neither extent nor gaps; invisible children keep
layout space. This also applies to generated if/for/match wrappers.

Empty conditionals, loops, and unmatched `match` blocks do not reserve layout
space or gaps. Their structural nodes remain available in the document tree.

Text colors inherit through containers; a child color overrides its ancestors
for that subtree. Each document root starts with the default text color.

The `hidden` attribute also removes an element and its descendants from layout,
painting, and hit testing. Bind `hidden={condition}` to toggle it; boolean `false`
and `null` omit the attribute, while literal strings (including `"false"`) keep
it present. A `hidden` class still hides the element independently.

Fixed-size containers permit visible child overflow. Hit testing searches those
children outside ancestor rectangles, matching painting, while retaining screen
clipping and hidden/invisible subtree suppression. Unnamed descendants bubble to
an ancestor ID; later overlapping siblings and roots keep precedence.

Equal flex children share all remaining pixels after fixed sizes and gaps;
indivisible pixels go to the first flex children in source order.

Custom `FontMetrics` set cell spacing and clipping for the fixed bitmap font.
Smaller cells crop glyphs, larger cells add spacing, and a zero cell dimension
suppresses text painting. Bitmap glyphs are not scaled.

Fractional opacity is parsed but currently paints opaque; framebuffer compositing is not implemented. Margin, font-size, and font-weight tokens are retained in `EmbeddedStyle` but do not currently affect layout or painting. Parsed zero padding overrides are resolved into edge values. Manually constructed `EmbeddedStyle` fields still use zero for unset values, so a zero side field falls back to broader nonzero padding.

The shared `crepuscularity-colors` palette is generated from Tailwind v4.3 `theme.css` (`scripts/gen-tailwind-colors.mjs`). Regenerate with: `bun scripts/gen-tailwind-colors.mjs --theme /path/to/theme.css`.

Not in v1: direct LVGL runtime adapter in the framebuffer path, animation, input events, responsive/dark variants, grid/shadows, `no_std` parsing on device. Use the separate [LVGL target](lvgl.md) when you want LVGL Pro XML output.

## Lower-level API

- [`Template`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.Template.html) — file-backed templates, custom framebuffers
- [`render_template_to_framebuffer`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/fn.render_template_to_framebuffer.html) — one-shot render
- [`EmbeddedDocument`](https://docs.rs/crepuscularity-embedded/latest/crepuscularity_embedded/struct.EmbeddedDocument.html) — retained tree with `by_id` and `node_at`
