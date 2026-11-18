// crates/crepuscularity-wasm/npm/browser.ts
import init, * as bindings from "./browser/crepuscularity_wasm.js";

// crates/crepuscularity-wasm/npm/parser.js
function createParser({ ir_version, parse_crepus_json, parse_template_json }) {
  const IR_VERSION = ir_version();
  function validateUnicode(value, label) {
    if (/[\uD800-\uDFFF]/u.test(value)) {
      throw new TypeError(`${label} contains an unpaired UTF-16 surrogate`);
    }
  }
  function serializeContext(context) {
    if (!context)
      return;
    return JSON.stringify(context, (_key, value) => {
      if (typeof value === "number" && !Number.isFinite(value)) {
        throw new TypeError("context numbers must be finite");
      }
      return value;
    });
  }
  function parseCrepus(source, context) {
    validateUnicode(source, "source");
    const json = parse_crepus_json(source, serializeContext(context));
    return JSON.parse(json);
  }
  function parseTemplate(source, filename, context) {
    validateUnicode(source, "source");
    if (filename !== undefined)
      validateUnicode(filename, "filename");
    const json = parse_template_json(source, filename, serializeContext(context));
    return JSON.parse(json);
  }
  return { IR_VERSION, parseCrepus, parseTemplate };
}

// crates/crepuscularity-wasm/npm/browser.ts
var parser;
function loadParser(wasmUrl = new URL("./browser/crepuscularity_wasm_bg.wasm", import.meta.url)) {
  return parser ??= init({ module_or_path: wasmUrl }).then(() => createParser(bindings));
}
export {
  loadParser
};
