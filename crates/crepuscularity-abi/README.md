# crepuscularity-abi

Optional **C ABI** (`cdylib`) for in-process View IR compilation and event dispatch. Polyglot plugins should prefer **`crepus native ir`** (subprocess JSON); use this crate when a host language needs sessions without spawning the CLI.

- Header: [`include/crepuscularity_abi.h`](include/crepuscularity_abi.h)
- Does **not** depend on [Equilibrium](https://github.com/tschk/equilibrium)
- Replaces any experimental `crepuscularity-ffi` approach: plain `extern "C"`, no proc-macro FFI framework in-tree

```bash
cargo build -p crepuscularity-abi --profile abi
cargo test -p crepuscularity-abi
```

## Embedder contract

- **`CrepusSession` is not thread-safe** — one session per thread, or serialize externally.
- **Event callback JSON pointers** stay valid until the next `crepus_session_dispatch_event_json` on that session.
- **Production builds** use the workspace `abi` profile (`--profile abi`) so panics never unwind across the C ABI.

See [docs/view-ir-contract.md](../../docs/view-ir-contract.md) and [docs/polyglot.md](../../docs/polyglot.md).
