# Crepuscularity Lite

**Also:** [Documentation home](README.md) · [Runtime and reactivity](runtime.md) · [GPUI](gpui.md) · [TUI](tui.md)

`crepuscularity-lite` is the desktop shell layer for apps that want GPUI windows, Rust-native capabilities, and an embedded JavaScript/TypeScript guest runtime without building a full web stack. It embeds V8 through the official Rust `v8` crate and exposes a native-capability bridge so guest code can call registered Rust plugins.

Lite now lives in this repository at `crates/crepuscularity-lite`. Use the in-tree crate while developing the full Crepuscularity workspace, and use the published crate for downstream apps.

## Install

```toml
[dependencies]
crepuscularity-lite = "0.4.3"
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

## Host trees in the GPUI demo

Foreground redraws acquire an immutable shared tree through
`Bridge::host_render_tree` / `HostState::render_snapshot`. Repeated reads share the
same normalized tree until the guest replaces it; retained readers keep their
previous version. The full `HostSnapshot` API still returns an owned tree and
storage/navigation metadata. Element construction remains on the foreground
thread, and snapshot acquisition still uses the host mutex.

The `host.renderTree` bridge method accepts a `tree` object containing `type`,
optional `id`, `className` (or `class`), `style`, and `children`. The demo renderer
handles `View` containers, `ScrollView`, `Button`, `Text`, and a display-only
`TextInput`; the latter shows its value or placeholder and dispatches handlers,
but does not implement text editing.

Style keys use camelCase. Explicit fields take precedence over utility classes;
within a class list, the last recognized utility wins. Button and input defaults
are applied before explicit host styles. Unknown utilities are ignored.

| Fields | Meaning |
| --- | --- |
| `width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `maxHeight` | Pixel dimensions; width/height `-2` mean size to content |
| `widthFraction`, `heightFraction` | Parent proportions: `1` fills, `0.5` uses half; pixel dimensions take precedence |
| `padding`, `paddingX`, `paddingY`, `gap`, `radius`, `borderWidth` | Finite nonnegative pixels, including fractions |
| `display`, `direction`, `alignItems`, `justifyContent` | Flex/block/grid/none display, row/column direction, alignment and justification |
| `gridCols`, `gridRows`, `flexGrow` | Positive grid track counts and finite nonnegative flex growth |
| `background`, `color`, `borderColor`, `opacity`, `shadow` | CSS hex colors with trailing alpha, clamped opacity, and named GPUI shadow presets |
| `fontSize`, `fontWeight`, `fontFamily`, `textAlign`, `italic`, `underline`, `strikethrough` | Text styling; font size must be positive and font families must be installed |
| `overflow`, `overflowX`, `overflowY` | Visible, hidden, clip, scroll, or auto; axis values override the general value |

Utilities include fractional spacing and bracketed pixels (`p-1.5`,
`gap-[2.5px]`, `w-[100.5px]`, `text-[18.5px]`), proportions (`w-full`, `h-1/2`),
CSS hex colors (`bg-[#1234]`), palette opacity (`text-red-500/50`), grid counts,
named font weights, text decorations, and named shadows. `opacity-25` means
`0.25`; finite percentage values are clamped. `auto` overflow uses GPUI scrolling.
Scroll views default to columns and scrolling on both axes, while honoring
configured layout and overflow. Hidden nodes skip children and handler attachment.

Handler fields remain `on_press` and `on_key_down`. Guests supply
`globalThis.__crepusHostDispatch(handlerId, payload)`. Press/focus payloads carry
`source` and `nodeId`; keyboard payloads carry `kind: "keyDown"`, `nodeId`, and
GPUI's formatted `keystroke`. Nodes with keyboard handlers are focusable.
Fallback IDs use tree positions; assign explicit IDs to reorderable children.
GPUI event simulations exercise shared-handler buttons, hidden siblings, focused
keyboard delivery, grid placement, and scroll movement. Physical-window behavior
still needs validation.

## Filesystem listing

The `fs.list` method accepts an object with an optional string `path` relative to
the filesystem sandbox. Omitting `path` or using `""` lists the sandbox root;
non-string paths and non-object payloads return `invalid_payload`.

## Downloads

Enable the `download` capability to use `download.addUri`, `list`, `pause`,
`resume`, and `remove`. `addUri` accepts a valid HTTP(S) `url` and an optional
`filename`. Without a filename, the URL's final nonempty path component is used;
query strings and fragments are excluded, and root URLs use `download.bin`.
Names have path separators and NUL characters replaced with underscores. Task
outputs live under the OS downloads directory in `crepuscularity-lite/downloads`.

