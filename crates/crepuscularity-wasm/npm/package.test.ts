import { expect, test } from "bun:test";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Install only the declared publication files; direct source imports can mask
// missing JavaScript sidecars or a lost CommonJS boundary under Node.
for (const runtime of ["node", "bun"]) {
  test(`${runtime} imports the installed JavaScript package`, () => {
    const source = dirname(fileURLToPath(import.meta.url));
    const manifest = JSON.parse(readFileSync(join(source, "package.json"), "utf8"));
    const root = mkdtempSync(join(tmpdir(), "crepus-wasm-package-"));
    try {
      const installed = join(root, "node_modules", manifest.name);
      mkdirSync(installed, { recursive: true });
      cpSync(join(source, "package.json"), join(installed, "package.json"));
      for (const file of manifest.files) cpSync(join(source, file), join(installed, file), { recursive: true });
      const result = Bun.spawnSync([runtime, "--input-type=module", "--eval", `
        import assert from 'node:assert/strict';
        import { IR_VERSION, parseTemplate } from '@tschk/crepuscularity-wasm';
        import { loadParser } from '@tschk/crepuscularity-wasm/browser';
        import * as types from '@tschk/crepuscularity-wasm/types';
        assert.ok(import.meta.resolve('@tschk/crepuscularity-wasm').endsWith('/index.js'));
        assert.ok(import.meta.resolve('@tschk/crepuscularity-wasm/browser').endsWith('/browser.js'));
        assert.ok(import.meta.resolve('@tschk/crepuscularity-wasm/types').endsWith('/types.js'));
        assert.equal(typeof loadParser, 'function');
        assert.equal(typeof types, 'object');
        const ir = parseTemplate('<p>package-entry</p>', 'App.svelte');
        assert.equal(ir.version, IR_VERSION);
        assert.ok(JSON.stringify(ir).includes('package-entry'));
        assert.throws(() => parseTemplate('\\uD800', 'App.svelte'), /surrogate/);
      `], { cwd: root, stdout: "pipe", stderr: "pipe" });
      expect(new TextDecoder().decode(result.stderr)).toBe("");
      expect(result.exitCode).toBe(0);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}
