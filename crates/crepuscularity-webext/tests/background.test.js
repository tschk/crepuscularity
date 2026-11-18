import { afterEach, expect, test } from "bun:test";

const originalChrome = globalThis.chrome;
const originalBrowser = globalThis.browser;
const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.chrome = originalChrome;
  globalThis.browser = originalBrowser;
  globalThis.fetch = originalFetch;
});

async function host(initial, { deferStartupRead = false, runtime = {} } = {}) {
  const saved = { ...initial };
  let listener;
  let failGet = false;
  let failSet = false;
  let getError;
  let setError;
  let startup = true;
  let releaseStartupRead = () => {};
  let installed;
  const writes = [];
  const storage = {
    async get(defaults) {
      if (failGet) throw getError;
      const snapshot = Object.fromEntries(Object.entries(defaults).map(([key, fallback]) => [key, key in saved ? saved[key] : fallback]));
      if (startup && deferStartupRead) {
        return new Promise((resolve) => { releaseStartupRead = () => resolve(snapshot); });
      }
      return snapshot;
    },
    async set(delta) {
      if (failSet) throw setError;
      writes.push({ ...delta });
      Object.assign(saved, delta);
    },
  };
  const event = { addListener() {} };
  globalThis.browser = undefined;
  globalThis.chrome = {
    runtime: {
      onMessage: { addListener(callback) { listener = callback; } },
      onInstalled: { addListener(callback) { installed = callback; } },
    },
    storage: { onChanged: event },
  };
  globalThis.fetch = async () => ({ arrayBuffer: async () => new ArrayBuffer(0) });
  const source = (await Bun.file(new URL("../assets/background.js", import.meta.url)).text())
    .replace('import { browserApi } from "./browser-shim.js";', "")
    .replace('import init, * as runtimeModule from "../vendor/runtime.js";', "");
  new Function("browserApi", "init", "runtimeModule", source)(
    { storage: { sync: storage } }, async () => {}, runtime
  );
  startup = false;
  // Let startup work finish unless this test deliberately holds its read.
  await new Promise((resolve) => setTimeout(resolve, 0));
  return {
    saved,
    writes,
    releaseStartupRead() { releaseStartupRead(); },
    async installed() { installed(); await new Promise((resolve) => setTimeout(resolve, 0)); },
    failGet(error) { failGet = true; getError = arguments.length ? error : new Error("get denied"); },
    failSet(error) { failSet = true; setError = arguments.length ? error : new Error("set denied"); },
    message(payload) {
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("background did not respond")), 200);
        const retained = listener(payload, {}, (response) => { clearTimeout(timer); resolve(response); });
        expect(retained).toBe(true);
      });
    },
  };
}

test("runtime failures with unprintable thrown values still receive a reply", async () => {
  const app = await host({}, { runtime: {
    handle_background_message() { throw Object.create(null); },
  } });
  expect(await app.message({ type: "custom" })).toEqual({
    ok: false, error: "Runtime threw an unprintable value",
  });
  expect((await app.message({ type: "settings:get" })).ok).toBe(true);
});

test("partial settings updates preserve saved omitted values and return latest settings", async () => {
  const app = await host({ enabled: false, autoRender: true, extra: "retained" });
  const response = await app.message({ type: "settings:set", settings: { autoRender: false } });
  expect(app.saved).toEqual({ enabled: false, autoRender: false, extra: "retained" });
  expect(response).toEqual({ ok: true, settings: { enabled: false, autoRender: false } });
  expect(await app.message({ type: "settings:get" })).toEqual({ ok: true, settings: { enabled: false, autoRender: false } });
});

test("concurrent disjoint partial updates do not overwrite one another", async () => {
  const app = await host({ enabled: true, autoRender: true, extra: "retained" });
  const responses = await Promise.all([
    app.message({ type: "settings:set", settings: { enabled: false } }),
    app.message({ type: "settings:set", settings: { autoRender: false } }),
  ]);
  expect(app.saved).toEqual({ enabled: false, autoRender: false, extra: "retained" });
  for (const response of responses) expect(response).toEqual({ ok: true, settings: { enabled: false, autoRender: false } });
});

test("settings get supplies defaults and reports storage failures", async () => {
  const app = await host({});
  expect(await app.message({ type: "settings:get" })).toEqual({
    ok: true, settings: { enabled: true, autoRender: false },
  });
  app.failGet();
  expect(await app.message({ type: "settings:get" })).toEqual({ ok: false, error: "get denied" });
  expect(await app.message({ type: "settings:set", settings: { enabled: false } })).toEqual({ ok: false, error: "get denied" });
});

test("settings set failure returns an error without changing stored values", async () => {
  const app = await host({ enabled: false, autoRender: true });
  app.failSet();
  expect(await app.message({ type: "settings:set", settings: { autoRender: false } })).toEqual({ ok: false, error: "set denied" });
  expect(app.saved).toEqual({ enabled: false, autoRender: true });
});

test("startup and installation do not write defaults over saved preferences", async () => {
  const app = await host({ enabled: false, autoRender: true, extra: "retained" });
  expect(app.writes).toEqual([]);
  await app.installed();
  expect(app.writes).toEqual([]);
  expect(app.saved).toEqual({ enabled: false, autoRender: true, extra: "retained" });
});

test("a delayed startup snapshot cannot overwrite a newer message update", async () => {
  const app = await host({ enabled: true, autoRender: true }, { deferStartupRead: true });
  const response = await app.message({ type: "settings:set", settings: { enabled: false } });
  expect(response.ok).toBe(true);
  expect(app.saved.enabled).toBe(false);
  app.releaseStartupRead();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(app.saved.enabled).toBe(false);
  expect(app.writes).toEqual([{ enabled: false }]);
});

for (const [label, failure, expected] of [
  ["null", () => null, "null"],
  ["undefined", () => undefined, "undefined"],
  ["string", () => "denied", "denied"],
  ["null prototype", () => Object.create(null), "Storage operation failed"],
  ["throwing message", () => ({ get message() { throw new Error("getter"); }, toString() { return "readable"; } }), "readable"],
  ["unprintable message", () => ({ message: Object.create(null), toString() { return "readable"; } }), "readable"],
  ["throwing conversion", () => new Proxy({}, { get() { throw new Error("getter"); } }), "Storage operation failed"],
]) {
  for (const operation of ["get", "set", "read after set"]) {
    test(`settings ${operation} replies to ${label} storage rejection`, async () => {
      const app = await host({ enabled: false });
      if (operation === "set") app.failSet(failure());
      else app.failGet(failure());
      const request = operation === "get"
        ? { type: "settings:get" }
        : { type: "settings:set", settings: { enabled: true } };
      expect(await app.message(request)).toEqual({ ok: false, error: expected });
      expect(app.saved.enabled).toBe(operation === "read after set");
    });
  }
}
