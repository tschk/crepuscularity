import init, * as bindings from "./browser/crepuscularity_wasm.js";
import { createParser } from "./parser.js";
let parser;
export function loadParser(wasmUrl = new URL("./browser/crepuscularity_wasm_bg.wasm", import.meta.url)) {
  return parser ??= init({ module_or_path: wasmUrl }).then(() => createParser(bindings));
}