Pause preserves saved bytes. Resume uses the saved file length and validates
partial-response offsets and body lengths; short ranges trigger further requests.
Strong ETags are sent as `If-Range`, and a full replacement response restarts the
file. Servers without strong ETags provide no resource-version protection.
Repeated resume calls reuse the active worker. Completed and removed tasks reject
pause and resume. Remove deletes the task's output, including paused or completed files.

Cancellation is cooperative: blocked requests and reads can wait for data or the
60-second request timeout. Task snapshots report saved byte counts, status, speed,
and errors. This runtime keeps tasks in memory; it does not recover them after an
application restart.

## Threads and Workers

The window bridge accepts `requestDecorations` with `style: "client"` or
`style: "server"`; omitting `style` defaults to server decorations. Other values
return `invalid_payload` without scheduling a change. `setContentSize` requires
positive logical-pixel dimensions that remain finite and nonzero after conversion
to the host's size representation.

`V8Host` is isolate-local. Use `V8ThreadRuntime` when guest work must live off the UI thread, and use `WorkerRuntime` for worker-style guest scripts. Do not run GPUI window mutations directly from worker threads; queue host commands and apply them from the GPUI side.

`WorkerHandle::post_message` delivers parsed JSON as the guest message event's
`data`. Keys such as `__proto__` remain ordinary data properties. Delivery uses a
JSON parser captured before guest entry, so replacing the guest's `JSON` global
does not change host-message decoding.

Compute-worker `eval` and `post_message` wait synchronously for guest work to
finish. Dropping `WorkerRuntime` also joins its thread and waits for a running
entry script. Keep these calls and teardown off the UI thread; a nonreturning
guest can prevent them from completing. The GPUI host's awaitable evaluation and
abort path described below use `V8ThreadRuntime` instead.

The GPUI host discovers and parses its configuration and constructs its bridge
on a dedicated startup thread. It opens the window with that configuration after
the startup result arrives, keeping configuration I/O off the UI event loop.
Canceling startup delivery disposes the produced root rather than retaining it.
V8 isolate initialization still happens asynchronously on its own worker; root
construction confirms thread creation, not successful isolate initialization.

`V8ThreadHandle::eval_rpc_async` returns an awaitable reply; the GPUI host uses it
to yield while guest evaluation runs. Both reply APIs share one FIFO and isolate.
The host also queues guest-file and prelude reads and Oxc transpilation on that
worker, keeping preparation and evaluation in the same FIFO entry so later UI
events cannot overtake a load. `eval_prepared_rpc_async` provides this path for
trusted source preparation. Preparation errors leave the current isolate intact.
Abort prevents prepared code from evaluating, but synchronous reads or compiler
work already in progress must return before the worker can finish teardown.

The legacy `eval_rpc` receiver blocks when read and must not be waited on from
the UI thread. Reset drops the previous isolate before creating its replacement.
If creation fails, non-reset evaluations return an unavailable error until a
later reset succeeds; previous guest globals are lost.

Guest-file watching follows the configured logical path through file and parent
directory symlink retargets. Core's managed watcher reconciles its nonrecursive
parent subscriptions in the background; unrelated neighboring files do not trigger
reloads. Callbacks still run on the existing debouncer thread and must schedule
UI work themselves. Debouncing can emit continuous and final notifications, and
retarget reconciliation can produce a later notification. The watcher remains
alive for the process lifetime.

`shutdown()` and ordinary `V8ThreadRuntime` drop remain graceful: queued requests
finish before the worker exits, and drop joins the worker. For UI teardown,
`V8ThreadRuntime::abort()` interrupts JavaScript and discards queued work. It
returns the worker join handle for completion tracking away from the UI thread;
repeated calls return `None`. The GPUI host uses this path when its root closes,
joining on a separate thread instead of blocking the UI or its executor pool.

An already-running native bridge callback cannot be forcibly interrupted. Its
native operations can continue after abort, and teardown remains pending until
it returns. Cancellation-aware native operations or process isolation are needed
for a hard termination guarantee.

## Current Scope

`crepuscularity-lite` is an application shell and bridge runtime, not a replacement for the `.crepus` parser or renderers. Use it when you need an embedded guest runtime inside a native desktop host. Use `crepuscularity-gpui` for direct GPUI template rendering, `crepuscularity-tui` for terminal UIs, and `crepuscularity-web` / `crepuscularity-webext` for HTML and extension targets.
