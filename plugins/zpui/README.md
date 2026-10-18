# Crepuscularity → ZPUI 0.2.0 (experimental)

This optional Zig adapter consumes Crepuscularity View IR v7 and constructs
ZPUI elements. Compile `.crepus` templates with the existing Rust compiler:

```sh
crepus native ir examples/counter.crepus > counter.json
```

The adapter consumes View IR; the Rust compiler owns template parsing and
expression evaluation. Hosts pass JSON to `Document.parse`, retain the document,
and call the fallible `render` inside a ZPUI view. Named button handlers reach
`onCrepusClick`. Controlled widgets read typed values through `crepusValue` and
report changes through `onCrepusChange`; the host commits state and calls
`cx.notify()`. Links and image resolution also stay under host control.

Use a compiler that emits IR version 7. The installed `crepus 0.11.0` emitted
version 5 during this work; its output is retained as a rejection fixture rather
than changed to version 7. The bundled exporter uses the current Rust parser
and native lowering crate to generate the checked v7 counter fixture:

```sh
cargo run --locked -j1 --manifest-path tools/fixture-exporter/Cargo.toml -- examples/counter.crepus > counter.json
cmp counter.json tests/fixtures/counter.json
```

## Upstream baseline and build boundary

The source API baseline is
[`plyght/zpui@cca22983e7e8124bb8e650616e3803f217e52b95`](https://github.com/plyght/zpui/tree/cca22983e7e8124bb8e650616e3803f217e52b95).
Its package reports version `0.0.1` and requires **Zig 0.17.0**. Pin the revision,
not that development version alone. This adapter's standard-library-only IR
module can be tested with Zig 0.16; actual ZPUI consumers require the upstream
compiler and platform build configuration.

Use the complete upstream checkout and its configured `zpui` build module.
It provides platform libraries, translated C headers, shaders, and resources;
importing `src/zpui.zig` alone does not configure those dependencies. The
upstream package's current `.paths` list only includes its build files and
`src`, while its build references `vendor` and app resources. Verify packaging
before using a fetched archive as a Zig package dependency.

The adapter vendors no ZPUI code and does not download it. Native rendering links
a separate Rust static library under `native-text`, locked to
`unicode-segmentation =1.13.3` for UAX #29 extended graphemes. It requires Cargo
and a matching installed Rust target. Decoder builds need only Zig. This library
has its own workspace/lock and adds no root Cargo dependency, shared macro change,
or CE/Kit type. The host supplies its configured ZPUI module:

```zig
// Host build.zig; `zpui_module` is supplied by the host's pinned upstream build.
const bridge = b.dependency("crepuscularity_zpui", .{
    .target = target,
    .optimize = optimize,
}).module("crepuscularity_zpui");
bridge.addImport("zpui", zpui_module);
app.root_module.addImport("zpui", zpui_module);
app.root_module.addImport("crepuscularity_zpui", bridge);
```

Declare the local adapter package in the host's `build.zig.zon`:

```zig
.dependencies = .{
    .crepuscularity_zpui = .{ .path = "../crepuscularity/plugins/zpui" },
},
```

[`examples/counter.zig`](examples/counter.zig) is a host view, not a standalone
platform launcher. Initialize it with compiler-produced JSON and open it through
the host's normal ZPUI window setup. It shows click dispatch and composition with
native Zig elements. Applications may similarly compose retained editors or
other native widgets outside this initial IR subset.

## Ownership and identity

`Document.parse` owns its nodes and strings; the input buffer may be freed after
parsing. Do not copy an owning document or call `deinit` twice. Keep the document
alive while elements and their event listeners refer to it, including until the
old frame has been replaced after a document update. Schedule replacement and
retirement through the host's frame lifecycle rather than freeing a document
inside one of its own click callbacks.

Callback payloads and copied collection scopes belong to retained per-window,
per-namespace storage, rather than the temporary frame arena. Successful renders
retain the current and preceding generations through presentation; failed
renders discard only their new allocations. Window/element disposal releases
the storage. Hosts must still copy callback arguments they retain after a
callback returns.

Give each adapter instance a distinct namespace. Element IDs include that
namespace, the node's tree position, and its optional source `style.id`.
IDs distinguish siblings and instances; positional IDs do not promise to retain
state when a document reorders nodes. The example retains its document for the
whole view lifetime.
The namespace string must also remain alive while its frame is active.

## Coverage and finish goal

The goal is all 25 node kinds, 71 style fields and five event contracts with
working behavior and acceptance evidence. [COVERAGE.md](COVERAGE.md) and
[coverage.json](coverage.json) record the inventory and unfinished work. CI checks
the inventory against the Rust source. The goal remains in progress.

All 25 kinds decode with owned storage. The bridge implements 24 directly,
including a retained editor and icon-bearing tabs. A macOS WKWebView reference
provides real browser navigation; other platforms require a host factory and
separate acceptance. The finish goal remains in progress.

The editor supports host-controlled text, selection, pointer dragging, grapheme
navigation/deletion, multiline vertical movement, clipboard and UTF-16 IME
preedit/commit. It owns its text layout between frames. Secure inputs mask extended
graphemes, preserve multiline breaks and block plaintext clipboard/IME extraction.
This does not establish OS secure-keyboard mode, undo, word movement or autoscroll.
Hosts can override `crepusInput` with their own editor.

Styles cover spacing, size constraints, aspect ratio, flex layout, alignment,
text size/weight/family/decorations, colors, borders, opacity, visibility/clipping,
static/relative/absolute positioning, shadows and eight two-stop gradients.
Translations move paint, hit regions and accessibility bounds without changing
flow layout. Images support all nine object positions with actual UV cropping.
Unicode upper/lower casing inherits through containers; `normal` resets it, and
option values remain unchanged. Capitalization and transformed editor presentation
are unsupported. Source classes remain owned metadata.

Widths/heights/maxima accept nonnegative points, `-1` for parent fill and `-2`
for auto sizing. Minimum sizes are nonnegative. Colors accept `#RRGGBB`.
Gradients require direction and both endpoints. Cursors accept `auto`, `default`,
`pointer`, `text`, and `move`. Fixed positioning, wait cursor, letter spacing,
z-index, scale/rotation and user selection are still explicit errors. Native
controls retain their platform-owned intrinsic height.

## Host contract

`render` now returns `!AnyElement`. Call `validateHost(View, &view, &document)`
after initializing host state to catch missing methods, unknown bindings, wrong
value types or invalid selections before opening a window. Rendering repeats
validation so later invalid host state also returns an error. A host can handle
that error in its normal UI error path. The examples validate during `init`.

| Method | Required when |
| --- | --- |
| `onCrepusClick(handler, event, window, cx)` | A button has `onClick` |
| `crepusValue(bind) ?Value` | Text or a control has a binding; picker/tabs always have one |
| `onCrepusChange(change: Change, window, cx)` | A control has a binding or `onChange` |
| `onCrepusLink(link: Link, event, window, cx)` | A link appears |
| `crepusImage(src) ?ImageSource` | Custom/remote image resolution; otherwise configure ZPUI's asset source |
| `crepusResolve(expression, scope) ?Value` | Optional expression override; scoped record paths work without it |
| `crepusCondition(expression, scope) ?bool` | Optional condition override; boolean bindings work without it |
| `onCrepusScopedClick(handler, scope, event, window, cx)` | A button callback inside a collection; may also handle unscoped clicks |
| `onCrepusLongPress(handler, scope, mouseDown, window, cx)` | A node has `onLongPress` |
| `onCrepusDrop(files: Files, window, cx)` | A dropzone has `onDrop` |
| `onCrepusPick(files: Files, window, cx)` | A file picker has `onPick` |
| `crepusAcceptExtensions(accept) ![]const []const u8` | MIME filters; return extensions without dots, or an error |
| `crepusInput(request: Input, window, cx) !AnyElement` | Optional retained-editor override |
| `crepusWebView(request: WebView, window, cx) !NativeViewId` | Required outside macOS or on a headless platform; optional browser override on macOS |

`Value` is `text`, `boolean`, `number`, `record`, or `list`. Lists contain keyed
`Item` values; records contain named `Field` values. Keys must be nonempty and unique.
`Scope` contains parent, item name, key, index and value. `row.title` resolves a
record field in the nearest matching scope; arbitrary expressions remain host-owned. Text and selections require text,
toggles/checkboxes require booleans, and sliders require finite numbers within
the declared range. Readers must be pure and stable during one render.
Selection values must match an option. `Change` includes
the optional binding name, optional handler name, typed new value and optional scope. Controls
without a binding or handler are displayed disabled. An unbound handler receives
the event; the host must replace its snapshot to change the displayed value.

On a platform without native controls, opt into ZPUI's drawn controls with
`window.setDesktopControls(true)` and configure a supported desktop theme
(e.g. `.adwaita` or `.breeze`). A missing native/drawn backend returns
`MissingControlBackend` rather than creating an empty widget.

Image storage and strings returned by the host must outlive the frame. The pinned
ZPUI loader reads configured assets and encoded/decoded images; it does not fetch
URI sources. A remote-image resolver must return encoded data, a decoded image,
or a configured asset, not another URI. Linked image resources remain under the
host's ownership. Link dispatch does not open external destinations itself.
Encoded bytes must also survive any in-flight background decode; retain them
in host asset storage for the image cache's lifetime.
`Change` and `Link` strings borrow document/host storage; copy them if retaining
them beyond that storage's lifetime.

[`examples/form.zig`](examples/form.zig) demonstrates typed controlled state,
selection, link dispatch and a borrowed decoded image alongside the existing
[counter](examples/counter.zig). Both views require normal host window setup.

Async dialogs and hold timers copy their context; other event data and scopes are
borrowed for the callback duration. Copy data retained by the host. The host's
allocator must outlive outstanding platform-dialog callbacks. Input overrides must
provide actual retained editing and copy request data they retain. Web factories
must create/reuse an attached browser view and detach it at the end of its host
lifetime. A backend without native child views fails explicitly. `MacBrowser` owns
one window attachment, rejects reuse in another window and uses a nonpersistent
WebKit data store. Its `title()` borrows WebKit storage; copy retained results.

Rotation timers reset when phrases/interval change and cancel on unmount. Conditions
validate only the active branch; tabs validate only the selected panel. Readers
must be pure and stable throughout one render. Long press recognizes a 500 ms hold
and cancels after movement above 8 points or release. Its callback is separate from
normal click/change handling; hosts decide whether to suppress those actions.

## Validation

```sh
python3 tools/check-coverage.py
zig fmt --check build.zig build.zig.zon src/*.zig examples/*.zig tests/*.zig
zig build test -j1 --summary all
python3 tools/test-native.py --zig /path/to/zig-0.17.0/zig --zpui-source /path/to/zpui
```

Twenty-one decoder tests cover current compiler fixtures, all node shapes,
Unicode/ownership, IR boundaries, malformed file filters and style contracts.
They pass on Zig 0.16 and 0.17. Forty-four native headless tests compile the renderer
and examples against pinned ZPUI, including scoped runtime, async lifecycle,
retained input, icon tabs, translations, image crops and casing. Five separately
locked Rust tests cover grapheme boundaries, UTF-16 and Unicode case/mask helpers;
strict Clippy and format checks also run in CI. Allocator failures fail acceptance.

For real macOS acceptance, run:

```sh
python3 tools/test-native.py --zig /path/to/zig-0.17.0/zig --zpui-source /path/to/zpui --desktop
```

This opens its own AppKit/Metal window, edits Unicode through the native input
handler, navigates WKWebView, verifies its title and 200×100 `#123456` snapshot,
and drains deferred platform teardown. It requires Xcode's Swift compiler and an
active graphical session. It does not capture another app or change OS permissions.
The snapshot is written to `/tmp/crepus-zpui-browser-acceptance.png`. This proves the
specified macOS scenario; broader OS widget/GPU behavior remains a finish gate.

On case-insensitive macOS filesystems, two upstream `SOURCE` provenance files
collide with their `source/` directories. The harness permits only those metadata
deletions and rejects altered compiler inputs. CI regenerates the counter/form
fixtures from the current Rust parser and lowering crate. Standalone archives use
the bundled IR inventory snapshot; repository checks also require exact agreement
with the authoritative Rust source.

## Licensing

The new adapter follows Crepuscularity's ISC license. ZPUI is MIT-licensed.
Preserve the applicable MIT notice
and the separate notices for upstream vendored libraries and assets when
distributing a linked application. This PR contains the independently written
adapter and Crepuscularity fixtures. The Unicode dependency is MIT OR Apache-2.0;
this distribution includes its MIT license and copyright notice under `native-text`.

### Key-validation performance probe

From this directory, run the standalone CPU-time probe on macOS/Linux:

```sh
zig test -OReleaseFast --dep host -Mroot=tests/key_scaling.zig \
  --dep view_ir -Mhost=src/host.zig -Mview_ir=src/view_ir.zig \
  -lc --test-filter 'manual collection'
```

It validates 1,000/2,000/4,000 unique keys and a duplicate at the last row,
25 times each, checking the original slice and final row. On an Apple Silicon
host with Zig 0.16.0, median CPU times from three runs were
24.6/107.5/461.2 ms for the original unique-key scan and 0.37/0.63/1.47 ms
for the hash-set version. Last-row duplicates changed from
25.8/108.5/457.4 ms to 0.36/0.67/1.25 ms. These are validation measurements,
not frame latency. Temporary hash storage is released after each call; allocation
failure or a list beyond safe hash-table capacity uses the original scan and
preserves its first-error behavior. The normal decoder test command also runs
host tests covering that fallback and borrowed-slice/error compatibility.
