# Crepuscularity plugin bindings

`crepus plugins bindgen` generates bindings from the plugin manifest's ABI header.
The bundled Crepuscularity contract has a checked adapter for C, C++, Zig, Rust,
C#, and V. It preserves all 13 functions, opaque session pointers, signed 32-bit
status results, nullable callbacks, and caller-owned returned strings. Release
returned strings with `crepus_string_free` and keep managed callbacks alive until
they are replaced or the session is freed.

Rust, C#, D, Nim, and Odin use checked repository generators for supported
headers, including the opaque Crepus session and callback types. Unsupported type
forms, target calling conventions, and identifiers fail explicitly. Other
equilibrium backends use the checked bundled contract when it matches; otherwise
they accept only supported scalar imports with one-line prototypes and named
parameters. Unsupported declarations, warnings, and required companion files fail
before any generated outputs are replaced. Filesystem write errors can still
leave partially written output.

Typed generators admit the bundled include guard, `<stdint.h>`/`<stddef.h>`
includes, and C++ linkage wrappers. Other preprocessor directives and trigraphs
are rejected rather than evaluated. V retains its checked canonical Crepus
adapter and the restricted scalar-header fallback used by legacy backends.

The repository's Go, Python, Java, Kotlin, Swift, PHP, Ruby, and TypeScript adapters
retain their existing behavior. TypeScript uses the repository's Bun/Node stub.

Run the normal regression suite with:

```sh
cargo test -p crepuscularity-plugin-bindgen
```

Optional compiler checks require Zig, V with a C compiler, the .NET 8 SDK, and the
built ABI library for V linking:

```sh
cargo build -p crepuscularity-abi
CREPUS_ABI_LIB_DIR="$PWD/target/debug" cargo test -p crepuscularity-plugin-bindgen generated_ -- --ignored --test-threads=1
```

The session lifecycle test links generated Rust bindings to the real ABI library:

```sh
cargo build -p crepuscularity-abi
CREPUS_ABI_LIB_DIR="$PWD/target/debug" cargo test -p crepuscularity-plugin-bindgen generated_rust_bindings_run_the_real_session_lifecycle -- --ignored
```
