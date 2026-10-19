# Crepuscularity Lite

**Also:** [Documentation home](README.md) · [Runtime and reactivity](runtime.md) · [GPUI](gpui.md) · [TUI](tui.md)

`crepuscularity-lite` is the desktop shell layer for apps that want GPUI windows, Rust-native capabilities, and an embedded JavaScript/TypeScript guest runtime without building a full web stack. It embeds V8 through the official Rust `v8` crate and exposes a native-capability bridge so guest code can call registered Rust plugins.

Lite now lives in this repository at `crates/crepuscularity-lite`. Use the in-tree crate while developing the full Crepuscularity workspace, and use the published crate for downstream apps.

## Install

```toml
[dependencies]
crepuscularity-lite = "0.4.16"
```

The crate builds V8 and GPUI. On macOS, use the Metal helper when Cargo cannot find Xcode's downloaded Metal toolchain:

```bash
scripts/metal-env.sh -- cargo check -p crepuscularity-lite
scripts/metal-env.sh -- cargo run -p crepuscularity-lite --features cli --bin cl -- --help
```

## Runtime Model

The shell has three layers:

| Layer | Responsibility |
| --- | --- |
| GPUI host | Owns windows, app lifecycle, keyboard/mouse events, and UI-thread work |
| V8 host | Evaluates guest JavaScript, installs `Crepus.invoke`, and keeps the bridge in the V8 context |
| Rust bridge | Registers native plugins, checks capabilities, serializes requests/responses, and queues deferred host actions |

Guest code is not allowed to mutate GPUI directly. Plugins return data or `HostDeferred` commands such as setting the window title; the host applies those commands on the UI thread.

## Bridge API

Register plugins on a `Bridge`, then pass the bridge into `V8Host`.

```rust
use std::sync::Arc;
use crepuscularity_lite::{Bridge, ClipboardPlugin, CorePlugin, V8Host};

let mut bridge = Bridge::default();
bridge.register(CorePlugin::default());
bridge.register(ClipboardPlugin::default());

let bridge = Arc::new(bridge);
let mut host = V8Host::new(bridge)?;
let result = host.eval(r#"Crepus.invoke("core.echo", { "message": "hello" })"#)?;
```

Use the capability system for file, clipboard, download, window, host, app, bench, and inauguration operations. Lite reads `crepus.toml` by default, falls back to legacy `crepus-lite.toml`, and grants only `core` plus `app` unless optional native capabilities are explicitly enabled.

### `inauguration` plugin (opt-in)

Plugin id **`inauguration`** bridges guest JS to the **`in`** CLI on `PATH` (no link to the `inauguration` Rust crate). Enable it for docs-site / polyglot workflows that shell out to `in execute`, `in build`, or patch files from guest code.

```toml
[capabilities]
inauguration = true
```

```js
const raw = JSON.parse(
  Crepus.invoke(
    "inauguration",
    "processRun",
    JSON.stringify({ command: "in test" })
  )
);
// raw.data: { exitCode, stdout, stderr, success }
```

Methods: `ping`, `whichIn`, `processRun`, `readFile`, `writeFile`. The [inauguration](https://github.com/tschk/inauguration) repo also ships `in plugin run crepuscularity` for `docs-site/build.in` when building its public site.

## Guest Code

Development builds can run TypeScript and TSX through the built-in Oxc transpiler. Production bundling, chunking, and code splitting remain the embedder's responsibility; the `cl` binary and `CrepusLiteConfig` support prelude scripts, worker scripts, and guest file watching.

```toml
[guest]
entry = "src/main.ts"
prelude = ["src/prelude.ts"]

[capabilities]
clipboard = true
window = true
```

## Threads and Workers

`V8Host` is isolate-local. Use `V8ThreadRuntime` when guest work must live off the UI thread, and use `WorkerRuntime` for worker-style guest scripts. Do not run GPUI window mutations directly from worker threads; queue host commands and apply them from the GPUI side.

## Current Scope

`crepuscularity-lite` is an application shell and bridge runtime, not a replacement for the `.crepus` parser or renderers. Use it when you need an embedded guest runtime inside a native desktop host. Use `crepuscularity-gpui` for direct GPUI template rendering, `crepuscularity-tui` for terminal UIs, and `crepuscularity-web` / `crepuscularity-webext` for HTML and extension targets.
