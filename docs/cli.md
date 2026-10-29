# CLI Guide

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [Extensions](webext.md)

The `crepus` CLI provides commands for scaffolding, building, and developing crepuscularity applications.

## Installation

```bash
# Default features = full (`crepus dev`, `preview`, TUI, Aurora)
cargo install --path crates/crepuscularity-cli
# Slim web/native/embedded only:
# cargo install --path crates/crepuscularity-cli --no-default-features
```

## Commands

### `crepus init <kind> <name>`

Scaffold a new app or target workspace:

```bash
crepus init gpui my-app
crepus init web my-site
crepus init webext my-extension
crepus init tui my-terminal-app
crepus init native my-native-shell
crepus init ios my-ios-host
```

Supported kinds are `web`, `webext`, `tui`, `native`, `ios`, and `gpui`. Existing scaffold commands remain aliases: `crepus new <name>` maps to `crepus init gpui <name>`, `crepus web new <name>` maps to `crepus init web <name>`, `crepus webext new <name>` maps to `crepus init webext <name>`, and `crepus ios new <name>` maps to `crepus init ios <name>`.

### `crepus new <name>`

Scaffold a new GPUI application:

```bash
crepus init gpui my-app
cd my-app
SDKROOT=$(xcrun --show-sdk-path) cargo run
```

Creates:
- `Cargo.toml` with crepuscularity dependencies
- `src/main.rs` with GPUI boilerplate
- `views/` directory for `.crepus` templates

### `crepus inspect`

Dump AST / HTML / View IR / context, or open a live GPUI preview for one `.crepus` file.
Replaces the old `crepus-dev` binary.

```bash
crepus inspect app.crepus --mode ast
crepus inspect app.crepus --mode render --var name=Ada
crepus inspect app.crepus --mode ir
crepus inspect app.crepus --mode ctx
crepus inspect app.crepus --mode preview
```

### `crepus dev`

Rebuild-and-relaunch loop: watches `src/` and `Cargo.toml` (not a root-level `build.rs`),
then rebuilds with Cargo and restarts the app. It does not patch a running app; templates
reload in place only when the app renders them through `HotReloadView` (see `docs/gpui.md`),
or with `crepus preview`.

```bash
crepus dev
crepus dev --bin my-binary
crepus dev --release
crepus dev --emit-events  # IDE integration
```

Options:
- `--bin NAME` — specify which binary to run (for workspaces)
- `--debug`, `--dev`, `--release` — choose the build mode; default is `--debug`
- `--opt-level none|fast|size|aggressive` — shared optimization level; release defaults to `fast`, debug/dev default to `none`
- `--emit-events` — emit structured JSON events to stdout

### `crepus build`

If the current directory or a parent has `crepus.toml` with `[[targets]]`, build all targets by default. Pass a target type or target id as the positional selector to build a subset. The existing `--target ID` selector remains supported. With no manifest targets, this falls back to the Cargo build wrapper.

```bash
crepus build
crepus build web
crepus build panel
crepus build --target panel
crepus build --manifest ./crepus.toml
crepus build --release --opt-level size
```

```toml
[[targets]]
type = "web"
id = "site"
site = "docs-site"
out = "docs-site/dist"
entry = "index.crepus"

[[targets]]
type = "webext"
id = "extension"
app = "examples/advanced/extensions/rs_vimium"

[[targets]]
type = "native"
id = "fixture"
template = "views/main.crepus"
out = "dist/fixture.json"
root = "pretty"

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

Supported manifest target types today: `web`, `webext`, `native` / `ir`, `lvgl`, and `embedded`. Build mode flags are shared across all target types. `web` and `webext` map `--release` to Cargo release WASM builds and run post-build WASM optimization when the effective optimization level is not `none`; render-only targets accept the same flags for manifest consistency.

Rust code can use the same manifest path for render-output targets (`web`, `native`, `lvgl`):

```rust
let artifacts = crepuscularity::target::write_manifest_file("crepus.toml")?;
```

### `crepus preview <file.crepus>`

Live preview a single template file:

```bash
crepus preview views/dashboard.crepus
```

Opens a window showing the rendered template with hot-reload.

To provide context variables, create `context.toml` in the same directory:

```toml
# views/context.toml
username = "alice"
score = 1200
logged_in = true
```

## Static web sites (`crepus web`)

Author pages in `.crepus` (same virtual-file semantics as `crepus web serve`). **`crepus web build`** compiles the site’s `runtime/` crate to **`wasm32-unknown-unknown`**, runs **wasm-bindgen**, and writes a **`dist/`** folder: thin **`index.html`**, **`app.js`**, **`crepus-bundle.json`** (all `*.crepus` sources), **`vendor/unocss.js`**, and **`pkg/runtime.js`** + **`runtime_bg.wasm`**. Copy and dynamic data live in **`.crepus`** (quoted text nodes) and, when you need typed Rust context, in **`runtime/src/lib.rs`** via **`crepuscularity_web::render_from_files`** (same pattern as extension runtimes calling into `crepuscularity-web`). The default web build is debug; use `--release` for release-profile WASM and post-build optimization.

### Prerequisites

- `rustup target add wasm32-unknown-unknown`
- `cargo install wasm-bindgen-cli`

If both a system package manager and **rustup** ship `cargo`/`rustc`, ensure rustup’s toolchain wins for WASM (the CLI prepends `~/.cargo/bin` to `PATH` for nested builds).

### `crepus web new <name>`

Scaffolds `index.crepus`, `crepus.toml`, and `runtime/` (thin `#[wasm_bindgen]` shim that calls **`crepuscularity_web::render_bundle`**).

