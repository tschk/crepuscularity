# STM32 + ILI9341 (240×320 SPI)

> **UNSTABLE** — embedded UI is in active development. On STM32F411 (no `std`), **parse `.crepus` on-chip is not supported yet**; use the host → panel workflow below.

## Two crates

| Crate | Runs on | Purpose |
| --- | --- | --- |
| [`host-sim`](host-sim/) | Your Mac / Linux | `Ui` + `embedded_template!` → RGB565 bytes (same API you will use when on-device parse lands) |
| [`stm32f411-ili9341`](stm32f411-ili9341/) | STM32F411 (`thumbv7em`) | `crepuscularity-embedded` feature `embassy-stm32` + blit `frame.bin` |

## 1. Validate templates on the host

```bash
cargo run -p embedded-stm32-host-sim
```

Uses [`crepuscularity-embedded`](../../../crates/crepuscularity-embedded/) with:

- `embedded_template!("../ui.crepus")` — compile-time parse check  
- `Ui::flush()` — render + `Rgb565Display` (mock panel RAM)

## 2. Export a frame for the board

```bash
WRITE_FRAME=stm32f411-ili9341/frame.bin cargo run -p embedded-stm32-host-sim
```

Writes **153 600 bytes** (240×320 RGB565) for `include_bytes!` in firmware.

## 3. Flash the STM32

Default wiring (common F411 “Black Pill” + 2.4″ ILI9341 SPI shield):

| Signal | Pin |
| --- | --- |
| SCK | PA5 |
| MISO | PA6 |
| MOSI | PA7 |
| CS | PA4 |
| D/C | PA3 |
| RST | PA2 |

```bash
rustup target add thumbv7em-none-eabihf
cd examples/advanced/embedded-stm32/stm32f411-ili9341
cargo build --release --target thumbv7em-none-eabihf
probe-rs run --chip STM32F411RETx target/thumbv7em-none-eabihf/release/embedded-stm32-f411-ili9341
```

You should see the dashboard from `ui.crepus` on the panel. Default pins are in [`panel/embassy_stm32.rs`](../../../crates/crepuscularity-embedded/src/panel/embassy_stm32.rs); fork that init if your board differs.

### Panel byte order

Keep `frame.bin` in little-endian RGB565 order: the firmware's direct
`panel.blit_full(FRAME)` decodes that format before mipidsi encodes the SPI transfer.
Use the host simulator's default panel configuration when exporting this file.
Do not byte-swap it with the ST7789 preset. If red and blue are reversed on a
physical module, check the controller's RGB/BGR channel-order configuration.

This differs from `Ui::flush(&mut panel)`: the STM32 adapter requests native
framebuffer bytes there, so `flush_framebuffer` bypasses panel byte swapping.

## Roadmap (STM32)

1. **Now:** host render → `frame.bin` → SPI blit (this example)  
2. **Next:** parse cache + `embedded_template!` (done on host)  
3. **Later:** compile-time bake → on-device layout/paint without `std` parser  
4. **Later:** live `Ui::render()` loop on STM32 when core is `alloc`-only

## Related docs

- [`docs/embedded.md`](../../../docs/embedded.md) — API reference
- [`examples/embedded-dashboard`](../../embedded-dashboard/) — simpler host-only demo
