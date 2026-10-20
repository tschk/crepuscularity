# Crepuscularity Reference Plugins

These plugins demonstrate the polyglot contract: spawn `crepus native ir`, read View IR JSON from stdout, decode it into host-language types, and create UI from that tree. The portable reference UI output is HTML; platform-specific packages can map the same node model to native controls. For in-process sessions and event dispatch, wrap `crates/crepuscularity-abi/include/crepuscularity_abi.h`. Native workspace files live next to the reference packages so a host project can drop in the matching package and use Crepus without adopting Rust.

Java's HTML convenience API uses the canonical `crepus render` source-HTML route
rather than extracting text from IR JSON. It preserves multiple nodes, web handler
metadata and escaping. Its `renderIr` API still uses the View IR boundary. See
[Java HTML context and rendering behavior](../docs/polyglot.md#java-html-convenience-api).

## Shared packages in this tree

| Path | Role |
|------|------|
| `crepuscularity-flutter/` | Flutter runtime renderer (`crepus flutter dep` / `add`) |
| `crepuscularity-components/` | Shared UI component catalog + themes (`crepus components list` / `add` / `themes`) |

Moonshine is a separate product ([`github.com/tschk/moonshine`](https://github.com/tschk/moonshine): `@tschk/moonshine`, `@tschk/crepus-moonshine`, `@tschk/moonshine-components`). Use `crepus moonshine new` / `crepus moonshine dep` from the CLI. Local `packages/moonshine*` trees are legacy bridges.

Workspace entrypoints:

```bash
go test ./plugins/go/...
bun --cwd plugins test
zig build test --build-file plugins/build.zig
swift build --package-path plugins
dotnet build plugins/CrepuscularityPlugins.sln --nologo --verbosity:minimal
cargo test --manifest-path plugins/rust/Cargo.toml
```

`plugins/crepuscularity-plugins.toml` lists every reference package, workspace file, and smoke command.

`crepus plugins bindgen` generates ABI imports for these packages. Rust generation
uses equilibrium's Rust binding API with an adapter for the opaque `CrepusSession`
and nullable `CrepusEventCallback` typedefs. Function declarations come from the
selected header, including `--abi-header` overrides; incompatible session or
callback typedefs are rejected. Rust, C#, Nim, D and Odin share bounded header
framing for multiline declarations, comments and C line continuations before
equilibrium-ffi parsing. Function names and parameter counts must survive parsing;
unnamed parameters and unsupported or incomplete signatures fail before output.
D and Odin also reject function names reserved by their target lexer with a
name-specific error before writing that binding. Reserved-name aliases are not
currently generated.
This is not a full C preprocessor or a general C type-equivalence check.
`cargo test --locked -p crepuscularity-plugin-bindgen --test header_output` checks
multiline equivalence and rejection behavior across all five backends.
`cargo test -p crepuscularity-plugin-bindgen --test rust_output` compiles freshly
generated Rust and checks every current ABI function signature.
`cargo test -p crepuscularity-plugin-bindgen --test script_output` checks freshly
generated Ruby/PHP syntax, the Nim banner marker, and silent PHP imports.
On Unix it also checks unusual header paths with Rust parsing, Python compilation
and `javac`; install Python and Java alongside Ruby and PHP. Script banners use each
language's comment syntax; PHP keeps its opening tag before the banner.

V binding generation supports spaces in include/library directories. It rejects
non-UTF-8 paths and V directive delimiters (line breaks, double quotes, `//`,
` #`), flag-directory separators (` -`, leading `-`) and V path-macro prefixes
(`$first_existing`, `$when_first_existing`). Commas in the derived library
directory are also rejected because Darwin linker rpath flags split on commas.
These are explicit generation errors rather than malformed emitted source.

A custom manifest may select `language = "nim"`. Nim generation uses typed ABI
declarations and exports opaque session pointers, nullable `cdecl` callbacks,
32-bit statuses and string/pointer functions from the selected header. It emits
raw `importc` declarations; link the ABI library through the consuming Nim build.
Unsupported types, including opaque sessions passed by value, fail before writing
that binding file. Function and parameter names that violate Nim underscore
token rules are also rejected before output; backticks still admit Nim keywords.
Callback strings retain the C header's borrowed, read-only lifetime contract; returned owned strings still require `crepus_string_free`.
Style-equivalent procedure names with unconditionally identical parameter types
fail during generation. Potential `clong`/`culong` overload collisions use generated
Nim `when` guards, so the consumer compiler rejects them only on targets where
the parameter types are identical. These checks preserve the C ABI type spellings.
The opt-in `nim_target_guards` test checks Linux and Windows types with `nim check`
without linking; run it with `-- --include-ignored`.
`cargo test -p crepuscularity-plugin-bindgen --test nim_output` requires Nim and a
C compiler. It checks all current signatures and header overrides, then exercises
callbacks against an independent C stub compiled with the real header; it does
not run the Rust ABI implementation.

The opt-in runtime gate uses a freshly built Rust ABI library. On macOS:

```sh
cargo build --locked -p crepuscularity-abi
CREPUS_ABI_LIB="$PWD/target/debug/libcrepuscularity_abi.dylib" cargo test --locked -p crepuscularity-plugin-bindgen --test nim_output generated_nim_calls_all_thirteen_exports_against_the_real_rust_abi -- --ignored --exact
```

Use the corresponding library path on other platforms; this runtime gate has
been exercised on macOS. It calls all 13 current exports through freshly generated
Nim bindings, including callbacks, state updates, errors and string cleanup.

A custom manifest may also select `language = "d"`. The D adapter exports module
`abi_generated`, opaque sessions and `extern(C)` callbacks/functions, preserving
32-bit statuses and pointer constness. It shares Nim's typed declaration parser;
unsupported declarations fail before that binding file is written. Link the ABI
library through the consuming D build and follow the same C string ownership and
callback lifetime rules. The native checks require installed LDC (`ldc2`) and a C
compiler:

```sh
cargo test --locked -p crepuscularity-plugin-bindgen --test d_output
CREPUS_ABI_LIB="$PWD/target/debug/libcrepuscularity_abi.dylib" cargo test --locked -p crepuscularity-plugin-bindgen --test d_output generated_d_calls_all_thirteen_exports_against_the_real_rust_abi -- --ignored --exact
```

The second command uses the freshly built macOS library from the preceding
example and exercises all 13 exports against Rust; the default tests include an
independent C callback stub. Other platforms require their corresponding library
path and remain unverified by this gate.

A custom manifest can select `language = "odin"`. The generated package
`abi_generated` imports `system:crepuscularity_abi` and exports all current typed
C procedures and callbacks. Point the host linker's library search path at the
standard-named ABI library. Unsupported declarations fail before writing output.
Odin `cstring` does not enforce C constness: callback text remains borrowed and
read-only, and `string(cstring)` borrows its allocation. Inspect or clone returned
text before calling `crepus_string_free`; callbacks must not reenter their session.

```sh
cargo test --locked -p crepuscularity-plugin-bindgen --test odin_output
CREPUS_ABI_LIB="$PWD/target/debug/libcrepuscularity_abi.dylib" cargo test --locked -p crepuscularity-plugin-bindgen --test odin_output generated_odin_calls_all_thirteen_exports_against_the_real_rust_abi -- --ignored --exact
```

These checks require installed Odin and a C compiler. On macOS the fixture uses
Apple's archive tool and, when installed, the Command Line Tools SDK in its build
subprocess to avoid the installed Odin driver's SDK-path quoting issue. It does not change
the global developer selection or installed tools. The explicit runtime gate
requires a freshly built, conventionally named Rust ABI library; only macOS has
been exercised.

C# generation preserves the current ABI's signed 32-bit status codes and pointer
signatures, including constructor and callback registration, with explicit Cdecl
imports. Callback pointers use `IntPtr`; keep the corresponding managed Cdecl
delegate alive until unregistering it or freeing the session. The raw ABI runtime
regression is `dotnet run --project scripts/fixtures/csharp-generated-abi -- "$CREPUS_ABI_LIB"`.

Build the CLI first:

```bash
cargo build -p crepuscularity-cli --no-default-features
export CREPUS_BIN="$PWD/target/debug/crepus"
export PATH="$PWD/target/debug:$PATH"
```

The Go and TypeScript/Bun CLI transports reject malformed UTF-8 in template files
and successful CLI JSON responses instead of silently replacing bytes. Their
transport tests use labelled CLI stubs for malformed responses alongside
current-CLI integration cases. TypeScript/Bun drains both pipes asynchronously
and preserves nonzero-exit stderr diagnostics before decoding successful output.

Python context renders preserve original template line endings when constructing
the UTF-8 stdin JSON envelope, including CRLF and lone CR. The transport regression
checks the exact source payload with a labelled CLI stub. Binary subprocess capture
preserves failed-command stderr even when stdout contains invalid UTF-8, retaining
strict UTF-8 validation and normalized diagnostic line endings.

Ruby validates successful CLI output as UTF-8. Failed commands preserve valid
UTF-8 stderr diagnostics even when stdout contains incomplete or invalid bytes.

PHP uses the same captured-process path with and without context, preserving
failed-command stderr before attempting to decode stdout as JSON.
Requests are written completely to a temporary stdin file before launch, allowing
stdout to drain even when the child emits output before consuming a large request.

Java and Kotlin reject malformed UTF-8 in successful IR and HTML responses.
Their diagnostic fixture covers both response routes, raw Unicode, invalid byte
sequences and failed-command stderr precedence.
Both adapters start draining stdout and stderr before writing large requests, preserving
failed-command diagnostics over stdin write errors and rejecting incomplete writes
even when a child exits successfully.

C# also requires UTF-8 CLI responses, accepting an optional UTF-8 BOM and rejecting
malformed bytes after checking exit status. UTF-16/UTF-32 BOM responses are not
accepted. Its `--transport` smoke mode covers IR and HTML with a labelled CLI byte fixture.
Malformed UTF-8 template files are rejected before launching the CLI. Source-file
BOM detection still supports UTF-8, UTF-16 and UTF-32 templates.
Failed commands take precedence over stdin I/O errors; a successful exit still
rethrows an incomplete-write error with its original stack trace.

Swift errors retain the CLI exit status and include nonempty stderr as their
localized description. Stderr is buffered in a private temporary file and forwarded
after exit, avoiding a second pipe that could block stdout drainage.

V captures CLI stderr separately from successful IR/HTML output. Nonzero commands
report stderr, falling back to captured stdout when no diagnostic is available.

Rust writes stdin on a joined worker while draining child output, so early
diagnostics cannot block large requests. Failed CLI diagnostics take precedence
over broken-pipe errors; successful exits still require the full request write.

Build the optional ABI runtime when testing in-process sessions:

```bash
cargo build -p crepuscularity-abi
export CREPUS_ABI_LIB="$PWD/target/debug/libcrepuscularity_abi.dylib" # macOS
```

The C# `CrepusAbiSession` implements `IDisposable`. After disposal, session
operations throw `ObjectDisposedException` before serialization or native calls.
Instances still require external synchronization, as specified by the C ABI.
`SetTemplate` rejects NUL characters in template/base-directory strings before
native calls, preserving the current template on rejection. With `CREPUS_ABI_LIB`
set to the native library, run `dotnet run --project plugins/csharp-smoke -- --abi-nul`
for the Unicode/string-boundary regression.
The native lifecycle regression runs with:

```bash
dotnet run --project scripts/fixtures/csharp-abi-lifetime -- "$CREPUS_ABI_LIB"
```

Run the full local smoke matrix:

```bash
scripts/plugin-smoke.sh
```

The matrix also compiles and runs `plugins/rust/abi_smoke.rs` against the native
library, exercising all generated ABI declarations, callbacks and string ownership.

Smoke commands for installed local toolchains:

```bash
python3 -m unittest discover plugins/python
go test ./plugins/go/...
bun --cwd plugins test
v -gc none test plugins/v
zig build test --build-file plugins/build.zig
cargo test --manifest-path plugins/rust/Cargo.toml
cargo test -p crepuscularity-abi
swift build --package-path plugins
gcc plugins/c/crepuscularity_plugin.c -o /tmp/crepus-c-smoke && /tmp/crepus-c-smoke plugins/fixtures/hello.crepus
g++ plugins/cpp/crepuscularity_plugin.cpp -std=c++17 -o /tmp/crepus-cpp-smoke && /tmp/crepus-cpp-smoke plugins/fixtures/hello.crepus
ruby -Iplugins/ruby -e 'require "crepuscularity_plugin"; p CrepuscularityPlugin.render_ir("plugins/fixtures/hello.crepus").version'
ruby -Iplugins/ruby -e 'require "crepuscularity_plugin"; raise unless CrepuscularityPlugin.render_html("plugins/fixtures/hello.crepus").include?("data-crepus-kind")'
```

Optional when the toolchains are installed:

```bash
dotnet build plugins/CrepuscularityPlugins.sln --nologo --verbosity:minimal
javac -d /tmp/crepus-java plugins/java/CrepuscularityPlugin.java
kotlinc plugins/kotlin/CrepuscularityPlugin.kt -d /tmp/crepus-kotlin
php -r 'require "plugins/php/CrepuscularityPlugin.php"; var_dump(CrepuscularityPlugin::renderIr("plugins/fixtures/hello.crepus")["version"]);'
```

ABI-backed session wrappers currently exist for C, Python, PHP, C#, and Zig. The wrappers expose the shared session lifecycle, context JSON, IR rendering, and event dispatch path. Other language references remain CLI-backed until their package-specific native loading story is promoted from smoke coverage to maintained SDK surface.

PHP ABI sessions own their native pointer and cannot be cloned. `close()` is
idempotent; destruction releases an open session. Run
`php plugins/php/session_ownership_smoke.php` for the safe counting-stub
regression (PHP FFI and a C compiler on a Unix host).

PHP rejects literal NUL bytes in template text, base directories and raw event
strings before passing them to the C ABI. NUL characters inside JSON-encoded
context/array-event values remain supported. Run `php plugins/php/abi_input_smoke.php`
with `CREPUS_ABI_LIB` set to the built ABI library for admission and state-preservation checks.

Python's maintained ABI session and newly generated Python session bindings reject
`copy.copy` and `copy.deepcopy` because each session owns its native pointer.
Run `python3 -m unittest discover -s plugins/python -p test_crepuscularity_abi.py`
with `CREPUS_ABI_LIB` set to the built ABI library for ownership and event checks.

Secure-input HTML adapter regressions (installed Ruby/PHP, isolated CLI stubs):
`ruby plugins/ruby/tests/test_secure_input.rb` and
`php plugins/php/secure_input_smoke.php`. Single-line secure inputs preserve
password masking; multiline inputs retain textarea rendering.

Python, Ruby, PHP, Go, TypeScript/Bun and C# HTML adapters render View IR links recursively, preserving
`href`, optional `target`/`rel`, and child content. Values are escaped;
`javascript:`, `vbscript:` and navigable `data:` destinations omit `href`.
Dynamic `hrefBind` remains a snapshot in these adapters. Focused checks:
`python3 -m unittest discover -s plugins/python -p test_link_html.py`,
`ruby plugins/ruby/tests/test_link_html.rb`, and
`php plugins/php/link_html_smoke.php` (isolated IR/CLI collaborators).
TypeScript checks: `bun test ./plugins/typescript-bun/link-html.test.ts` with
`CREPUS_BIN` pointing at a built CLI; these cover both isolated IR and real CLI output.

Go input defaults and password masking use the same shared View IR fixture. Run
`CREPUS_BIN="$PWD/target/debug/crepus" go -C plugins/go test -race ./...`
after building the current CLI. The tests also exercise public CLI/session output.

Python, Ruby, PHP, Go, TypeScript/Bun and C# Input renderers preserve View IR
`style.inputType`, `style.formName` and Boolean `style.required`. Supported types
follow the native text-compatible input allowlist; secure inputs use password,
and multiline controls remain textareas. Names, including explicit empty names,
are escaped. Shared cases live in
[`fixtures/input-metadata.view-ir.json`](fixtures/input-metadata.view-ir.json);
each adapter has an input-metadata regression alongside its input-default checks.
PHP's `empty_context_smoke.php` also checks that default session contexts serialize
as JSON objects while nested lists retain their shape.

C# uses both shared input fixtures through public HTML/session APIs:
`dotnet run --project plugins/csharp-smoke -- --input-html plugins/fixtures`.
This fixture mode currently requires Unix and Python for its labelled CLI stub.

C# Link conversion uses public renderer/session checks via
`dotnet run --project plugins/csharp-smoke -- --link-html`. This mode uses a
labelled Unix/Python CLI stub; real CLI and browser verification are separate.

Zig 0.16 CLI calls require an explicit `std.Io` argument; see
[Zig adapter ownership and build checks](../docs/plugin-surface.md#zig-016-reference-adapters).
