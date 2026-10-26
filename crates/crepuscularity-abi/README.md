# crepuscularity-abi

Optional **C ABI** (`cdylib`) for in-process View IR compilation and event dispatch. Polyglot plugins should prefer **`crepus native ir`** (subprocess JSON); use this crate when a host language needs sessions without spawning the CLI.

- Header: [`include/crepuscularity_abi.h`](include/crepuscularity_abi.h)
- Does **not** depend on [Equilibrium](https://github.com/tschk/equilibrium)
- Replaces any experimental `crepuscularity-ffi` approach: plain `extern "C"`, no proc-macro FFI framework in-tree

```bash
cargo build -p crepuscularity-abi --profile abi
cargo test -p crepuscularity-abi
```

To check the exported header and shared library from a C caller on macOS or
Linux, run this smoke test from the repository root:

```bash
cargo build --locked -p crepuscularity-abi
cc -std=c11 -Wall -Wextra -Werror \
  -I crates/crepuscularity-abi/include crates/crepuscularity-abi/tests/smoke.c \
  -L target/debug -lcrepuscularity_abi -Wl,-rpath,"$PWD/target/debug" \
  -o /tmp/crepus-abi-smoke
/tmp/crepus-abi-smoke
```

This checks the development library; production hosts should use `--profile abi`.

On macOS the dylib uses `@rpath/libcrepuscularity_abi.dylib` as its install name.
Linked hosts must provide an rpath to its directory, such as
`-Wl,-rpath,@executable_path` when shipping it beside the executable. Check the
production library in a temporary relocated directory with:

```bash
cargo build --locked -p crepuscularity-abi --profile abi
python3 crates/crepuscularity-abi/tests/macos_relocation.py target/abi/libcrepuscularity_abi.dylib
```

## Embedder contract

- **Component selection** applies to template and virtual-file sources, including event rerenders. An explicit `crepus_session_set_component` overrides a file entry's `#Component` suffix. Passing `NULL` clears the override and restores the original entry; unknown components fail during rendering.
- **`CrepusSession` is not thread-safe** — one session per thread, or serialize externally.
- **Event names** accept a raw name, JSON string, or object with `handler` (or the `event` alias). When both fields are present, `handler` wins. Binding updates and callback/result routing use that same selected name. Valid JSON containing unrepresentable numbers is rejected before context updates or callbacks.
- **Event callback JSON pointers** stay valid until the next `crepus_session_dispatch_event_json` on that session.
- **Callbacks must not re-enter their session**, including freeing it. Dispatch holds an exclusive session reference until it returns. Native calls reject re-entry and report the error through `crepus_last_error`; a rejected free leaves the session alive. The Python wrapper raises `RuntimeError` before the native call.
- **Python callback exceptions** are re-raised by `dispatch_event` after native dispatch returns. Context updates already applied by the event remain in place.
- **Production builds** use the workspace `abi` profile (`--profile abi`) so panics never unwind across the C ABI.

See [docs/view-ir-contract.md](../../docs/view-ir-contract.md) and [docs/polyglot.md](../../docs/polyglot.md).
