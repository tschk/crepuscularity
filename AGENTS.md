# Crepuscularity

Write once. Target desktop, terminal, web, browser extensions, mobile, and embedded.

## Project Structure

```
crepuscularity/
  crates/
    crepuscularity/          — manifest/target build (crepus.toml)
    crepuscularity-cli/      — `crepus` CLI (main entrypoint)
    crepuscularity-core/     — parser (6 frontends), eval, AST, context, error types
    crepuscularity-components/ — Rust component catalog registry (CLI)
    crepuscularity-web/      — HTML/WASM rendering (web + SSR)
    crepuscularity-webext/   — browser extension builds (MV3)
    crepuscularity-tui/      — Ratatui terminal rendering
    crepuscularity-native/   — View IR for iOS/Android native
    crepuscularity-embedded/ — RGB565 framebuffer for SPI/LTDC panels
    crepuscularity-lvgl/     — LVGL XML generation
    crepuscularity-gpui/     — GPUI desktop rendering
    crepuscularity-runtime/  — hot-reload + shared runtime
    crepuscularity-lite/     — V8-based JS runtime
    crepuscularity_macros/   — proc macros (note the underscore)
    crepuscularity-reactive/ — WASM signals, memos, effects, hydration
    crepuscularity-abi/      — C ABI sessions over the View IR
    crepuscularity-wasm/     — WASM parser + `@tschk/crepuscularity-wasm` npm package
    crepuscularity-lsp/      — language server
    crepuscularity-tauri/    — Tauri host integration
    crepuscularity-tauri-macros/ — proc macros for the Tauri host
    crepuscularity-plugin-bindgen/ — plugin binding generation
  plugins/
    crepuscularity-flutter/      — Flutter View IR / .crepus renderer
    crepuscularity-components/   — Flutter/Svelte packages + catalog source (omi path deps)
  examples/
    showcase/                — small single-file DSL demos (render target)
    web-site/                — reference site: index.crepus + runtime/
    quicknote/               — in-repo MV3 browser extension
    weather/                 — GPUI desktop app
    ui-library/              — reusable component catalog demos
    native-shells/           — SwiftUI/XcodeGen + Gradle View IR hosts
    embedded-dashboard/      — host-sim RGB565 panel
    advanced/                — benchmarks, SSR, embedded boards, LVGL, mobile, Tauri
    examples.toml            — catalog by target (see examples/README.md)
```

## Parser Frontends

`crepuscularity-core` has six frontends under `src/parser/`, dispatched by file
extension in `parse_template_with_path`, all producing the same `ast::Node` tree:

| Extension | Frontend | Module |
|-----------|----------|--------|
| `.vue` | Vue SFC | `parser/vue/` |
| `.svelte` | Svelte | `parser/svelte/` |
| `.astro` | Astro | `parser/astro/` |
| `.component.html`, `.ng.html`, `.ng` | Angular | `parser/angular/` |
| `.csx` `.jsx` `.tsx` (or first line starts with `<`) | JSX | `parser/jsx/` |
| anything else | indentation | `parser/indent/` |

- Svelte, Vue, Astro, and Angular are **first-party Rust**; no `svelte`/`vue`/`astro`/`@angular` crate dependency. Do not add one.
- They compile the **template only**. `<script>` / frontmatter / component classes are extracted verbatim and never executed — runes, stores, Composition API, `Astro.props`, `@Input`/`@Output`, and lifecycle do not run.
- Unsupported markup constructs must be **hard parse errors**, never silent drops. Keep it that way when extending them.
- `parseTemplate(source, filename)` in `@tschk/crepuscularity-wasm` is the one JS entry point for all six; the filename selects the frontend.
- Full support matrix: [`docs/frontends.md`](docs/frontends.md).

## Targets & Conventions

### `crepus web` — Static sites + WASM runtime

- Scaffold: `crepus web new <name>` → `index.crepus` + `runtime/` + `crepus.toml`
- Runtime entry: `runtime/src/lib.rs` exports `crepus_render(bundle_json: &str)`
- WASM bridge: `crepuscularity_web::render_bundle(bundle_json).map_err(|e| JsValue::from_str(&e.to_string()))`
  - **ALWAYS use `.to_string()`** on the error — `CrepusError` is not `&str`
- Template syntax: indent-based, UnoCSS classes, `slot-rotate`, `embed` islands
- Build output: `dist/` with `index.html` + `app.js` + `pkg/` (WASM) + `crepus-bundle.json`
- Dev server: `crepus web dev --site . --port 4000`
- WASM target: `wasm32-unknown-unknown`
- Required deps: `crepuscularity-web`, `wasm-bindgen`

### `crepus webext` — Browser extensions (MV3)

- Scaffold: `crepus webext new <name>` → `app/` + `runtime/`
- Supports: `chromium`, `firefox`, `safari`
- WASM-powered background/service workers + content scripts
- Uses `crepuscularity-webext` for manifest generation and WASM bridge
- Build: `crepus webext build --browser chromium --release`
- Dev: `crepus webext dev` (watch + auto-reload)

### `crepus tui` — Terminal apps (Ratatui)

