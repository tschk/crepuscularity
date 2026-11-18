import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const tick = () => Bun.sleep(0);
function extension({ gatedStart = false, rejectFirstStop = false, rejectEveryStop = false } = {}) {
  let stopAttempts = 0;
  const resolutions = [], instances = [], errors = [], subscriptions = [];
  let restart;
  const vscode = {
    workspace: { workspaceFolders: [], getConfiguration: () => ({ get: () => "" }) },
    window: { showErrorMessage: error => { errors.push(error); } },
    commands: { registerCommand: (_name, run) => { restart = run; return { dispose() {} }; } },
    Disposable: class { constructor(dispose) { this.dispose = dispose; } },
  };
  class LanguageClient {
    constructor(_id, _name, options) {
      this.command = options.run.command;
      this.started = false; this.stops = 0; this.gate = deferred(); instances.push(this);
    }
    async start() { if (gatedStart) await this.gate.promise; this.started = true; }
    async stop() {
      if (!this.started) throw new Error("stopped during startup");
      this.stops++;
      if (rejectEveryStop || (rejectFirstStop && stopAttempts++ === 0)) throw new Error("stop failed");
    }
  }
  // Actual extension module; only framework and resolver collaborators are injected.
  const source = readFileSync(new URL("../src/extension.ts", import.meta.url), "utf8")
    .replace(/^import .* from "\.\/server-executable";\n/m, "")
    .replace(/^import \* as vscode from "vscode";\n/m, "")
    .replace(/import \{[\s\S]*?\} from "vscode-languageclient\/node";\n/, "")
    .replace(/^export /gm, "");
  const js = new Bun.Transpiler({ loader: "ts" }).transformSync(source);
  const actual = new Function("vscode", "LanguageClient", "TransportKind", "resolveServerExecutable", js + "; return {activate, deactivate};")(
    vscode, LanguageClient, { stdio: 0 }, () => { const gate = deferred(); resolutions.push(gate); return gate.promise; });
  actual.activate({ subscriptions });
  return { ...actual, resolutions, instances, errors, subscriptions, restart: () => restart() };
}
test("deactivate/dispose prevents pending resolver from launching", async () => {
  for (const dispose of [false, true]) {
    const h = extension();
    if (dispose) h.subscriptions[1].dispose(); else await h.deactivate();
    h.resolutions[0].resolve("old"); await tick();
    expect(h.instances).toHaveLength(0); await h.deactivate();
  }
});
test("deactivate/dispose suppresses late resolver failure", async () => {
  for (const dispose of [false, true]) {
    const h = extension();
    if (dispose) h.subscriptions[1].dispose(); else await h.deactivate();
    h.resolutions[0].reject(new Error("old")); await tick();
    expect(h.instances).toHaveLength(0); expect(h.errors).toHaveLength(0); await h.deactivate();
  }
});
test("restart launches only newer resolver completion", async () => {
  const h = extension(); const restarted = h.restart(); await tick(); expect(h.resolutions).toHaveLength(2);
  h.resolutions[1].resolve("new"); await restarted; h.resolutions[0].resolve("old"); await tick();
  expect(h.instances.map(i => i.command)).toEqual(["new"]); await h.deactivate(); expect(h.instances[0].stops).toBe(1);
});
test("restart suppresses older resolver rejection", async () => {
  const h = extension(); const restarted = h.restart(); await tick(); h.resolutions[0].reject(new Error("old")); await tick();
  expect(h.errors).toHaveLength(0); h.resolutions[1].resolve("new"); await restarted; await h.deactivate();
});
test("deactivate/dispose waits for start and stops once", async () => {
  const h = extension({ gatedStart: true }); h.resolutions[0].resolve("server"); await tick();
  const stopped = h.deactivate(); h.subscriptions[1].dispose(); await tick(); expect(h.instances[0].stops).toBe(0);
  h.instances[0].gate.resolve(); await stopped; expect(h.instances[0].stops).toBe(1); expect(h.errors).toHaveLength(0);
});
test("late client startup rejection after deactivate is silent", async () => {
  const h = extension({ gatedStart: true }); h.resolutions[0].resolve("server"); await tick(); const stopped = h.deactivate();
  h.instances[0].gate.reject(new Error("startup failed")); await stopped; await tick();
  expect(h.instances[0].stops).toBe(0); expect(h.errors).toHaveLength(0);
});
test("current resolver failure still reports setting guidance", async () => {
  const h = extension(); h.resolutions[0].reject(new Error("set crepus.languageServerPath")); await tick();
  expect(h.errors).toHaveLength(1); expect(h.errors[0]).toContain("crepus.languageServerPath"); await h.deactivate();
});

test("a rejected shutdown does not permanently block later restart", async () => {
  const h = extension({ rejectFirstStop: true });
  h.resolutions[0].resolve("first"); await tick();
  await h.restart();
  expect(h.errors).toHaveLength(1);
  const restarted = h.restart(); await tick();
  expect(h.resolutions).toHaveLength(2);
  expect(h.instances[0].stops).toBe(2);
  h.resolutions[1].resolve("replacement"); await restarted;
  expect(h.instances.map(i => i.command)).toEqual(["first", "replacement"]);
  await h.deactivate();
});

test("overlapping restart retries restored client before replacing it", async () => {
  const h = extension({ rejectFirstStop: true });
  h.resolutions[0].resolve("first"); await tick();
  const older = h.restart();
  const newer = h.restart();
  await tick();
  expect(h.instances[0].stops).toBe(2);
  expect(h.instances).toHaveLength(1);
  expect(h.resolutions).toHaveLength(2);
  h.resolutions[1].resolve("replacement");
  await Promise.all([older, newer]);
  expect(h.instances.map(i => i.command)).toEqual(["first", "replacement"]);
  expect(h.errors).toHaveLength(0);
  await h.deactivate();
});

test("deactivate/dispose retries a failed in-flight restart shutdown", async () => {
  for (const dispose of [false, true]) {
    const h = extension({ rejectFirstStop: true });
    h.resolutions[0].resolve("first"); await tick();
    const restarted = h.restart();
    if (dispose) h.subscriptions[1].dispose(); else await h.deactivate();
    await restarted; await tick();
    expect(h.instances[0].stops).toBe(2);
    expect(h.instances).toHaveLength(1);
    expect(h.resolutions).toHaveLength(1);
    expect(h.errors).toHaveLength(0);
    await h.deactivate();
  }
});
test("persistent shutdown failure rejects deactivation without spinning", async () => {
  const h = extension({ rejectEveryStop: true });
  h.resolutions[0].resolve("first"); await tick();
  const restarted = h.restart();
  await expect(h.deactivate()).rejects.toThrow("stop failed");
  await restarted;
  expect(h.instances[0].stops).toBe(2);
});
