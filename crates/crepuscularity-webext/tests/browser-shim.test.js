import { afterEach, expect, test } from "bun:test";

const originalBrowser = globalThis.browser;
const originalChrome = globalThis.chrome;
afterEach(() => {
  globalThis.browser = originalBrowser;
  globalThis.chrome = originalChrome;
});

async function load(browser, chrome) {
  globalThis.browser = browser;
  globalThis.chrome = chrome;
  const source = await Bun.file(new URL("../assets/browser-shim.js", import.meta.url)).text();
  // Evaluate the actual asset independently of popup.test.js's module mock.
  return new Function(source.replace("export const browserApi =", "const browserApi =") + "\nreturn browserApi;")();
}

function bounded(promise) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("shim did not settle")), 200); }),
  ]).finally(() => clearTimeout(timer));
}

test("Chrome zero-arity methods receive one callback and retain their receiver", async () => {
  const calls = [];
  const runtime = { onMessage: {}, getURL: (path) => path };
  const local = {};
  const sync = {};
  for (const [name, area] of [["local", local], ["sync", sync]]) {
    area.get = function (...args) {
      calls.push([name, "get", this === area, args.length]);
      args[1]?.({ name });
    };
    area.set = function (...args) {
      calls.push([name, "set", this === area, args.length]);
      args[1]?.();
    };
  }
  runtime.sendMessage = function (...args) {
    calls.push(["runtime", "sendMessage", this === runtime, args.length]);
    args[1]?.({ ok: true });
  };
  const api = await load(undefined, { runtime, storage: { local, sync } });
  for (const name of ["local", "sync"]) {
    expect(await bounded(api.storage[name].get("key"))).toEqual({ name });
    expect(await bounded(api.storage[name].set({ key: "value" }))).toBeUndefined();
  }
  expect(await bounded(api.runtime.sendMessage({ type: "test" }))).toEqual({ ok: true });
  expect(calls).toEqual([
    ["local", "get", true, 2], ["local", "set", true, 2],
    ["sync", "get", true, 2], ["sync", "set", true, 2],
    ["runtime", "sendMessage", true, 2],
  ]);
});

test("Promise browser namespace wins over Chrome regardless of function arity", async () => {
  const calls = [];
  function area(name) {
    const value = {};
    value.get = function (keys, unused) { calls.push([name, "get", this === value, arguments.length]); return Promise.resolve({ name }); };
    value.set = function (keys, unused) { calls.push([name, "set", this === value, arguments.length]); return Promise.resolve(); };
    return value;
  }
  const runtime = { onMessage: {}, getURL: (path) => path };
  runtime.sendMessage = function (payload, unused) { calls.push(["runtime", "sendMessage", this === runtime, arguments.length]); return Promise.resolve("response"); };
  const api = await load({ runtime, storage: { local: area("local"), sync: area("sync") } }, { runtime: { get lastError() { throw Error("Chrome must not be consulted"); } } });
  for (const name of ["local", "sync"]) {
    expect(await bounded(api.storage[name].get("key"))).toEqual({ name });
    await bounded(api.storage[name].set({ key: "value" }));
  }
  expect(await bounded(api.runtime.sendMessage({}))).toBe("response");
  expect(calls).toEqual([
    ["local", "get", true, 1], ["local", "set", true, 1],
    ["sync", "get", true, 1], ["sync", "set", true, 1],
    ["runtime", "sendMessage", true, 1],
  ]);
});

test("callback lastError and Promise failures propagate without retries", async () => {
  let calls = 0;
  const runtime = { onMessage: {}, getURL: (path) => path };
  runtime.sendMessage = function (...args) {
    calls++;
    runtime.lastError = { message: "callback denied" };
    args[1]?.();
    delete runtime.lastError;
  };
  const area = { get() {}, set() {} };
  let api = await load(undefined, { runtime, storage: { local: area, sync: area } });
  await expect(bounded(api.runtime.sendMessage({}))).rejects.toThrow("callback denied");
  expect(calls).toBe(1);
  const failure = new Error("promise denied");
  runtime.sendMessage = function () { calls++; return Promise.reject(failure); };
  api = await load({ runtime, storage: { local: area, sync: area } }, undefined);
  await expect(bounded(api.runtime.sendMessage({}))).rejects.toBe(failure);
  expect(calls).toBe(2);
});

for (const [label, configure] of [
  ["lastError getter", (runtime, failure) => Object.defineProperty(runtime, "lastError", { get() { throw failure; } })],
  ["message getter", (runtime, failure) => { runtime.lastError = { get message() { throw failure; } }; }],
  ["message conversion", (runtime, failure) => { runtime.lastError = { message: { toString() { throw failure; } } }; }],
]) {
  for (const operation of ["sendMessage", "local.get", "local.set", "sync.get", "sync.set"]) {
    test(`Chrome ${operation} rejects asynchronous ${label} failures`, async () => {
      const failure = new Error(label);
      const escaped = [];
      let calls = 0;
      function native(...args) {
        calls++;
        queueMicrotask(() => {
          // Observe exceptions escaping the native callback without creating an unhandled test error.
          try { args.at(-1)({ ok: true }); } catch (error) { escaped.push(error); }
        });
      }
      const runtime = { onMessage: {}, sendMessage: native, getURL: (path) => path };
      configure(runtime, failure);
      const area = { get: native, set: native };
      const api = await load(undefined, { runtime, storage: { local: area, sync: area } });
      const [name, method] = operation.split(".");
      const pending = method ? api.storage[name][method]({ key: "value" }) : api.runtime.sendMessage({});
      await expect(bounded(pending)).rejects.toBe(failure);
      expect(escaped).toEqual([]);
      expect(calls).toBe(1);
    });
  }
}