- Scaffold: `crepus tui new <name>` → `app.crepus` + `Cargo.toml` + `src/main.rs`
- Rendering: `crepuscularity-tui` with `HotTemplate` for hot-reload
- Template vars: `set("key", "value")` method
- Preview: `crepus tui preview app.crepus` (q/Esc to quit)
- No WASM — native binary

### `crepus native` — iOS + Android (View IR)

- Scaffold: `crepus native new <name>` → iOS (SwiftPM) + Android (Gradle)
- Rendering: `crepuscularity-native` emits View IR JSON
- Synced view tree: `crepus native sync` updates shared `fixture.json`
- Build: `crepus native build ios|android`
- Preview IR: `crepus native ir file.crepus`

### `crepus ios` — XcodeGen + SwiftUI shells

- Scaffold: `crepus ios new <name>` → XcodeGen project + NativeShell SwiftPM
- View IR rendered via SwiftUI adapter
- Requires: `xcodegen` (`brew install xcodegen`)
- Build: `crepus ios build` (runs xcodegen + xcodebuild)

### `crepus embedded` — LVGL firmware

- Commands: `check` (validate template), `snapshot` (render PPM for debug)
- Rendering: `crepuscularity-embedded` with `crepuscularity-lvgl`
- Target: embedded MCUs (STM32, ESP32) via LVGL
- XML output for Screen/Component root types

### `crepus aurora` — SwiftUI (via Aurorality CLI)

- Delegates to `aurorality` CLI (`cargo install aurorality-cli`)
- Commands: `dev`, `build`, `new`, `swiftgen`
- Generates SwiftUI views from `.crepus` templates

### `crepus new` / `crepus init` — GPUI desktop apps

- `crepus new <name>` — scaffolds a GPUI desktop app
- `crepus init <kind> <name>` — same as `crepus <kind> new <name>`
- GPUI apps use `view! {}` macro with `.crepus` string templates

### `crepus moonshine` — Moonshine + Crepus web apps

**Moonshine is a separate product:** [`github.com/tschk/moonshine`](https://github.com/tschk/moonshine) (`@tschk/moonshine`; local checkout often `~/projects/moonshine`). Prefer the `moonshine` CLI from that repo when available; `crepus moonshine` remains a working Crepuscularity fallback. Crepuscularity compiles `.crepus` → View IR and emits a TSX file that imports `@tschk/moonshine/react`.

- Scaffold: `crepus moonshine new <name>` → Vite shell + `index.crepus` + `package.json` (imports `@tschk/moonshine/react`)
- Dep snippets: `crepus moonshine dep` →
  - `@tschk/moonshine` → `github:tschk/moonshine#path:packages/core`
  - `@tschk/crepus-moonshine` → `github:tschk/moonshine#path:packages/crepus-moonshine`
  - `@tschk/moonshine-components` → `github:tschk/moonshine#path:components`
- Emit: `crepus web build --emit moonshine --site .` → `dist/crepus-emit.moonshine.tsx` (`renderCrepusIr` + `createApp` from `@tschk/moonshine/react`) + `crepus-view-ir.json`
- React component implementations: `moonshine/components/` as `@tschk/moonshine-components` (not in this repo)

### `crepus components` — shared UI catalog (`crepuscularity-components`)

- Rust registry: `crates/crepuscularity-components` (embedded `catalog/components.json`; sync from plugin)
- Plugin source (Flutter/Svelte packages kept for omi path deps): `plugins/crepuscularity-components/`
- `crepus components list` — list component ids (crate first, filesystem fallback)
- `crepus components add <id> [--target flutter|svelte|moonshine|gpui]` — path hints / install guidance
- `crepus components themes` — theme names (crate first, then `catalog/themes/`)

### `crepus web build --emit`

- `--emit html` (default) — existing WASM site build (`index.html` + `pkg/`)
- `--emit moonshine` — real TSX emit (`crepus-emit.moonshine.tsx`): literal JSX with `className` from `ViewStyle.classes`, importing only `createApp` from `@tschk/moonshine/react`

`WebEmitTarget` has exactly these two variants (`crates/crepuscularity-cli/src/cli.rs`). The former `--emit svelte|solid|react` stubs were removed as non-functional.

### `crepus flutter` — Flutter renderer dependency helper

- Package: `plugins/crepuscularity-flutter` (pub name `crepuscularity_flutter`)
- `crepus flutter dep` / `crepus flutter add` — print or insert pubspec dependency block

## Core CLI

| Command | Purpose |
|---------|---------|
| `crepus new <name>` | Scaffold GPUI app |
| `crepus init <kind> <name>` | Scaffold any target |
| `crepus dev [--bin]` | Hot-reload dev loop |
| `crepus build [--target]` | Build crepus.toml targets |
| `crepus preview <file>` | Live-preview (GPUI) |
| `crepus render <file>` | Render to HTML stdout |
| `crepus components …` | Shared component catalog |
| `crepus moonshine …` | Moonshine scaffold + deps |
| `crepus flutter …` | Flutter renderer deps |
| `crepus web build --emit …` | `html` (default) or `moonshine` TSX |

## Common Error Patterns

- `CrepusError` → String: always use `.to_string()` when converting to `JsValue::from_str`
- WASM builds need `--target wasm32-unknown-unknown` and `wasm-bindgen-cli`
- Template syntax: indentation-based, 2-space indents, shared root element class on same line as tag