### `crepus web build`

```bash
crepus web build --site ./my-site --out-dir ./dist
```

`crepus.toml` is the source of truth for web target metadata, including SEO. The build writes title, description, canonical, robots, keywords, author, theme color, Open Graph, Twitter card, alternate links, JSON-LD, and optionally `robots.txt` / `sitemap.xml`.

```toml
[[targets]]
type = "web"
id = "site"
site = "."
out = "dist"
entry = "index.crepus"
name = "Acme Labs"
description = "A concise fallback description."

[targets.seo]
title = "Acme Labs"
description = "A search and social description for Acme Labs."
canonical = "https://example.com/"
site_name = "Acme Labs"
locale = "en_US"
type = "website"
image = "https://example.com/og.png"
image_alt = "Acme Labs dashboard preview"
keywords = ["rust", "wasm", "ui"]
author = "Acme Labs"
robots = "index,follow"
theme_color = "#101820"
application_name = "Acme Labs"
generator = "crepuscularity"
twitter_card = "summary_large_image"
twitter_site = "@acme"
twitter_creator = "@acme"
json_ld = [
  """{"@context":"https://schema.org","@type":"WebSite","name":"Acme Labs","url":"https://example.com/"}"""
]

[[targets.seo.alternates]]
href = "https://example.com/fr/"
hreflang = "fr"

[targets.seo.robots_txt]
enabled = true
allow = ["/"]
disallow = ["/internal"]
sitemap = "https://example.com/sitemap.xml"

[targets.seo.sitemap]
enabled = true
base_url = "https://example.com"
paths = ["/", "/docs/"]
changefreq = "weekly"
priority = 0.8
```

Optional **`site.json`** remains accepted for older sites that already use `seo.title`, `seo.description`, `ogImage`, and theme CSS variables. New projects should keep metadata in `crepus.toml`.

### Interactivity (Svelte-style flexibility)

First paint is **`.crepus` → WASM** (same as `crepus web serve`). You can still add **fine-grained reactivity** the same way as on the web in general:

- **Client islands** — `embed ./islands/wave.ts title={title}` compiles the island entry with Bun, writes `dist/islands/*.js`, and mounts the module's `mount(el, props, ctx)` export after the `.crepus` shell renders. Shader imports work through Bun's browser bundler; use import attributes such as `import src from "./wave.glsl" with { type: "text" }` when the shader source should be bundled as a string.
- **Reactive graph** — the **`crepuscularity-reactive`** crate is the direction for signal-driven DOM updates; compose with WASM templates as “compiled shell + targeted patches”.
- **Hydration / islands** — described in **`docs/CREPUS_WEB_IMPLEMENTATION_SPEC.md`**; goal is optional client graphs without duplicating template semantics.

So: **flexible** — static SSR-like WASM output by default, **opt-in** client reactivity where you want it.

### DOM refs and events

Enable DOM helpers in the site runtime when you want typed `#id` access:

```toml
[dependencies]
crepuscularity-web = { path = "../../crates/crepuscularity-web", features = ["dom"] }
wasm-bindgen = "0.2"
```

Template:

```text
div #hero "Hi"
button @click="on_refresh_status" type="button"
  "Refresh"
```

Runtime:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn on_refresh_status() -> Result<(), JsValue> {
    let crepus = crepuscularity_web::crepus_refs!("../index.crepus");
    crepus.hero.text("Bye")
}
```

`crepus_refs!` scans the referenced `.crepus` file and reachable `include`s at compile time, then generates typed fields for discovered `#id`s. Missing DOM nodes return `Result::Err`; the web production path does not panic.

For Cargo rebuild tracking and early syntax validation, add a build dependency on the same crate and put this in `build.rs`:

```rust
fn main() {
    crepuscularity_web::build::compile_crepus("views").unwrap();
}
```

### Optional web features

- `dom`: wasm-side DOM lookup and mutation helpers such as `crepus.hero.text(...)`.

