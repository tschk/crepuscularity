import * as bindings from "./wasm/crepuscularity_wasm.js";
import { createParser } from "./parser.js";

export type * from "./parser.js";
export const { IR_VERSION, parseCrepus, parseTemplate } = createParser(bindings);
