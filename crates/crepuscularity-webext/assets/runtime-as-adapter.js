// crepuscularity — AssemblyScript runtime adapter
//
// Drop this file as vendor/runtime.js when building an AssemblyScript app.
// Wraps the AS WASM binary in the same interface that popup.js and content.js
// expect from a wasm-bindgen Rust build.
//
// Requirements:
//   asc assembly/index.ts --outFile vendor/runtime_bg.wasm --exportRuntime
//
// The --exportRuntime flag causes AS to export __getString / __newString so
// we can transfer strings across the WASM boundary without a separate library.

let wasm = null;

export default async function init(wasmUrl) {
  const response = await fetch(wasmUrl);
  const { instance } = await WebAssembly.instantiateStreaming(response, {
    env: {
      abort(msgPtr, filePtr, line, col) {
        const msg = wasm ? __getString(msgPtr) : "(no msg)";
        throw new Error(`AssemblyScript abort: ${msg} at ${line}:${col}`);
      },
    },
  });
  wasm = instance.exports;
}

// Reads a UTF-16 string from AS linear memory using the exported __getString.
function __getString(ptr) {
  if (!wasm || !wasm.__getString) throw new Error("AS runtime not exported; compile with --exportRuntime");
  return wasm.__getString(ptr);
}

// Allocates a UTF-16 string in AS linear memory using the exported __newString.
function __newString(str) {
  if (!wasm || !wasm.__newString) throw new Error("AS runtime not exported; compile with --exportRuntime");
  return wasm.__newString(str);
}

// Parses a JSON string returned by an AS export.
function jsonOut(ptr) {
  return JSON.parse(__getString(ptr));
}

// Serialises a JS value to JSON and writes it into AS memory.
function jsonIn(value) {
  return __newString(JSON.stringify(value));
}

// ------------------------------------------------------------------
// Exports matching the crepuscularity WASM contract.
// Implement any subset in your assembly/index.ts.
// ------------------------------------------------------------------

export function runtime_version() {
  return (wasm && wasm.runtime_version) ? __getString(wasm.runtime_version()) : "unknown";
}

export function browser_program() {
  if (!wasm.browser_program) {
    return 'export async function runBrowserProgram(_api) {}';
  }
  return __getString(wasm.browser_program());
}

export function app_manifest() {
  return wasm.app_manifest ? jsonOut(wasm.app_manifest()) : null;
}

export function extract_specs(text) {
  if (!wasm.extract_specs) return [];
  return jsonOut(wasm.extract_specs(__newString(text)));
}

export function render_frontend(request) {
  if (!wasm.render_frontend) throw new Error("render_frontend not implemented");
  return jsonOut(wasm.render_frontend(jsonIn(request)));
}

export function render_popup(state) {
  if (!wasm.render_popup) throw new Error("render_popup not implemented");
  return jsonOut(wasm.render_popup(jsonIn(state)));
}

export function handle_popup_action(action, data) {
  if (!wasm.handle_popup_action) return { noop: true };
  return jsonOut(wasm.handle_popup_action(__newString(action), jsonIn(data)));
}
