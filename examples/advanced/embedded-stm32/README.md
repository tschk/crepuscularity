# STM32 + ILI9341 (240×320 SPI)

> **UNSTABLE** — embedded UI is in active development. The STM32F411 firmware (no `std`) renders `ui.crepus` on-chip from a compile-time template (`embedded_document!`); no parser runs on the device.

## Two crates

| Crate | Runs on | Purpose |
| --- | --- | --- |
| [`host-sim`](host-sim/) | Your Mac / Linux | `Ui` + `embedded_template!` → RGB565 bytes via the `std` runtime parser |
| [`stm32f411-ili9341`](stm32f411-ili9341/) | STM32F411 (`thumbv7em`) | `no_std` + features `embassy-stm32`, `macros`: `embedded_document!("../ui.crepus")`, live `cpu`/`status` values, painted in 32-row bands |

## 1. Validate templates on the host

```bash
cargo run -p embedded-stm32-host-sim
```

Uses [`crepuscularity-embedded`](../../../crates/crepuscularity-embedded/) with:

- `embedded_template!("../ui.crepus")` — compile-time parse check  
- `Ui::flush()` — render + `Rgb565Display` (mock panel RAM)

## 2. Frame size on the F411

A full 240×320 RGB565 frame is 150 KiB and the F411 has 128 KiB of RAM, so the firmware lays
out the document once per update and paints it into a 240×32 band buffer ten times, blitting
each band with `flush_rgb565_rect`.

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

Bands are native little-endian RGB565, which the mipidsi blit decodes before encoding the SPI
transfer. If red and blue are reversed on a physical module, check the controller's RGB/BGR
channel-order configuration.

## Roadmap (STM32)

1. **Done:** compile-time bake (`embedded_document!`) → on-device evaluate/layout/paint without the `std` parser (this example)  
2. **Later:** `for`/`match`/includes in compiled templates  
3. **Later:** live `Ui::render()` runtime parsing on STM32 when core is `alloc`-only

## Related docs

- [`docs/embedded.md`](../../../docs/embedded.md) — API reference
- [`examples/embedded-dashboard`](../../embedded-dashboard/) — simpler host-only demo
