#!/usr/bin/env python3
"""Prepare an actual-source WASM callback ownership probe; never runs Cargo.

Copies core.rs with only a captured Drop sentinel instrumented into the real
callback. No callback algorithm is recreated. The probe uses existing locked
dependencies and an isolated temporary manifest. See printed runner commands.
"""
import argparse
import hashlib
from pathlib import Path
import shutil
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
out = args.out.resolve()
(out / 'src').mkdir(parents=True, exist_ok=True)
source_path = root / 'crates/crepuscularity-webext/src/wasm/core.rs'
source = source_path.read_text()
needle = '        let callback_reject = reject.clone();'
body = '            if let Some(error) = last_error() {'
assert source.count(needle) == 1 and source.count(body) == 1, 'callback layout changed; review instrumentation'
source = source.replace(needle, '        let callback_probe = CallbackProbe::new();\n' + needle)
source = source.replace(body, '            let _callback_probe = callback_probe;\n' + body)
source += '''
thread_local! { static LIVE_CALLBACK_PROBES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
struct CallbackProbe;
impl CallbackProbe {
    fn new() -> Self { LIVE_CALLBACK_PROBES.with(|n| n.set(n.get()+1)); Self }
}
impl Drop for CallbackProbe {
    fn drop(&mut self) { LIVE_CALLBACK_PROBES.with(|n| n.set(n.get()-1)); }
}
#[wasm_bindgen]
pub fn live_callback_probes() -> u32 { LIVE_CALLBACK_PROBES.with(|n| n.get()) }
#[wasm_bindgen]
pub async fn probe_call(target: JsValue) -> std::result::Result<JsValue, JsValue> {
    call_callback_method(&target, "probe.run", "run", &[]).await
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
'''
(out / 'src/lib.rs').write_text(source)
packages = tomllib.loads((root / 'Cargo.lock').read_text())['package']
names = ['js-sys', 'serde', 'serde_json', 'serde-wasm-bindgen', 'wasm-bindgen', 'wasm-bindgen-futures']
versions = {name: next(p['version'] for p in packages if p['name'] == name) for name in names}
manifest = '[package]\nname="callback-ownership-probe"\nversion="0.0.0"\nedition="2021"\n[lib]\ncrate-type=["cdylib"]\n[workspace]\n[dependencies]\n'
manifest += ''.join(f'{name}="={versions[name]}"\n' for name in names)
(out / 'Cargo.toml').write_text(manifest)
shutil.copyfile(root / 'Cargo.lock', out / 'Cargo.lock')
(out / 'probe.mjs').write_text('''import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import init, { probe_call, live_callback_probes } from "./pkg/callback_ownership_probe.js";
await init({ module_or_path: readFileSync(new URL("./pkg/callback_ownership_probe_bg.wasm", import.meta.url)) });
globalThis.chrome = { runtime: {} };
const cases = [
  ["sync", cb => cb("sync-value"), "sync-value", false],
  ["async", cb => queueMicrotask(() => cb("async-value")), "async-value", false],
  ["throw", () => { throw new Error("sync-failure"); }, "sync-failure", true],
  ["callback-then-throw", cb => { cb("first-value"); throw new Error("late-failure"); }, "first-value", false],
  ["lastError", cb => { chrome.runtime.lastError = {message:"last-error"}; try { cb(null); } finally { delete chrome.runtime.lastError; } }, "last-error", true],
];
for (const [name, run, expected, rejected] of cases) {
  const baseline = live_callback_probes();
  let value, failure;
  try { value = await probe_call({run}); } catch (error) { failure = String(error); }
  if (rejected) assert.ok(failure?.includes(expected), `${name}: ${failure}`);
  else { assert.equal(failure, undefined, name); assert.equal(value, expected, name); }
  assert.equal(live_callback_probes(), baseline, `${name}: captured Rust sentinel leaked`);
}
let callback;
const pending = probe_call({run(cb) { callback = cb; }});
await Promise.resolve();
assert.equal(live_callback_probes(), 1, "pending API must retain its callback");
callback("later-value");
assert.equal(await pending, "later-value");
assert.equal(live_callback_probes(), 0, "late callback must release ownership");
assert.throws(() => callback("second-value"), "one-shot callback rejects repeat invocation");
for (let i=0; i<1000; i++) {
  try { await probe_call({run() { throw new Error("repeat"); }}); } catch {}
}
assert.equal(live_callback_probes(), 0, "repeated synchronous throws retain callbacks");
console.log("PASS actual WASM callback ownership: sync/async/throw/callback-then-throw/lastError/repeated throw");
''')
(out / 'source.sha256').write_text(hashlib.sha256(source_path.read_bytes()).hexdigest() + '  ' + str(source_path) + '\n')
commands = f'''RUSTC="$(rustup which rustc)" RUSTC_WRAPPER="" CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=1 cargo build --offline --manifest-path {out}/Cargo.toml --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir {out}/pkg {out}/target/wasm32-unknown-unknown/debug/callback_ownership_probe.wasm
bun {out}/probe.mjs
'''
(out / 'commands.txt').write_text(commands)
print(commands)