Keep minimal sites on default features and opt in only when the runtime needs DOM mutations.

### HTMX and Alpine in the shell

`crepus web build` copies `static/` into `dist/static/`. To vendor HTMX or Alpine without blocking WASM-first paint, add the script file under `static/vendor/` and inject it from `crepus.toml`:

```toml
[[targets]]
type = "web"
site = "."
entry = "index.crepus"
head_html = """
  <script defer src="./static/vendor/htmx.min.js"></script>
  <script defer src="./static/vendor/alpine.min.js"></script>
"""
```

HTMX and Alpine should target stable subtrees that your Rust/WASM code is not replacing wholesale. Practical rule:

- Let WASM own `#hero`, `#status`, and other nodes mutated through `crepus_refs!`.
- Point HTMX swaps at sibling containers or leaf regions that Rust is not patching.
- Use Alpine for local state inside self-contained islands; avoid mixing Alpine `x-text` and Rust `text(...)` writes on the same node unless one side is clearly authoritative.

Canonical Alpine coexistence:

```text
div x-data="{ n: 0 }"
  span #counter_display x-text="n"
    "0"
  button x-on:click="n++" type="button"
    "++"
```

Use `x-on:*` for Alpine inside `.crepus`; Crepus reserves `@event=...` for Rust/WASM handler wiring.

### `crepus web dev`

Dev server with hot reload (also available as `crepus web serve`). See `crepus web --help`.

### `crepus web site-json`

Deprecated pretty-printer for `site.json`.

## iOS host apps (`crepus ios`)

Scaffold an **XcodeGen** + **SwiftPM** app with a local **`NativeShell`** package that renders **View IR** JSON (same contract as **`crepuscularity-native`**). Prerequisite: [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`wax install xcodegen`).

```bash
crepus ios new my-native-demo
cd my-native-demo
crepus ios generate   # run xcodegen (walks up until it finds crepus.toml [ios])
open *.xcodeproj
```

Build for the iOS Simulator from the app tree (after generate):

```bash
crepus ios build
```

`crepus ios generate` and `crepus ios build` **walk up** from the current directory until they find **`crepus.toml`** with an **`[ios]`** section — you normally do not need `--dir` or `--scheme`. Optional overrides: `crepus ios build --dir . --scheme Foo --destination 'platform=iOS Simulator,name=iPhone 15'`.

To refresh bundled fixtures after template changes, regenerate IR with **`crepuscularity-native`** (`render_template_to_ir` / `to_json_pretty`) and replace `NativeShell/Sources/NativeShell/fixture.json`. For `crepus native new` scaffolds, `crepus native sync views/main.crepus --dir . --out desktop/share/dashboard.view-ir.json --var name=Ada --pretty` writes the same View IR JSON into the root, iOS, Android, and explicit host fixture locations. Add `--no-defaults` when only the explicit `--out` files should be written. See [`examples/native-shells/README.md`](../examples/native-shells/README.md).

## Examples

