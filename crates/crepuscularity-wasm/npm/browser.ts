import init, * as bindings from "./browser/crepuscularity_wasm.js";
import { createParser } from "./parser.js";

export type * from "./parser.js";
let parser: Promise<ReturnType<typeof createParser>> | undefined;

/** Load one browser/worker parser instance; calls may then parse synchronously. */
export function loadParser(
  wasmUrl: string | URL = new URL("./browser/crepuscularity_wasm_bg.wasm", import.meta.url),
) {
  return parser ??= init({ module_or_path: wasmUrl }).then(() => createParser(bindings));
}
