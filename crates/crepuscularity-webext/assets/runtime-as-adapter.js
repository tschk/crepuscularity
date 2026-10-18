// crepuscularity — AssemblyScript runtime adapter
//
// Drop this file as vendor/runtime.js when building an AssemblyScript app.
// Wraps the AS WASM binary in the same interface that popup.js and content.js
// expect from a wasm-bindgen Rust build.
//
// Requirements:
//   asc assembly/index.ts --outFile vendor/runtime_bg.wasm --exportRuntime
//
// The --exportRuntime flag exposes allocation/GC helpers. This adapter moves
// UTF-16 strings through exported memory without a separate loader library.

let wasm = null;

export default async function init(input) {
  const source = input?.module_or_path ?? input;
  const imports = {
    env: {
      abort(msgPtr, filePtr, line, col) {
        const msg = wasm ? __getString(msgPtr) : "(no msg)";
        throw new Error(`AssemblyScript abort: ${msg} at ${line}:${col}`);
      },
    },
  };
  const { instance } = source instanceof ArrayBuffer || ArrayBuffer.isView(source)
    ? await WebAssembly.instantiate(source, imports)
    : await WebAssembly.instantiateStreaming(await fetch(source), imports);
  wasm = instance.exports;
}

// AS managed strings store their byte length immediately before the payload.
function __getString(ptr) {
  ptr >>>= 0;
  if (!ptr) return null;
  if (!wasm || !wasm.memory) throw new Error("AS memory is not exported");
  const length = new DataView(wasm.memory.buffer).getUint32(ptr - 4, true) >>> 1;
  const units = new Uint16Array(wasm.memory.buffer, ptr, length);
  let text = "";
  for (let i = 0; i < length; i += 1024) {
    text += String.fromCharCode(...units.subarray(i, i + 1024));
  }
  return text;
}

function __newString(str) {
  if (!wasm || !wasm.__new || !wasm.__pin || !wasm.__unpin || !wasm.memory) {
    throw new Error("AS runtime not exported; compile with --exportRuntime");
  }
  const ptr = wasm.__new(str.length * 2, 2) >>> 0;
  const units = new Uint16Array(wasm.memory.buffer, ptr, str.length);
  for (let i = 0; i < str.length; i++) units[i] = str.charCodeAt(i);
  return ptr;
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
  // Keep the first argument alive while allocating the second outside the WASM stack.
  const actionPtr = wasm.__pin(__newString(action));
  try {
    return jsonOut(wasm.handle_popup_action(actionPtr, jsonIn(data)));
  } finally {
    wasm.__unpin(actionPtr);
  }
}