The example catalog lives in [`examples/examples.toml`](../examples/examples.toml), grouped by output target. External browser extensions ([undivisible/anywhere](https://github.com/undivisible/anywhere), [undivisible/rs_vimium](https://github.com/undivisible/rs_vimium)) are symlinked under `examples/advanced/extensions/` — see [`examples/README.md`](../examples/README.md) and `./scripts/examples-link.sh`.

## Mobile Apps (`crepus mobile`)

Use `crepus mobile` as the unified iOS + Android workflow:

```bash
crepus mobile new my-mobile-app
cd my-mobile-app
crepus mobile dev --platform all --port 4001
crepus mobile sync --var name=Ada --pretty
crepus mobile codegen --platform ios --out ios/Sources/NativeShell/Generated --view-name DashboardView
crepus mobile codegen --platform android --out android/app/src/main/java/dev/example/generated --view-name DashboardView
crepus mobile build --platform ios
crepus mobile run --platform ios
crepus mobile build --platform android
crepus mobile run --platform android
```

`crepus mobile build` and `crepus mobile run` refresh `fixture.json` and generated SwiftUI/Compose source before invoking `xcodebuild`, Gradle, `simctl`, or `adb`. `crepus mobile dev` watches `views/main.crepus` by default, writes current fixtures/generated source, and serves:

```text
GET /health
GET /ir
GET /events
```

The scaffolded SwiftUI and Compose runtimes load bundled `fixture.json` first, then poll the local dev server in debug builds.

## Native View IR and Source (`crepus native`)

`crepus native new` creates a thin SwiftUI/Compose shell with View IR and its app-local Rust bridge; it ships no Bluetooth Java or Bluetooth permissions. Add capabilities only where needed: `crepus native add sensors --dir .` or `crepus native add bluetooth --dir .`. Bluetooth installs Android and iOS central scan bridges with `status`, `requestPermission`, `scan`, and `stopScan` operations.

Emit the same **View IR** JSON contract used by native shells and polyglot plugins:

```bash
crepus native ir views/main.crepus --ctx context.json --pretty
crepus native ir views/ui.crepus --component Card --var title=Hello
cat views/main.crepus | crepus native ir --stdin --base-dir views
crepus native sync views/main.crepus --dir my-mobile-app --out desktop/share/dashboard.view-ir.json --no-defaults --var name=Ada --pretty
crepus native codegen views/main.crepus --platform swiftui --out Generated --view-name DashboardView --var name=Ada
crepus native codegen views/main.crepus --platform compose --out android/app/src/main/java/dev/example/generated --view-name DashboardView --var name=Ada
```

For tool integrations, `crepus native ir --stdin-json` accepts an envelope with `entry`, `files`, `template`, `context`, and `pretty`. Successful output is JSON on stdout only; failures are JSON on stderr. See [Polyglot plugins](polyglot.md).

`crepus native codegen` lowers through the same View IR and writes generated SwiftUI (`.swift`) or Jetpack Compose (`.kt`) source for production host apps.

## Embedded framebuffer (`crepus embedded`) — UNSTABLE

> **Still in development; largely untested on real hardware.** Prefer host `cargo test` / `crepus embedded snapshot` for validation. Physical display wiring is experimental — see [Embedded / framebuffer](embedded.md). CLI commands are for CI and debug snapshots, not a supported production deploy path yet.

```bash
crepus embedded check ui/dashboard.crepus
crepus embedded snapshot ui/dashboard.crepus --width 240 --height 320 --out /tmp/preview.ppm
crepus embedded snapshot ui.crepus --component Card --ctx context.json --var cpu=88 --width 128 --height 64 --out card.ppm
```

- **`check`** — parse validation (use in CI alongside `crepuscularity_core::build::compile_crepus` in `build.rs`).
- **`snapshot`** — writes RGB888 PPM (P6) for visual inspection only, not for shipping to devices.

See [Embedded / framebuffer](embedded.md).

## Aurorality — SwiftUI for macOS/iOS (`crepus aurora`)

Aurorality targets SwiftUI apps from `.crepus` templates. Requires the `aurorality` binary (`cargo install aurorality-cli`).

```bash
crepus aurora dev --watch views           # hot-reload dev server (WebSocket)
crepus aurora dev --watch views --port 8080
crepus aurora build --watch views --out generated/ir
crepus aurora new my-aurora-project
crepus aurora swiftgen --view main.crepus --out Generated --view-name AppView
crepus aurora bundle --views views --out generated/ir --js-entry plugins/main.js --js-out bundle/main.js
crepus aurora bindgen --input plugins --output generated
```

All arguments are forwarded to `aurorality`. Run `aurorality --help` for full options.

## Browser Extension Commands

### `crepus webext new <name>`

Scaffold a new browser extension:

```bash
crepus webext new my-extension
cd my-extension
crepus build
```

Creates:
- `crepus.toml` — extension target and configuration
- `runtime/` — Rust WASM runtime crate
- `views/` — `.crepus` templates

### `crepus webext build`

Build the extension to `dist/unpacked/`:

```bash
crepus webext build
crepus webext build --app ./path/to/extension
```

### `crepus webext manifest`

Print the generated `manifest.json`:

```bash
crepus webext manifest
crepus webext manifest --app ./path/to/extension
```

## IDE Integration

The `--emit-events` flag outputs structured JSON events for IDE integration:

```bash
crepus dev --emit-events
```

Events:
- `CompilationStarted` — build started
- `CompilationSuccess` — build succeeded with timing
- `CompilationError` — build failed with diagnostics

Example output:

```json
{"event":"CompilationStarted","timestamp":"2024-01-15T10:30:00Z"}
{"event":"CompilationSuccess","timestamp":"2024-01-15T10:30:05Z","duration_ms":5123}
{"event":"CompilationError","timestamp":"2024-01-15T10:31:00Z","errors":[{"message":"..."}]}
```

## Environment

On macOS, GPUI requires the Xcode SDK path and, on Xcode installs with a separate Metal Toolchain component, the matching toolchain selector:

```bash
export SDKROOT=$(xcrun --show-sdk-path)
export DEVELOPER_DIR=$(xcode-select -p)
export TOOLCHAINS=Metal
```

From the repository checkout, use `eval "$(scripts/metal-env.sh)"` to export those values and prepend the downloaded `Metal.xctoolchain/usr/bin` to `PATH` for direct `metal` / `metallib` checks. `TOOLCHAINS` is the environment variable `xcrun` reads when GPUI's build script runs `xcrun -sdk macosx metal`; on current Xcode installs the working selector is the short value `Metal`. Use `scripts/metal-env.sh --check` to verify local state without network access.
