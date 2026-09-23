import { describe, it, expect } from "bun:test";
import { runtime_version } from "./runtime-as-adapter.js";

describe("runtime_version", () => {
    it("returns 1.0.0", () => {
        expect(runtime_version()).toBe("1.0.0");
    });
});
