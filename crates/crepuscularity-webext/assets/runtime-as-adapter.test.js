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
    mockWasm = {
      runtime_version: () => 42,
      __getString: (ptr) => {
        if (ptr === 42) return "1.2.3";
        return "";
      }
    };
    await init("dummy.wasm");
    expect(runtime_version()).toBe("1.2.3");
  });
});
