# @tschk/crepuscularity-wasm

The crepuscularity parser, compiled to WebAssembly. This is the same Rust code
that backs the `crepus` CLI — not a reimplementation — so JavaScript consumers
and native targets lower templates identically.

```ts
import { parseCrepus, IR_VERSION } from "@tschk/crepuscularity-wasm";

const ir = parseCrepus(`div flex flex-col gap-4\n  span text-lg\n    "hello"`);
// { version: 7, root: [ { kind: "stack", ... } ] }
```

Template variables are bound by passing a context object. String, finite number,
boolean, and null values retain their types for expression evaluation. Non-finite
numbers (`NaN` and infinities) throw `TypeError` instead of becoming JSON null:

```ts
parseCrepus('span\n  "hello {name}"', { name: "Ada" });
```

`parseCrepus` and `parseTemplate` reject source strings and filenames containing
unpaired UTF-16 surrogates with `TypeError` before WebAssembly encoding can replace
them. Valid astral characters and literal replacement characters are preserved.

## Other syntaxes

`parseTemplate` picks a frontend from the filename, so `.crepus`, `.jsx`/`.tsx`,
`.svelte`, `.vue`, `.astro`, and Angular component templates
(`*.component.html`, `*.ng.html`, `*.ng`) all compile to the same View IR:

```ts
import { parseTemplate } from "@tschk/crepuscularity-wasm";

parseTemplate('<div class="flex"><span>{t}</span></div>', "Panel.svelte");
parseTemplate("<template><div class=\"flex\">{{ t }}</div></template>", "Panel.vue");
```

These are first-party frontends: neither `svelte` nor `vue` is a dependency.
They compile the **template**. A `<script>` block is extracted and its
semantics — Svelte runes, Vue's composition API, stores, lifecycle — are not
executed; expressions are evaluated by crepuscularity's own evaluator. A
construct the shared AST cannot represent is a parse error rather than a silent
drop.

## View IR

`parseCrepus` returns a `ViewIr`, a versioned tree of `ViewNode`s. Every node
carries the original template class tokens on `style.classes`, so web renderers
can pass them straight through as `className` instead of re-deriving CSS from
the lowered style hints.

Check `IR_VERSION` if you persist or transport IR — the schema is versioned and
bumps on incompatible changes.

## Browsers and workers

The root entry is synchronous and uses the Node/Bun loader. For browsers or
module workers, load the explicit browser entry first:

```ts
import { loadParser } from "@tschk/crepuscularity-wasm/browser";

const { parseTemplate, IR_VERSION } = await loadParser();
const ir = parseTemplate("<p>Hello</p>", "Panel.svelte");
```

Initialization is shared, including failures. Parsing is synchronous after loading;
use this entry inside a worker to keep parsing off the UI thread. Serve
`browser/crepuscularity_wasm_bg.wasm` relative to the bundled entry, or pass its
served URL to the first `loadParser(url)` call when your bundler relocates assets.
The browser entry has the same validation and context limits as the root entry.

Run the real Chromium/WebKit worker acceptance test after building with
`python3 browser.test.py` (requires installed Python Playwright browsers).

## Building

Requires the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI matching
the `wasm-bindgen` crate version, plus Bun for the smoke tests.
From this npm directory, run `bun run test` after rebuilding the bindings.

```sh
../build.sh
```

The build honors `CARGO_TARGET_DIR`, including paths containing spaces. Cargo and
`wasm-bindgen` use the same resolved target directory.

If the shell selects a compiler whose sysroot lacks the WASM target while rustup's
selected compiler has it installed, select that compiler explicitly:

```sh
RUSTC="$(rustup which rustc)" ../build.sh
```
