# View IR contract

**Also:** [Polyglot plugins](polyglot.md) · [Plugin surface](plugin-surface.md) · [Native shells](native.md)

This document is the stable cross-language boundary for Crepuscularity. Rust (`crepuscularity-native`) is the authoritative compiler; hosts consume **View IR JSON** only.

## Version

- Field: `ViewIr.version` (camelCase in JSON).
- Must equal [`IR_VERSION`](https://docs.rs/crepuscularity-native/latest/crepuscularity_native/constant.IR_VERSION.html) from `crepuscularity-native` (currently **7**).
- Breaking IR shape changes bump `IR_VERSION` and require plugin type updates.

## Document shape

Root object:

```json
{
  "version": 7, /* always IR_VERSION; see above */
  "root": [ /* ViewNode[] */ ]
}
```

Node kinds include `stack`, `text`, `button`, `input`, `picker`, `image`, `link`, and others defined in [`crates/crepuscularity-native/src/ir.rs`](../crates/crepuscularity-native/src/ir.rs). Style hints use camelCase keys aligned with Swift/Kotlin decoders in `examples/native-shells/` and CLI scaffolds.

## JSON Schema

Generate schema for codegen (Swift, Kotlin, TypeScript, etc.):

```bash
cargo run -p crepuscularity-native --features schema --bin export-view-ir-schema
cargo run -p crepuscularity-native --features schema --bin export-view-ir-schema -- -o view-ir.schema.json
```

Plugins should pin a schema revision or `IR_VERSION` in their package metadata.

## Compile entrypoint (CLI)

Primary integration: subprocess **`crepus native ir`**. No Rust embedding required.

| Mode | Invocation |
| --- | --- |
| File | `crepus native ir <path.crepus> [--component Name] [--ctx FILE] [--var k=v] [--pretty]` |
| Stdin template | `crepus native ir --stdin --base-dir DIR` |
| JSON envelope | `crepus native ir --stdin-json` |

**Success:** compact or pretty JSON on **stdout** only.

**Failure:** `{"error":"..."}` on **stderr**, exit code non-zero.

### `--stdin-json` envelope

```json
{
  "entry": "main.crepus",
  "files": { "main.crepus": "motion.div\n  \"Hi\"" },
  "template": "div\n  \"Hi\"",
  "component": "Card",
  "context": { "name": "Ada", "items": [{ "label": "a" }] },
  "baseDir": "/path/to/project",
  "pretty": false
}
```

When both `files`+`entry` and `template` are present, **files+entry** wins. Context values are JSON scalars or arrays of objects (for `for` loops). Nested objects in context are rejected.

## Optional in-process ABI

[`crepuscularity-abi`](../crates/crepuscularity-abi/) exposes the same compile/session model via C (`include/crepuscularity_abi.h`): template or virtual files, context JSON, `crepus_session_render_ir_json`, event dispatch. This crate does **not** depend on Equilibrium; language packages wrap the C API with their own FFI tooling.

## Hot reload envelope

When a plugin supports live template updates, transport messages from `crepuscularity_native::hot_reload`:

- `HotReloadEnvelope { sequence, message }`
- `message.kind`: `noop`, `patch`, `fullReload`, `error`, plus dev-only variants for Aurorality

Patch payloads use `IrMutation` paths from `crepuscularity_native::mutations`. Plugins may apply patches or fall back to full `ViewIr` replacement.

## Equilibrium

[Equilibrium](https://github.com/softwarecorporation/equilibrium) is **not** part of this contract. Plugin maintainers may use it in separate repos to compile/load their own native glue.
