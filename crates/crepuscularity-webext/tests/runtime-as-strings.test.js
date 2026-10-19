import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Optional compiler acceptance gate: CREPUS_ASC_PATH=/path/to/assemblyscript/bin/asc.js bun test ...
const compiler = process.env.CREPUS_ASC_PATH;
test.skipIf(!compiler)("AssemblyScript adapter exchanges real managed strings across allocation and GC", async () => {
  const directory = mkdtempSync(join(tmpdir(), "crepus-as-strings-"));
  try {
    const fixture = fileURLToPath(new URL("./fixtures/as-adapter.ts", import.meta.url));
    const output = join(directory, "fixture.wasm");
    const result = Bun.spawnSync([process.execPath, compiler, fixture, "--exportRuntime", "--outFile", output]);
    expect(result.exitCode, result.stderr.toString()).toBe(0);
    const runtime = await import(`../assets/runtime-as-adapter.js?strings=${Date.now()}`);
    const bytes = await Bun.file(output).arrayBuffer();
    const instantiate = WebAssembly.instantiate;
    // Force collection before host allocations, when earlier arguments need external roots.
    WebAssembly.instantiate = async (source, imports) => {
      const native = await instantiate(source, imports);
      return { instance: { exports: {
        ...native.instance.exports,
        __new(size, id) {
          native.instance.exports.__collect();
          return native.instance.exports.__new(size, id);
        },
      } } };
    };
    try {
      await runtime.default({ module_or_path: bytes });
    } finally {
      WebAssembly.instantiate = instantiate;
    }
    expect(runtime.runtime_version()).toBe("AS\u0000é😀\ud800");
    expect(runtime.browser_program()).toBe("\ufeffprogram\u0000😀");
    expect(runtime.app_manifest()).toEqual({ name: "é😀" });
    const cyclic = {};
    cyclic.self = cyclic;
    expect(() => runtime.handle_popup_action("failed", cyclic)).toThrow();
    for (const length of [0, 10_000, 200_000]) {
      const data = { text: "é\u0000😀\ud800".repeat(length), nested: [true, null, 2.5] };
      expect(runtime.extract_specs(JSON.stringify([data]))).toEqual([data]);
      expect(runtime.render_frontend(data)).toEqual(data);
      expect(runtime.render_popup(data)).toEqual(data);
      for (let i = 0; i < 10; i++) {
        const action = `action-${i}-é\u0000😀`;
        expect(runtime.handle_popup_action(action, data)).toEqual({ actionLength: action.length, data });
      }
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
