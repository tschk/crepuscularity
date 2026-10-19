import { afterEach, expect, test } from "bun:test";

const fetchOriginal = globalThis.fetch;
afterEach(() => { globalThis.fetch = fetchOriginal; });
// Actual valid WASM module with no exports; exercises startup without the AS string ABI.
const bytes = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
let version = 0;
async function adapter() {
  return import(`../assets/runtime-as-adapter.js?test=${++version}`);
}

for (const [label, input] of [
  ["current loader envelope", () => ({ module_or_path: bytes.buffer })],
  ["raw buffer", () => bytes.buffer],
  ["typed view", () => bytes],
]) {
  test(`AssemblyScript adapter initializes ${label} without fetching`, async () => {
    globalThis.fetch = () => { throw new Error("byte input must not be fetched"); };
    const runtime = await adapter();
    await runtime.default(input());
    expect(runtime.runtime_version()).toBe("unknown");
  });
}

test("AssemblyScript adapter retains URL initialization", async () => {
  const fetched = [];
  globalThis.fetch = async (url) => {
    fetched.push(url);
    return new Response(bytes, { headers: { "Content-Type": "application/wasm" } });
  };
  const runtime = await adapter();
  await runtime.default("runtime.wasm");
  expect(fetched).toEqual(["runtime.wasm"]);
  expect(runtime.runtime_version()).toBe("unknown");
});

test("AssemblyScript adapter rejects invalid WASM bytes", async () => {
  globalThis.fetch = () => { throw new Error("byte input must not be fetched"); };
  const runtime = await adapter();
  await expect(runtime.default({ module_or_path: new Uint8Array([1, 2, 3]) })).rejects.toBeInstanceOf(WebAssembly.CompileError);
});
