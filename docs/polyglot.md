# Polyglot Plugins

**Also:** [Documentation home](README.md) · [View IR contract](view-ir-contract.md) · [Plugin surface](plugin-surface.md) · [Native shells](native.md) · [CLI](cli.md)

Crepuscularity's cross-language contract is **View IR JSON**. Rust remains the compiler: it parses `.crepus`, evaluates context, expands includes, and lowers to `ViewIr`. Language plugins call the `crepus` CLI (default) or optionally load **`crepuscularity-abi`** for in-process sessions.

Equilibrium stays separate. Plugin authors can use Equilibrium, PyO3, JNI, `cgo`, `ctypes`, or another bridge in their own package when they want native loading, but core Crepuscularity does not require those tools.

## Contract summary

See **[View IR contract](view-ir-contract.md)** for the full spec (versioning, schema export, CLI stderr/stdout, hot reload). See **[Plugin surface](plugin-surface.md)** for what each language package must implement.

- `ViewIr.version` must equal `IR_VERSION` (currently **7**).
- Reference plugins: [`plugins/README.md`](../plugins/README.md).

## CLI Boundary

```bash
crepus native ir views/main.crepus --ctx context.json --pretty
```

`crepus native ir` writes only IR JSON to stdout. Errors are JSON on stderr and exit nonzero.

Supported forms:

```bash
crepus native ir <file.crepus> [--component Name] [--ctx FILE] [--var k=v] [--pretty]
crepus native ir --stdin --base-dir DIR
crepus native ir --stdin-json
```

`--stdin-json` accepts:

```json
{
  "entry": "main.crepus",
  "files": {
    "main.crepus": "div\n  \"Hello\""
  },
  "template": "div\n  \"Hello\"",
  "filename": "main.crepus",
  "baseDir": ".",
  "context": {
    "name": "Ada"
  },
  "pretty": true
}
```

When `files` and `entry` are present, they win over `template`. Context supports scalar values and arrays of objects for loop contexts.

For `template` input, optional `filename` selects the parser frontend (for example,
`App.svelte` or `Page.astro`). `baseDir` sets the directory for relative includes.

## Java HTML convenience API

Java `renderHtml(path, context)` delegates to the canonical `crepus render` command.
It returns source HTML tags, web-renderer attributes and handler metadata, retaining
all rendered nodes and escaping text through Rust. Hosts attach their own event
listeners to that metadata. `renderIr` continues to expose View IR JSON.

The Java adapter passes an owned temporary JSON context file, including for an
empty context, so an adjacent `context.toml` is not loaded implicitly. The file
is removed after success or failure. Use the CLI built from the matching checkout:
HTML JSON contexts share native IR validation (scalars and arrays of scalar values
or objects with scalar fields). Unsupported nesting fails before HTML output.
Files with a `.json` extension must contain valid JSON; the ordinary TOML context
route remains available.

## ABI Boundary

`crates/crepuscularity-abi` exposes the same compiler/session model as a plain C ABI for languages that prefer in-process loading:

```c
CrepusSession *session = crepus_session_new();
crepus_session_set_template_string(session, template_utf8, base_dir_utf8);
crepus_session_set_context_json(session, "{\"count\":1}");
char *ir_json = crepus_session_render_ir_json(session);
crepus_session_dispatch_event_json(session, "{\"handler\":\"increment\"}");
crepus_string_free(ir_json);
crepus_session_free(session);
```

The ABI is intentionally small: session setup, context JSON, virtual files, IR rendering, event dispatch, and a callback hook. It does not embed Equilibrium. Language packages can wrap this through `ctypes`, `cgo`, P/Invoke, JNI/JNA, Swift C interop, Zig `@cImport`, V C interop, or equivalent native mechanisms.

Event dispatch accepts either a raw handler string or JSON:

```json
{
  "handler": "bind:message:hello",
  "payload": {
    "source": "textField"
  },
  "context": {
    "message": "hello"
  }
}
```

The return value is a JSON envelope with the handler and freshly rendered `ViewIr`. Native shells can either use that directly or install a callback with `crepus_session_set_event_callback` to route actions into application state.

## Reference Plugins

Reference plugins live under `plugins/` and all use subprocess JSON:

| Plugin | Runtime |
| --- | --- |
| `plugins/python` | Python |
| `plugins/go` | Go |
| `plugins/typescript-bun` | TypeScript on Bun |
| `plugins/v` | V |
| `plugins/zig` | Zig |
| `plugins/csharp` | C# / .NET |
| `plugins/swift` | Swift |
| `plugins/java` | Java |
| `plugins/kotlin` | Kotlin |
| `plugins/ruby` | Ruby |
| `plugins/php` | PHP |
| `plugins/c` | C |
| `plugins/cpp` | C++ |
| `plugins/rust` | Rust |

Each reference plugin has two jobs: invoke the Crepus compiler and create a usable UI representation from the resulting IR. These are reference implementations, not registry packages. Production plugins can live in dedicated package repositories with their own CI, versioning, and release cadence.
