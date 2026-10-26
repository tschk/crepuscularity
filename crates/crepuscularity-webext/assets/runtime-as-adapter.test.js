import { describe, it, expect, beforeEach, afterEach, mock } from "bun:test";
import init, { runtime_version } from "./runtime-as-adapter.js";

describe("runtime_version", () => {
  let mockWasm;
  let originalFetch;
  let originalWebAssembly;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
    originalWebAssembly = globalThis.WebAssembly;

    globalThis.fetch = mock(() => Promise.resolve({}));
    globalThis.WebAssembly = {
      instantiateStreaming: mock(async () => {
        return {
          instance: {
            exports: mockWasm,
          }
        };
      })
    };
  });

  afterEach(() => {
      globalThis.fetch = originalFetch;
      globalThis.WebAssembly = originalWebAssembly;
  });

  it("should return 'unknown' if wasm.runtime_version is not defined", async () => {
    mockWasm = {}; // runtime_version is undefined
    await init("dummy.wasm");
    expect(runtime_version()).toBe("unknown");
  });

  it("should return version string from wasm if runtime_version is defined", async () => {
    const memory = new originalWebAssembly.Memory({ initial: 1 });
    const ptr = 16;
    const text = "1.2.3";
    new DataView(memory.buffer).setUint32(ptr - 4, text.length * 2, true);
    new Uint16Array(memory.buffer, ptr, text.length).set([...text].map((char) => char.charCodeAt(0)));
    mockWasm = { runtime_version: () => ptr, memory };
    await init("dummy.wasm");
    expect(runtime_version()).toBe("1.2.3");
  });
});
