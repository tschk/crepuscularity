import { test, expect } from "bun:test";
import { runtime_version } from "./runtime-as-adapter.js";

test("runtime_version returns '1.0.0'", () => {
    expect(runtime_version()).toBe("1.0.0");
});
