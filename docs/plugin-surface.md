# Plugin surface

**Also:** [View IR contract](view-ir-contract.md) · [Polyglot plugins](polyglot.md) · [Reference plugins](../plugins/README.md)

A **Crepuscularity language plugin** is a package in any host language that lets applications use `.crepus` without reimplementing the DSL. Plugins sit on the [View IR contract](view-ir-contract.md); they do not embed the Rust parser unless they opt into `crepuscularity-abi`.

## Required capabilities

| Capability | Description |
| --- | --- |
| **Types** | Host structs or classes matching `ViewIr` / `ViewNode` for the current `IR_VERSION`, generated from JSON Schema or hand-maintained against fixtures. |
| **Compile** | Call `crepus native ir` (subprocess) or `crepuscularity-abi` (in-process) to obtain IR JSON from templates, includes, and context. |
| **Render** | Map decoded nodes to a UI representation. Reference plugins implement portable **HTML** (`render_html` / `renderHtml`); platform plugins map to SwiftUI, Compose, GPUI, Qt, GTK, terminal widgets, etc. |

The Python, Ruby, PHP, Go, C# and TypeScript/Bun HTML adapters preserve carriage returns
in attribute values with character references, including form names and hidden-input
values. Their HTML adapters preserve `style.formValue` as the initial
value of unbound inputs and textareas. Explicit empty values remain distinct from
missing values. Nonempty bindings retain precedence; these adapters do not add a
live browser binding runtime. Secure single-line inputs retain password masking; multiline inputs remain textareas. Textarea output preserves initial LF and CR content
through HTML parsing, with the browser normalizing the editable value as usual.
Swift and Rust also preserve CR in the attributes their reference HTML subsets support.

Go, Ruby and C# picker HTML emits options and a `data-bind` name without selecting
an option from the context. The host must apply bound selection; plain HTML uses
the browser's default first option. This differs from the Solid/Vue/Imba SSR
selection behavior described in the View IR contract.

The TypeScript/Bun adapter awaits the CLI asynchronously and drains both output
streams while it runs. Rendering therefore yields to the host event loop during
compilation. Output is buffered; JSON decoding and HTML serialization still run
on the JavaScript thread. This does not provide cancellation or a lag-free
guarantee for large results.

The Python and Ruby reference adapters read templates and exchange CLI streams as strict
UTF-8, independently of the process locale. Invalid UTF-8 remains an error.
The Go and TypeScript/Bun adapters reject invalid UTF-8 template bytes before
starting the CLI.
The generated Python ctypes scaffold owns its native session. Release it with
`close()` or a `with CrepusAbiSession(...)` block; repeated close calls are safe.
Its destructor also releases an unclosed session.
Java and Kotlin escape UTF-16 surrogate code units in context JSON, preserving
valid pairs and allowing the CLI to reject unpaired surrogates without replacement.
TypeScript/Bun binding events preserve `__proto__` as an own context data field
that is included in CLI requests.

The POSIX C and C++ reference HTML helpers delegate to the Rust CLI web renderer
with an explicit empty context. They retain all rendered nodes and do not autoload
sibling context files. C reports insufficient caller-buffer capacity as an error;
C++ returns the complete string. Their IR APIs remain separate.
V also delegates HTML rendering to the canonical CLI, using its explicit context
map and per-call temporary context files.

The Java reference HTML convenience API delegates source rendering to the Rust
CLI's web renderer; its output follows that renderer's HTML/attribute contract.
Its IR API remains separate. See [Java HTML behavior](polyglot.md#java-html-convenience-api).

Rust session callbacks remain registered when they return an ordinary error;
dispatch propagates the error before rendering.

## Recommended capabilities

| Capability | Description |
| --- | --- |
| **Events** | Wire `onClick` / `data-onclick` handlers and binding events (`bind:var:value`) to host callbacks; re-render or patch after dispatch. |
| **Sessions** | Hold template path, virtual files, and context in a small session type (see Python `ViewSession`, Go `ViewSession`, TypeScript `CrepusViewSession`). |
| **Hot reload** | Subscribe to file changes; apply `HotReloadEnvelope` patch or full reload messages when integrating with `crepus dev` or custom watchers. |

## Out of scope for core plugins

- Shipping Equilibrium or another FFI framework inside Crepuscularity.
- Full GPUI/web parity in every host (each plugin chooses supported `ViewStyle` fields).
- Publishing all reference plugins to registries (optional; separate repos encouraged for production).

## Reference layout

In-tree references live under [`plugins/`](../plugins/). Manifest: [`plugins/crepuscularity-plugins.toml`](../plugins/crepuscularity-plugins.toml).

| Language | Path | Compile | ABI session |
| --- | --- | --- | --- |
| Python | `plugins/python` | CLI | Optional (`crepuscularity_abi.py`) |
| Go | `plugins/go` | CLI | — |
| TypeScript (Bun) | `plugins/typescript-bun` | CLI | — |
| Zig | `plugins/zig` | CLI | Optional (`crepuscularity_abi.zig`) |
| V | `plugins/v` | CLI | — |
| Rust | `plugins/rust` | CLI | — |
| Swift | `plugins/swift` | CLI | — |
| C# | `plugins/csharp` | CLI | Optional |
| C / C++ | `plugins/c`, `plugins/cpp` | CLI | C optional ABI |
| Ruby, PHP, Java, Kotlin | respective dirs | CLI | PHP optional ABI |

Run local smoke: `scripts/plugin-smoke.sh` (after `cargo build -p crepuscularity-cli` and `cargo build -p crepuscularity-abi`).

The typed binding generator preserves C `const` at each pointer level in Rust and
the shared type model used by the typed backends. For example, `const char **`
becomes `*mut *const c_char`, while `char * const *` becomes `*const *mut c_char`.
Top-level pointer `const` does not qualify the pointed-to value. Unsupported
declarators and pointer qualifiers return errors.

Generated Rust bindings mark foreign blocks `unsafe` for
[Rust 2024 compatibility](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html).
This syntax requires Rust 1.82 or newer and also works in the 2021 edition.

## Production plugins

For registry releases, prefer dedicated repositories (`crepuscularity-python`, etc.) with their own CI and semver. Depend on a pinned `crepus` binary or document the minimum `crepuscularity-cli` version. Link back to this contract when `IR_VERSION` changes.


### Zig 0.16 reference adapters

CLI calls take `(allocator, io, path)`: pass `init.io` from `std.process.Init` to
`renderIr` or `renderHtml`. The executable `crepus` is resolved through that I/O
implementation's parent `PATH`; HTML uses the canonical renderer with empty context.
The caller frees `ViewIr.json` and `UiDocument.html` with the supplied allocator.

ABI sessions keep `Session.init(allocator)`. `renderIr` and `dispatchEvent` return
owned snapshots whose `root` is `[]ViewNode`; call `ir.deinit(allocator)` when done.
Snapshots remain valid after context changes or session teardown. Session handles
must not be copied. `setContext` and `patchContext` accept JSON-compatible Zig values.
Embedded NUL in raw template, directory or event strings is rejected.
`toHtml` supports text, stack, scroll, button and image nodes and reports
`UnsupportedNode` for other kinds. `onEvent` callback registration remains unsupported;
poll the IR returned by dispatch instead.

After building the CLI and native debug ABI library, put `target/debug` on `PATH`.
Both `zig build test --build-file plugins/build.zig` and the nested
`plugins/zig/build.zig` entry compile the actual adapters and run their tests against
the built CLI and ABI. They require Zig 0.16.
