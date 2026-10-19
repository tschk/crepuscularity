# Crepuscularity plugin bindings

`crepus plugins bindgen` generates bindings from the plugin manifest's ABI header.
The bundled Crepuscularity contract has a checked adapter for C, C++, Zig, Rust,
C#, and V. It preserves all 13 functions, opaque session pointers, signed 32-bit
status results, nullable callbacks, and caller-owned returned strings. Release
returned strings with `crepus_string_free` and keep managed callbacks alive until
they are replaced or the session is freed.

The complete contract must match the bundled header; whitespace and comments may
differ. Changed or additional declarations require updating the adapter. Other
headers use equilibrium's supported scalar imports with one-line prototypes and
named parameters. Multiline prototypes, unnamed/variadic parameters, and C line
splicing are rejected because the import parser cannot preserve them. Unsupported declarations or
backends fail before generated files are replaced. Filesystem write errors can
still leave partially written output.

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
