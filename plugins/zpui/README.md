# Crepuscularity → ZPUI (experimental)

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

The adapter declares **no automatic upstream download** and vendors no ZPUI
code. It adds no Cargo dependencies, shared macro changes, or CE/Kit types.
The host supplies its already configured module:

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

Give each adapter instance a distinct namespace. Element IDs include that
namespace, the node's tree position, and its optional source `style.id`.
IDs distinguish siblings and instances; positional IDs do not promise to retain
state when a document reorders nodes. The example retains its document for the
whole view lifetime.
The namespace string must also remain alive while its frame is active.

## Coverage

The adapter implements **18 of the 25 View IR v7 node kinds**. The compiler-generated
form fixture contains every supported kind; the remaining kinds stay explicit
`UnsupportedNode` errors.

| Node kinds | Mapping |
| --- | --- |
| `stack`, `scroll` | Row/column layout, spacing, alignment, justification, recursive children, horizontal/vertical scrolling |
| `text`, `button` | Resolved/bound text; accessible button with named click dispatch and keyboard focus |
| `toggle`, `checkbox`, `slider` | Native or ZPUI-drawn controls, typed boolean/numeric bindings and change events, slider range/step |
| `picker`, `tabs` | Native popup/segmented selection by option value; tabs render only the selected panel |
| `progress`, `meter` | Painted proportional track and accessible value/minimum/maximum |
| `badge`, `divider`, `spacer` | Tone colors, separator orientation, fixed-size or flexible space |
| `list`, `listItem` | Ordered/unordered markers and accessible list structure |
| `link` | Visible-text accessible name, mouse/keyboard dispatch, preserved `href`/`target`/`rel` |
| `image` | Host asset/decoded source, image fitting, alt name, loading placeholder and atlas output |

Styles include padding and all margin edges, sizing/minimum/maximum constraints,
aspect ratio, font size/weight/family, text alignment and line height, italic and
text decorations, clipping/line clamp, colors, borders/radius, opacity, visibility,
flex grow/shrink/wrap/direction/self alignment, relative/absolute positioning and
insets, shadows, supported cursors, and image fit. `classes` remain validated
metadata; the adapter uses the compiler's resolved hints.

Widths/heights/maxima accept nonnegative points, `-1` for parent fill and `-2`
for auto sizing. Minimum sizes are nonnegative. Colors accept `#RRGGBB`.
Image positioning is centered; other positions fail. `position` accepts only
`relative` and `absolute`. Supported cursors are `auto`, `default`, `pointer`,
`text`, and `move`. Font families map to the platform system font, `serif`, or
`monospace`. Styling native controls affects their container and requested width;
the platform owns their intrinsic control height. Segmented tabs retain ZPUI's
native radio-group selection semantics and expose a selected tab-panel role.

The seven remaining node kinds are `if`, `forEach`, `input`, `slotRotate`,
`webView`, `dropzone`, and `filePicker`. Tab icons, long-press handlers,
transforms, gradients, text transforms, letter spacing, z-index, selection hints,
and other unsupported active fields fail explicitly. This adapter adds no
expression VM, retained text editor, timer runtime or web browser. The pinned
upstream provides low-level input/IME APIs but no ready-made text editor.

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

`Value` is `text`, `boolean`, or `number`. Text and selections require text,
toggles/checkboxes require booleans, and sliders require finite numbers within
the declared range. Readers must be pure and stable during one render.
Selection values must match an option. `Change` includes
the optional binding name, optional handler name, and typed new value. Controls
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

## Validation

```sh
zig fmt --check build.zig build.zig.zon src/*.zig examples/*.zig tests/*.zig
zig build test -j1 --summary all
python3 tools/test-native.py --zig /path/to/zig-0.17.0/zig --zpui-source /path/to/zpui
```

The nine original decoder tests cover the canonical Rust golden fixture,
Unicode/storage ownership, version rejection, unsupported behavior, size limits,
and the exact 128/129 node-depth boundary. Seven additional decoder tests exercise
a compiler-generated fixture containing all 18 supported kinds, option ownership,
invalid controls/options, typed style contracts and the remaining unsupported kinds.

Fifteen native headless tests compile the renderer and both example views against
the exact pinned ZPUI module. They exercise mouse/Space/Enter activation and focus,
toggle/checkbox host changes, slider range/step, popup and tab selection, bound-text
redraw, link metadata, layout constraints/alignment/margins, colors/borders/shadows,
scrolling on both axes, decoded image atlas output, accessibility roles/ranges,
text decoration overrides, extreme numeric ranges, document retirement, and missing-host/backend errors.
The test allocator checks leaks. The harness verifies toolchain/source pins, uses
one build job and opens no desktop windows.

On case-insensitive macOS filesystems, two upstream `SOURCE` provenance files
collide with their libraries' `source/` directories. The harness permits only
those metadata deletions; changes to compiler inputs are rejected.

The counter and form fixtures come from the current Rust parser/native lowering
crate. The macOS workflow regenerates both and runs decoder/native acceptance.
These checks establish headless layout/event/atlas behavior; actual GPU output,
native OS widget integration and other platforms still need separate acceptance.
No release or deployment workflow is added.

## Licensing

The new adapter follows Crepuscularity's ISC license. ZPUI is MIT-licensed.
Preserve the applicable MIT notice
and the separate notices for upstream vendored libraries and assets when
distributing a linked application. This PR contains the independently written
adapter and Crepuscularity fixtures.
