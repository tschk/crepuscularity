# crepuscularity-wasm

WebAssembly bindings for the crepuscularity parser. Compiles a template to
[View IR](https://docs.rs/crepuscularity-native) and hands it to JavaScript as
JSON.

This is the same Rust code the `crepus` CLI uses, not a reimplementation, so
JavaScript consumers and native targets lower templates identically.

```rust
// Exposed to JS via wasm-bindgen:
parse_template_json(source, filename, context_json) -> String
parse_crepus_json(source, context_json) -> String
ir_version() -> u32
```

`parse_template_json` selects a parser frontend from `filename`'s extension, so
`.crepus`, `.jsx`/`.tsx`, `.svelte`, `.vue`, `.astro`, and Angular component
templates (`*.component.html`, `*.ng.html`, `*.ng`) all reach the same IR.
View IR currently stores one bare handler per event. Modifiers and repeated
handlers for the same event are rejected during lowering rather than discarded.
The HTML renderer has a richer event contract.

The IR crosses the boundary as a JSON string rather than a structured
`JsValue`: it avoids a `serde-wasm-bindgen` dependency, and `JSON.parse` on one
string beats field-by-field reflection for trees of any real size.

## Building

Requires the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI matching
the `wasm-bindgen` crate version, plus Bun for the JavaScript smoke tests.
`build.sh` reports a version mismatch; put a matching CLI on `PATH` to build.

```sh
./build.sh
```

The generated bindings and a TypeScript shim are published to npm as
[`@tschk/crepuscularity-wasm`](https://www.npmjs.com/package/@tschk/crepuscularity-wasm);
see `npm/README.md` for the JavaScript API.
