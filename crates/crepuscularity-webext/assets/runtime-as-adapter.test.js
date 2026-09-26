import { test, expect, beforeEach, afterEach, describe } from "bun:test";
import init, { runtime_version } from "./runtime-as-adapter.js";

describe("runtime_version", () => {
    let originalFetch;
    let originalWebAssembly;

    beforeEach(() => {
        originalFetch = global.fetch;
        originalWebAssembly = global.WebAssembly;
    });

    afterEach(() => {
        global.fetch = originalFetch;
        global.WebAssembly = originalWebAssembly;
    });

    test("returns 'unknown' if wasm is not initialized", () => {
        expect(runtime_version()).toBe("unknown");
    });

    test("returns 'unknown' if runtime_version is not exported by wasm", async () => {
        global.fetch = async () => ({});
        global.WebAssembly = {
            instantiateStreaming: async () => ({
                instance: {
                    exports: {}
                }
            })
        };

        await init("dummy.wasm");
        expect(runtime_version()).toBe("unknown");
    });

    test("returns string from wasm if runtime_version is exported", async () => {
        global.fetch = async () => ({});
        global.WebAssembly = {
            instantiateStreaming: async () => ({
                instance: {
                    exports: {
                        runtime_version: () => 123,
                        __getString: (ptr) => {
                            if (ptr === 123) return "2.0.0-mock";
                            return "wrong";
                        }
                    }
                }
            })
        };

        await init("dummy.wasm");
        expect(runtime_version()).toBe("2.0.0-mock");
    });
});
