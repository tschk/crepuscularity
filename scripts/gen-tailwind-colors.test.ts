import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

for (const css of ["", "upstream unavailable", "--color-invalid: nope;"]) {
  test(`invalid theme preserves existing output: ${JSON.stringify(css)}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "crepus-palette-"));
    try {
      const theme = join(dir, "theme.css");
      const output = join(dir, "palette.rs");
      writeFileSync(theme, css);
      writeFileSync(output, "existing palette\n");
      const result = Bun.spawnSync([process.execPath, join(import.meta.dir, "gen-tailwind-colors.mjs"), "--theme", theme, output]);
      expect(result.exitCode).not.toBe(0);
      expect(new TextDecoder().decode(result.stderr)).toContain("theme contains no supported colors");
      expect(readFileSync(output, "utf8")).toBe("existing palette\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

test("HTTP errors preserve output even when the response contains colors", () => {
  const dir = mkdtempSync(join(tmpdir(), "crepus-palette-http-"));
  try {
    const output = join(dir, "palette.rs");
    const preload = join(dir, "fetch.ts");
    writeFileSync(output, "existing palette\n");
    writeFileSync(preload, 'globalThis.fetch = async () => new Response("--color-white: #fff;", { status: 503 });');
    const result = Bun.spawnSync([process.execPath, "--preload", preload, join(import.meta.dir, "gen-tailwind-colors.mjs"), output]);
    expect(result.exitCode).not.toBe(0);
    expect(new TextDecoder().decode(result.stderr)).toContain("theme download failed: HTTP 503");
    expect(readFileSync(output, "utf8")).toBe("existing palette\n");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("valid theme emits named and OKLCH colors", () => {
  const dir = mkdtempSync(join(tmpdir(), "crepus-palette-valid-"));
  try {
    const theme = join(dir, "theme.css");
    const output = join(dir, "palette.rs");
    writeFileSync(theme, "--color-white: #fff;\n--color-test-500: oklch(100% 0 0);");
    const result = Bun.spawnSync([process.execPath, join(import.meta.dir, "gen-tailwind-colors.mjs"), "--theme", theme, output]);
    expect(result.exitCode).toBe(0);
    const generated = readFileSync(output, "utf8");
    expect(generated).toContain('"test-500" => "#ffffff"');
    expect(generated).toContain('"white" => "#fff"');
    const harness = join(dir, "check.rs");
    const binary = join(dir, "check");
    writeFileSync(harness, '#[path = "palette.rs"] mod palette;\nfn main() { assert_eq!(palette::lookup_named_color("test-500"), Some("#ffffff")); assert_eq!(palette::lookup_named_color("unknown"), None); }');
    expect(Bun.spawnSync(["rustc", "--edition", "2021", "-D", "warnings", harness, "-o", binary]).exitCode).toBe(0);
    expect(Bun.spawnSync([binary]).exitCode).toBe(0);

  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

for (const [args, message] of [
  [["--theme"], "--theme requires a file path"],
  [["--theme", "--typo"], "--theme requires a file path"],
  [["--typo"], "unknown option: --typo"],
] as const) {
  test(`invalid arguments fail before retrieval: ${args.join(" ")}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "crepus-palette-args-"));
    try {
      const output = join(dir, "palette.rs");
      const preload = join(dir, "fetch.ts");
      writeFileSync(output, "existing palette\n");
      writeFileSync(preload, 'globalThis.fetch = async () => { throw new Error("unexpected fetch"); };');
      const result = Bun.spawnSync([process.execPath, "--preload", preload, join(import.meta.dir, "gen-tailwind-colors.mjs"), output, ...args]);
      expect(result.exitCode).not.toBe(0);
      expect(new TextDecoder().decode(result.stderr)).toContain(message);
      expect(readFileSync(output, "utf8")).toBe("existing palette\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

for (const hex of ["#", "#f", "#12", "#1234", "#12345", "#12345678"]) {
  test(`unsupported palette hex preserves output: ${hex}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "crepus-palette-hex-"));
    try {
      const theme = join(dir, "theme.css");
      const output = join(dir, "palette.rs");
      writeFileSync(theme, `--color-white: #fff;\n--color-invalid: ${hex};`);
      writeFileSync(output, "existing palette\n");
      const result = Bun.spawnSync([process.execPath, join(import.meta.dir, "gen-tailwind-colors.mjs"), "--theme", theme, output]);
      expect(result.exitCode).not.toBe(0);
      expect(new TextDecoder().decode(result.stderr)).toContain("unsupported palette hex color");
      expect(readFileSync(output, "utf8")).toBe("existing palette\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

for (const value of ["", "50garbage 0 0", "nope 0 0", "50% nope 0", "50% 0"]) {
  test(`invalid OKLCH preserves output: ${value}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "crepus-palette-oklch-"));
    try {
      const theme = join(dir, "theme.css");
      const output = join(dir, "palette.rs");
      writeFileSync(theme, `--color-white: #fff;\n--color-invalid-500: oklch(${value});`);
      writeFileSync(output, "existing palette\n");
      const result = Bun.spawnSync([process.execPath, join(import.meta.dir, "gen-tailwind-colors.mjs"), "--theme", theme, output]);
      expect(result.exitCode).not.toBe(0);
      expect(new TextDecoder().decode(result.stderr)).toContain("invalid palette OKLCH color");
      expect(readFileSync(output, "utf8")).toBe("existing palette\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

for (const css of ["--color-white: #fff;\n--color-white: #000;", "--color-test-500: oklch(50% 0 0);\n--color-test-500: oklch(100% 0 0);"]) {
  test(`duplicate palette keys preserve output: ${css}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "crepus-palette-duplicate-"));
    try {
      const theme = join(dir, "theme.css");
      const output = join(dir, "palette.rs");
      writeFileSync(theme, css);
      writeFileSync(output, "existing palette\n");
      const result = Bun.spawnSync([process.execPath, join(import.meta.dir, "gen-tailwind-colors.mjs"), "--theme", theme, output]);
      expect(result.exitCode).not.toBe(0);
      expect(new TextDecoder().decode(result.stderr)).toContain("duplicate palette color");
      expect(readFileSync(output, "utf8")).toBe("existing palette\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}
