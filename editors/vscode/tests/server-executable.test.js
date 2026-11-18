import { afterEach, expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { resolveServerExecutable } from "../src/server-executable";

const roots = [];
afterEach(async () => { await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true }))); });
async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), "crepus-lsp-resolver-"));
  roots.push(root);
  const bin = path.join(root, "bin");
  const workspace = path.join(root, "workspace");
  await mkdir(bin); await mkdir(path.join(workspace, "target", "debug"), { recursive: true });
  return { root, bin, workspace };
}
async function executable(file) { await writeFile(file, "#!/bin/sh\nexit 0\n"); await chmod(file, 0o755); return file; }

test("PATH executable wins even when a workspace fallback exists", async () => {
  const { bin, workspace } = await fixture();
  const installed = await executable(path.join(bin, "crepus-lsp"));
  await executable(path.join(workspace, "target", "debug", "crepus-lsp"));
  expect(await resolveServerExecutable(workspace, "", { platform: "linux", env: { PATH: bin } })).toBe(installed);
});
test("workspace fallback is used when PATH has no executable", async () => {
  const { bin, workspace } = await fixture();
  const local = await executable(path.join(workspace, "target", "debug", "crepus-lsp"));
  await writeFile(path.join(bin, "crepus-lsp"), "not executable");
  expect(await resolveServerExecutable(workspace, "", { platform: "linux", env: { PATH: bin } })).toBe(local);
});
test("explicit paths and command names remain authoritative", async () => {
  const { workspace } = await fixture();
  for (const configured of ["/custom/server with spaces", "custom-crepus-lsp"]) {
    expect(await resolveServerExecutable(workspace, configured, { env: { PATH: "" } })).toBe(configured);
  }
});
test("directories are rejected and missing server explains the configuration", async () => {
  const { bin, workspace } = await fixture();
  await mkdir(path.join(bin, "crepus-lsp"));
  await expect(resolveServerExecutable(workspace, "", { platform: "linux", env: { PATH: bin } })).rejects.toThrow("crepus.languageServerPath");
});
test("PATH-only lookup works without a workspace and preserves directory precedence", async () => {
  const { root, bin } = await fixture();
  const second = path.join(root, "second"); await mkdir(second);
  const installed = await executable(path.join(bin, "crepus-lsp"));
  await executable(path.join(second, "crepus-lsp"));
  expect(await resolveServerExecutable(undefined, "", { platform: "linux", env: { PATH: `${bin}:${second}` } })).toBe(installed);
});
test("Windows PATH/PATHEXT order finds native files, never shell scripts", async () => {
  const { root, bin, workspace } = await fixture();
  const second = path.join(root, "other bin"); await mkdir(second);
  await writeFile(path.join(bin, "crepus-lsp.CMD"), "echo no shell");
  const installed = path.join(second, "crepus-lsp.EXE"); await writeFile(installed, "native fixture");
  expect(await resolveServerExecutable(workspace, "", { platform: "win32", env: { PATH: `${bin};"${second}"`, PATHEXT: ".CMD;.EXE;.COM" } })).toBe(installed);
  const preferred = path.join(second, "crepus-lsp.COM"); await writeFile(preferred, "native fixture");
  expect(await resolveServerExecutable(workspace, "", { platform: "win32", env: { PATH: second, PATHEXT: ".com;.exe".toUpperCase() } })).toBe(preferred);
});
test("Windows local executable fallback uses .exe", async () => {
  const { bin, workspace } = await fixture();
  const local = path.join(workspace, "target", "debug", "crepus-lsp.exe"); await writeFile(local, "native fixture");
  expect(await resolveServerExecutable(workspace, "", { platform: "win32", env: { PATH: bin } })).toBe(local);
});
