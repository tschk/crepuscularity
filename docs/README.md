# Documentation

Guides and references for the `.crepus` DSL, tooling, and browser extensions.

| Guide | Description |
| --- | --- |
| [DSL reference](dsl.md) | Syntax, control flow, attributes, animations, SwiftUI semantic tags |
| [Input frontends](frontends.md) | `.crepus`, JSX/TSX, Svelte, Vue, Astro, Angular → one AST; support and limits |
| [Components](components.md) | `include`, slots, defaults, multi-component files |
| [CLI](cli.md) | `crepus` commands for apps, web, and extensions |
| [Changelog](changelog.md) | Security and crate releases |
| [Production readiness](production.md) | build gates, security boundaries, and performance checks |
| [Observability](observability.md) | timing, tracing, and flamegraph workflow |
| [Timing profile](timing-profile.md) | SSR request breakdown, flamegraph recipes |
| [Runtime and reactivity](runtime.md) | state model, update lifecycle, hydration, and Metal setup |
| [Browser extensions](webext.md) | Manifest V3 apps with `crepus webext` |
| [Extension policy (MV3)](webext-policy.md) | CSP safety, store submission, permissions checklist |
| [GPUI integration](gpui.md) | Desktop apps with GPUI |
| [GPUI source inventory](reviews/gpui-inventory-2026-11-18.md) | Compiled templates, runtime previews, Kit adapter, and verification limits |
| [ZPUI integration](zpui.md) | Experimental Zig adapter over View IR v7 |
| [Lite shell](lite.md) | GPUI + V8 desktop shell and Rust plugin bridge |
| [Native shells](native.md) | iOS/Android apps via native UI frameworks |
| [Tauri native compatibility](tauri-compatibility.md) | Tauri replacement contract and audit matrix |
| [React Native adaptation](react-native-adaptation.md) | Source-level React Native migration boundary |
| [Polyglot plugins](polyglot.md) | Overview: View IR JSON, CLI, optional ABI |
| [View IR contract](view-ir-contract.md) | Stable JSON boundary, schema, CLI envelopes, hot reload |
| [Plugin surface](plugin-surface.md) | Required capabilities for language packages |
| [Aurorality](aurorality.md) | SwiftUI engine and semantic native tag workflow |
| [IDE extensions](ide-extensions.md) | Editor integration sketch (CLI tasks, `aurorality dev`, diagnostics) |
| [TUI mode](tui.md) | Terminal user interfaces |
| [Embedded / framebuffer](embedded.md) | **UNSTABLE** — RGB565 `Ui`; in development, largely untested on hardware |
| [LVGL](lvgl.md) | LVGL Pro XML generator for panels and embedded UI workflows |

Implementation details for compilers and agents live in-repo at [`CREPUS_WEB_IMPLEMENTATION_SPEC.md`](CREPUS_WEB_IMPLEMENTATION_SPEC.md) (not published on the docs site).

**Published HTML**

`crepus web build --manifest docs-site/crepus.toml` builds the landing page and
the configured LLM documentation sources. The current manifest does not configure
a Markdown-to-HTML renderer; the former `docs-renderer` binary was removed.
The Pages workflow checks the landing page and WASM artifacts, not individual
guide pages. Markdown changes in this directory alone do not establish that the
corresponding `/docs/*.html` pages have been rebuilt or published.
