# Crepuscularity → ZPUI (experimental)

This optional Zig adapter consumes Crepuscularity View IR v7 and constructs
ZPUI elements. Compile `.crepus` templates with the existing Rust compiler:

```sh
crepus native ir examples/counter.crepus > counter.json
```

The adapter neither parses templates nor executes expressions. Hosts pass the
returned JSON to `Document.parse`, retain the document, and call `render` inside
a ZPUI view's render method. Named button handlers reach the host through
`onCrepusClick`; the host owns state changes and calls `cx.notify()`.

Use a compiler that emits IR version 7. The installed `crepus 0.11.0` emitted
version 5 during this work; its output is retained as a rejection fixture rather
than changed to version 7. Building the current compiler and generating a v7
counter snapshot remain integration checks.

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

## Initial coverage

| Area | Mapping |
| --- | --- |
| Document | Version 7, array of roots, bounded size/depth, owned decoded storage |
| `stack` | Row/column flex layout, spacing, recursive children |
| `text` | Resolved content, including escaped Unicode |
| `button` | Label, accessibility role/name, named `onClick` host callback and focusability |
| `scroll` | Identified horizontal or vertical scrolling container |
| Style | Padding and per-edge padding, margin, width/height, font size/weight, opacity, colors, corner radius, border, hidden/overflow-hidden, flex direction |

Widths and heights accept nonnegative points, `-1` for fill, and `-2` for auto
content sizing. Colors currently accept `#RRGGBB`. Original `classes` are
validated metadata; resolved style hints drive rendering, and this module does
not reinterpret Tailwind tokens.

Other non-null node fields and style hints produce errors. Unsupported node
kinds—including input, image, links, conditional IR, and collection IR—also
produce errors. Nullable inactive fields are accepted. This is an evaluated
snapshot adapter: binding ownership, expression evaluation, template hot reload,
IME/editing, drag/drop, full accessibility, and platform parity are outside the
initial acceptance scope. A successful decode does not prove GPU rendering or
native event dispatch.

## Validation

```sh
zig fmt --check build.zig build.zig.zon src/*.zig examples/*.zig
zig build test -j1 --summary all
```

Tests consume a byte-for-byte copy of the canonical Rust golden fixture at
`crates/crepuscularity-native/tests/fixtures/hello.json`. They cover multiple
roots, Unicode, input-buffer independence, cleanup on invalid documents, scroll
axis, sizing sentinels, version rejection, and unsupported behavior.
They also reject the installed compiler's actual v5 output, excessive depth,
and oversized documents.

The eight decoder tests passed on installed Zig 0.16.0. Before merge, rerun on the
upstream Zig 0.17.0 toolchain, compile the host view against the exact pinned
module, and exercise button dispatch, keyboard focus, scrolling, sizing, and
document replacement in a headless or platform fixture. Native ZPUI compilation
and those interaction checks have not run. This draft adds no release or
deployment workflow.

## Licensing

The new adapter follows Crepuscularity's ISC license. At the inspected upstream
revision, GitHub reports no detected project license and the repository tree has
no top-level LICENSE. Separate vendored libraries and assets have their own
notices; those do not establish a license for ZPUI itself. Upstream licensing
clarification is required before distributing a ZPUI-linked release. This PR
contains only the independently written adapter and Crepuscularity fixture.
