<img src="docs-site/static/favicon.svg" alt="" height="50">

# Crepuscularity

> **Stability:** This project is **unstable** and in active development. APIs, CLI flags, and template semantics may change without a semver-major release until **1.0**. Pin exact dependency versions and expect occasional breakage.

[deepwiki](https://deepwiki.com/tschk/crepuscularity)

Think React Native turned into a systems UI toolkit: one compact `.crepus` language can drive GPUI desktop apps, Ratatui terminal UIs, Chromium/Firefox extensions, web output, native mobile shells, embedded panels, and LVGL Pro.

## 60-Second Quick Start

```bash
# 1. Install the CLI (default features = full: dev/preview/tui/aurora)
cargo install --path crates/crepuscularity-cli

# 2. If `crepus` is not found, Cargo installed it under ~/.cargo/bin — add it to PATH.
#    (Rustup normally prepends this for login shells; some terminals omit it.)
export PATH="$HOME/.cargo/bin:$PATH"

# 3. Scaffold and run a GPUI desktop app
crepus init gpui hello && cd hello && cargo run
```

On macOS, GPUI needs the Xcode SDK path — `export` it once or prefix the run:

```bash
SDKROOT=$(xcrun --show-sdk-path) cargo run
```

Keep going from the same install:

```bash
# Browser extension (MV3)
crepus init webext my-extension && cd my-extension && crepus build
# Load dist/unpacked/ in chrome://extensions

# Render a template to HTML on stdout
crepus render examples/showcase/product-dashboard.crepus --ctx examples/showcase/product-dashboard.json
```

## Why Crepuscularity

- **GPUI component workflow with hot reload** — live template updates without recompiling app code
- **Rust-owned MV3 extension target** — write popup, background, and content-script surfaces in `.crepus`, then emit Chromium or Firefox bundles without adopting a JavaScript framework or app bundler as the primary authoring model
- **One syntax, multiple backends** — the same template works across GPUI (native desktop), HTML, Ratatui terminals, and browser extensions today; **`crepuscularity-native`** lowers `.crepus` to JSON **View IR** for SwiftUI / Jetpack Compose shells (see [`examples/native-shells`](examples/native-shells/README.md))
- **Polyglot plugin contract** — host languages consume View IR JSON through `crepus native ir` or the optional `crepuscularity-abi` C session API, then create UI from the typed node tree; drop-in reference packages and native workspace files live under [`plugins/`](plugins/README.md)
- **Terminal UIs without a second UI language** — **`crepuscularity-tui`** maps `.crepus` elements, includes, slots, control flow, and Tailwind-style terminal classes onto Ratatui frames
- **Embedded panels (in testing)** — **`crepuscularity-embedded`** renders `.crepus` into an RGB565 buffer you flush over SPI (**ILI9341**, **ST7789**) or into an **LTDC** framebuffer on STM32/ESP32; see [`docs/embedded.md`](docs/embedded.md) and [`examples/embedded-dashboard`](examples/embedded-dashboard/README.md)
- **Desktop shell for embedded guest apps** — **`crepuscularity-lite`** embeds V8 in a GPUI host with a Rust native-capability bridge, optional file watching, workers, plugin capabilities, and TypeScript/TSX guest transpilation
- **Compile-time and runtime paths** — `view!` macro for zero-overhead AOT compilation; `parse_template` / `render_nodes` for full runtime flexibility and hot reload

## Overview

Write UI in a concise, indentation-based template DSL (`.crepus` files). Templates compile at build time via the `view!` macro or render at runtime with full hot-reload support. The same `.crepus` syntax drives native desktop (GPUI), terminal UIs (Ratatui), browser extensions (MV3), and HTML output — and is the foundation for native mobile backends targeting SwiftUI and Jetpack Compose. React output is available through the Moonshine TSX emit (`crepus web build --emit moonshine`); JSX, Svelte, Vue, Astro, and Angular are supported as **input** syntaxes, not as output targets.

Use [Aurorality](https://github.com/tschk/aurorality) for united SwiftUI macOS + iOS apps.

## Minimal target builds

Put every output in one `crepus.toml`, then run one command:

```toml
[[targets]]
type = "web"
id = "site"
site = "docs-site"
out = "docs-site/dist"
entry = "index.crepus"

[[targets]]
type = "lvgl"
id = "panel"
template = "ui.crepus"
out = "dist/panel.xml"
name = "Panel"
root = "screen"

[targets.vars]
device = "STM32F411"
```

```bash
crepus build
crepus build web
crepus build panel
crepus build --target panel
```

Rust build scripts can use the same manifest for render-output targets:

```rust
let artifacts = crepuscularity::target::write_manifest_file("crepus.toml")?;
```

## Template Syntax (or JSX, Svelte, or Vue templates — see [Input frontends](docs/frontends.md))

```text
div w-full h-full bg-zinc-950 text-white flex flex-col p-8
  div text-2xl font-bold mb-4
    "Hello {name}"
  if {score > 50}
    div text-green-400
      "High score!"
  else
    div text-red-400
      "Keep going"
```

## Features

- **`.crepus` syntax** — indentation-based, Tailwind-style classes
- **Semantic native tags** — optional component tags for native shells (`navigationstack`, `sidebar`, `item`, `menu`, `label`, SF symbols, etc. in the SwiftUI `swiftgen` path)
- **Control flow** — `if/else`, `match`, `for`
- **String interpolation** — `"Hello {name}"`
- **Expressions** — arithmetic, comparison, logical operators, property access
- **Components** — single-file and multi-component files with slot support
- **Hot reload** — live template updates via the runtime renderer
- **Web SEO metadata** — `crepus.toml` web targets generate document head tags, social cards, `robots.txt`, and `sitemap.xml`
- **Browser extensions** — `crepus webext` commands for MV3 extensions; typed Rust browser API wrappers; popup pre-rendered at build time; release WASM optimization when `wasm-opt` is installed; no JS bundler needed
- **IDE integration** — structured JSON events with `--emit-events`

## Output Targets

The `.crepus` DSL is the primary language. Each output target is a renderer that consumes the same parsed template — not a different framework.


| Crate                   | Output                                                         |
| ----------------------- | -------------------------------------------------------------- |
| `crepuscularity-gpui`   | Native desktop (GPUI elements) — primary target                |
| `crepuscularity-tui`    | Ratatui terminal frames with `.crepus` templates and typed handles |
| `crepuscularity-lite`   | GPUI desktop shell with embedded V8, Rust plugins, and guest workers |
| `crepuscularity-native` | View IR JSON for SwiftUI / Compose host apps (not an on-screen renderer) |
| `crepuscularity-abi`    | Optional C ABI sessions for in-process IR rendering and event dispatch |
| `crepuscularity-web`    | HTML strings — server rendering, WASM, browser extensions      |
| `crepuscularity-webext` | MV3 browser extensions — manifest, assets, capability scanning |
| `crepuscularity-embedded` | **UNSTABLE** — RGB565 framebuffer for SPI/LTDC panels (ILI9341, ST7789, ESP-LCD, …); see [`docs/embedded.md`](docs/embedded.md) |
| `crepuscularity-lvgl`   | LVGL Pro XML for panels and embedded UI workflows; see [`docs/lvgl.md`](docs/lvgl.md) |
| `crepuscularity-wasm`   | WASM parser behind `@tschk/crepuscularity-wasm`; `parseTemplate` for all six frontends |


## Input Frontends

The core parser has **six frontends**, selected by file extension, all
compiling to the same AST: indentation (`.crepus`), JSX/HTML tags
(`.csx`/`.jsx`/`.tsx`), Svelte (`.svelte`), Vue SFC (`.vue`), Astro
(`.astro`), and Angular component templates (`.component.html`/`.ng.html`/`.ng`).
The Svelte, Vue, Astro, and Angular frontends are hand-written Rust with no
`svelte`, `vue`, `astro`, or `@angular` dependency.

They compile the **template only**. `<script>`, frontmatter, and component
classes are extracted and never executed, so runes, stores, the Composition
API, `Astro.props`, `@Input`/`@Output`, and lifecycle hooks do not run;
unsupported markup constructs are hard parse errors rather than silent drops.
[`docs/frontends.md`](docs/frontends.md) lists exactly what is and is not
supported.

From JavaScript, `parseTemplate(source, filename)` in
`@tschk/crepuscularity-wasm` is the single entry point for all six.

## CLI Commands

```bash
crepus init <kind> <name>            # Scaffold web, webext, tui, mobile, native, ios, or gpui apps
crepus new <name>                    # Alias for crepus init gpui <name>
crepus dev [--emit-events]           # Hot-reload dev loop
crepus build [type-or-id] [--target ID]  # Build crepus.toml targets, selected type/id, or cargo fallback
crepus preview <file.crepus>         # Live preview

crepus web new <name>                # Alias for crepus init web <name>
crepus webext new <name>             # Alias for crepus init webext <name>
crepus webext build [--app PATH] [--browser chromium|firefox]  # Build to dist/unpacked/ or dist/<browser>/
crepus webext manifest [--app PATH] [--browser chromium|firefox]  # Print manifest.json

crepus aurora dev [--watch DIR]      # SwiftUI hot-reload dev server (via Aurorality)
crepus aurora build [--watch DIR]    # Compile .crepus → View IR JSON
crepus aurora new <name>             # Scaffold SwiftUI + Rust project

crepus ios new <name>                # Alias for crepus init ios <name>
crepus ios generate [--dir]          # Run xcodegen (finds crepus.toml [ios] upward)
crepus ios build [--dir] [...]       # Simulator build via xcodebuild

crepus mobile new <name>             # Preferred iOS + Android scaffold
crepus mobile dev [--platform all]   # View IR hot-reload server on port 4001
crepus mobile build --platform android|ios|all
crepus mobile run --platform android|ios

crepus native ir <file.crepus>       # Emit View IR JSON for plugins and native shells
crepus native sync <file.crepus> --dir <app> [--out FILE]  # Write View IR fixtures into native containers

crepus embedded check <file.crepus>  # UNSTABLE: validate template for firmware CI
crepus embedded snapshot … --out x.ppm  # UNSTABLE: debug PPM only (use Ui + rgb565() on device)
```

## Documentation

- **Rendered site (GitHub Pages):** [here](https://crepuscularity.undivisible.dev) — WASM landing page plus HTML generated from the Markdown in [`docs/`](docs/). Built in CI with `crepus web build --manifest docs-site/crepus.toml`.
- **Sources:** [docs/README.md](docs/README.md) indexes [DSL](docs/dsl.md) (including SwiftUI semantic tag mappings), [components](docs/components.md), [CLI](docs/cli.md), [runtime/reactivity](docs/runtime.md), [GPUI](docs/gpui.md), [TUI](docs/tui.md), [embedded / framebuffer](docs/embedded.md) (**UNSTABLE** — STM32, ESP32, SPI panels), [Lite](docs/lite.md), [native shells](docs/native.md), [polyglot plugins](docs/polyglot.md), and [extensions](docs/webext.md). Compiler-focused detail stays in-repo as [CREPUS_WEB_IMPLEMENTATION_SPEC.md](docs/CREPUS_WEB_IMPLEMENTATION_SPEC.md) but is not shipped on the public docs site.
- **Native shells:** [examples/native-shells](examples/native-shells/README.md) — SwiftUI/XcodeGen (**`ios/`**), Gradle/Compose (**`android/`**), and shared **View IR** `fixture.json` next to **`crepuscularity-native`** (replaces the old separate **`crepuscularity-native-ui`** checkout).
- **Contributors & coding agents:** root [**`AGENTS.md`**](AGENTS.md) is the canonical instructions (macOS `SDKROOT`, `cargo fmt` / `clippy` / `test` before push, workspace layout, DSL notes). **`CLAUDE.md`** is a symlink to **`AGENTS.md`** so duplicate context files cannot drift.

## GPUI — Tailwind Class Support

The GPUI renderer maps Tailwind-style class strings to native GPUI style calls. This is a native desktop renderer, not a browser, so the mapping is intentionally selective.

### Supported


| Category             | Classes                                                                                                                                            |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Layout**           | `flex`, `grid`, `block`, `hidden`, `flex-col/row`, `flex-wrap`, `flex-1/auto/none`, `grow`, `shrink`                                               |
| **Justify / Align**  | `justify-start/end/center/between/around`, `items-start/end/center/baseline`, `content-`*                                                          |
| **Align self**       | `self-start`, `self-end`, `self-center`, `self-stretch`, `self-baseline`, `self-auto`                                                              |
| **Sizing**           | `w-`*, `h-*`, `size-*`, `min-w/h-*`, `max-w/h-*` — numbers, fractions, `full`, `auto`, `[Npx]`                                                     |
| **Aspect ratio**     | `aspect-square`, `aspect-video`, `aspect-auto`, `aspect-[N/M]`                                                                                     |
| **Spacing**          | `p-`*, `px-*`, `py-*`, `pt/pb/pl/pr-*`, `m-*`, `mx/my-*`, `mt/mb/ml/mr-*`, `gap-*`, `gap-x/y-*`                                                    |
| **Position**         | `absolute`, `relative`, `top/bottom/left/right/inset-`*                                                                                            |
| **Overflow**         | `overflow-hidden`, `overflow-x/y-hidden`                                                                                                           |
| **Colors**           | Full Tailwind palette (`bg/text/border-{family}-{shade}`), hex (`bg-[#rrggbb]`), hsla, rgba with `/alpha`                                          |
| **Border**           | `border`, `border-0/2/4/8`, per-side `border-t/b/l/r-`*, `border-dashed`                                                                           |
| **Border radius**    | `rounded-`* — all sizes, all sides, all corners                                                                                                    |
| **Typography**       | Font weight (`font-thin` → `font-black`), style (`italic`), size (`text-xs` → `text-9xl`, `text-[Npx]`)                                            |
| **Typography**       | Alignment (`text-left/center/right`), decoration (`underline`, `line-through`, `decoration-`*)                                                     |
| **Typography**       | Line height (`leading-`*, `leading-[N]`), tracking (`tracking-*`, `tracking-[Npx]`), truncation (`truncate`, `text-ellipsis`, `whitespace-nowrap`) |
| **Typography**       | Text transform (`uppercase`, `lowercase`, `capitalize`, `normal-case`)                                                                             |
| **Font**             | `font-['Family']`, `line-clamp-N`, `font-features` via `FontFeatures` API                                                                          |
| **Shadow**           | `shadow-2xs` → `shadow-2xl`, `shadow-none`                                                                                                         |
| **Ring**             | `ring`, `ring-0/1/2/4/8`, `ring-[Npx]` — rendered as box-shadow spread                                                                             |
| **Opacity**          | `opacity-N` (0–100), `opacity-{expr}`                                                                                                              |
| **Cursor**           | `cursor-default/pointer/text/move/not-allowed` and all resize variants                                                                             |
| **Grid**             | `grid-cols-N`, `grid-rows-N`, `col-span-N`, `col-start/end-N`, `row-span/start/end-N`                                                              |
| **Arbitrary values** | `w-[Npx]`, `bg-[#hex]`, `text-[size]`, `rounded-[Npx]`, `border-[Npx]`, `aspect-[N/M]`, etc.                                                       |
| **Dynamic context**  | `bg-{expr}`, `text-{expr}`, `border-{expr}`, `opacity-{expr}` — evaluated against template context                                                 |
| **State prefixes**   | `hover:`, `focus:`, `active:` — accepted silently (state styling requires `.hover()`/`.on_click()` handlers in code)                               |


### Not Supported (GPUI hard limits)

These cannot be added without forking GPUI itself:


| Gap                                  | Reason                                                                                 |
| ------------------------------------ | -------------------------------------------------------------------------------------- |
| `ring-{color}`                       | Ring color customisation requires architectural change; default ring is blue-500/50    |
| `md:` / `lg:` / `xl:` breakpoints    | No CSS cascade or viewport queries — GPUI uses Taffy layout                            |
| `dark:` variant                      | No built-in dark mode detection — use `bg-{theme.surface}` context expressions instead |
| `group-hover:` / `peer:`             | No selector/relationship system                                                        |
| `before:` / `after:` pseudo-elements | No pseudo-elements — add child `div` nodes in the template instead                     |
| `z-`* (z-index)                      | GPUI uses painter's algorithm / GPU layers, not z-index                                |
| `float`                              | Not supported by Taffy layout engine                                                   |
| `ring-inset`                         | GPUI has no inset box-shadow — accepted silently                                       |


## Project Structure

```text
crates/
  crepuscularity/           Facade re-exporting prelude
  crepuscularity-core/      AST, parser (indent / JSX / Svelte / Vue), evaluator
  crepuscularity_macros/    Compile-time view! proc-macro
  crepuscularity-runtime/   Hot-reload renderer (Tailwind → GPUI styler)
  crepuscularity-web/       HTML backend
  crepuscularity-webext/    Browser extension support
  crepuscularity-gpui/      GPUI prelude + view! macro
  crepuscularity-tui/       Ratatui backend + template_refs! handles
  crepuscularity-lite/      GPUI + V8 shell and native plugin bridge
  crepuscularity-native/    View IR JSON for SwiftUI / Compose shells
  crepuscularity-abi/       C ABI sessions over the View IR
  crepuscularity-wasm/      WASM parser; @tschk/crepuscularity-wasm npm package
  crepuscularity-reactive/  WASM signals, memos, effects, hydration lifecycle
  crepuscularity-components/ Rust component catalog registry
  crepuscularity-embedded/  RGB565 framebuffer for SPI/LTDC panels
  crepuscularity-lvgl/      LVGL Pro XML generation
  crepuscularity-lsp/       Language server
  crepuscularity-tauri/     Tauri host integration
  crepuscularity-tauri-macros/  Proc macros for the Tauri host
  crepuscularity-plugin-bindgen/ Plugin binding generation
  crepuscularity-cli/       crepus CLI
examples/
  examples.toml             Catalog by target (see examples/README.md)
  extensions/               Git symlinks to undivisible/anywhere, rs_vimium
  weather/                  Weather app example
  quicknote/                Browser extension example (in-repo)
  native-shells/            SwiftUI/XcodeGen and Gradle View IR hosts
```
## Other stable polished examples
- [herdr-gui](https://github.com/undivisible/herdr-gui) - a gui interface for herdr built with crepuscular gpui
- [drift-wallpaper](https://github.com/undivisible/drift-wallpaper) - settings menu made with crepuscular gpui
- [rs_vimium](https://github.com/undivisible/rs_vimium) - name self explanatory built with the crepuscular webextension framework
- [tsc.hk](https://github.com/tschk/tsc.hk) - the tschk website is built with crepuscularity-web
- [crepuscularity.tsc.hk](docs-site) - official crepuscularity website made with yours truly
- [inauguration.tsc.hk](https://github.com/tschk/inauguration/tree/master/docs-site) - inauguration website made with crepuscularity + inauguration

also have a working cross platform mobile app that ill have an example for soon :)


## Building

Install the CLI:

```bash
# default features include desktop/tui/aurora (`crepus dev`)
cargo install --path crates/crepuscularity-cli
```

On macOS, GPUI requires the Xcode SDK path:

```bash
SDKROOT=$(xcrun --show-sdk-path) cargo build
```

When Xcode uses the downloadable Metal Toolchain component, let Cargo inherit the exact Xcode environment GPUI's build script needs:

```bash
eval "$(scripts/metal-env.sh)"
cargo build
scripts/metal-env.sh -- cargo check -p crepuscularity-gpui
scripts/metal-env.sh --check
```

The required variables are `SDKROOT` for SDK headers, `DEVELOPER_DIR` for the active Xcode, and `TOOLCHAINS=Metal` for `xcrun` toolchain selection. `scripts/metal-env.sh` reads the downloaded Metal toolchain search path from `xcodebuild -showComponent MetalToolchain -json`, exports the short selector that `xcrun -sdk macosx metal` accepts, and prepends `Metal.xctoolchain/usr/bin` to `PATH` for direct `metal` / `metallib` probes. If `--check` reports `xcrun_metal=failed`, run `xcodebuild -downloadComponent MetalToolchain` or install the component from Xcode Settings > Components before debugging Cargo code.

## License

ISC License — see [LICENSE](LICENSE).

