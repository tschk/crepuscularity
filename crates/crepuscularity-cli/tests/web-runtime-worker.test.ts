import { expect, test } from "bun:test";
import { createRuntimeQueue } from "../assets/web/runtime-worker.js";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => { resolve = done; });
  return { promise, resolve };
}

test("initialization is lazy, shared, awaited once and exposes callable names", async () => {
  let loads = 0;
  const ready = deferred();
  const dispatch = createRuntimeQueue(async () => {
    loads++; await ready.promise;
    return { default() {}, value: 1, read: () => 7 };
  });
  expect(loads).toBe(0);
  const init = dispatch({ id: 1, kind: "init" });
  const call = dispatch({ id: 2, kind: "call", name: "read", args: [] });
  await Promise.resolve(); await Promise.resolve();
  expect(loads).toBe(1);
  ready.resolve();
  expect(await init).toEqual({ exports: ["read"] });
  expect(await call).toBe(7);
  expect(loads).toBe(1);
});

test("async handler and selected render are one FIFO transaction", async () => {
  const gate = deferred();
  const trace: string[] = [];
  let state = 0;
  const dispatch = createRuntimeQueue(async () => ({
    async change(json: string) { trace.push(`start:${json}`); await gate.promise; state++; trace.push("changed"); },
    render(prefix: string) { trace.push(`render:${state}`); return prefix + state; },
    read() { trace.push("later"); return state; },
  }));
  const event = dispatch({ kind: "event", name: "change", eventJson: "payload", render: { name: "render", args: ["html:"] } });
  const next = dispatch({ kind: "call", name: "read", args: [] });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(trace).toEqual(["start:payload"]);
  gate.resolve();
  expect(await event).toBe("html:1");
  expect(await next).toBe(1);
  expect(trace).toEqual(["start:payload", "changed", "render:1", "later"]);
});

test("handler rejection skips render, is not replayed and does not poison FIFO", async () => {
  let calls = 0;
  let renders = 0;
  const dispatch = createRuntimeQueue(async () => ({ fail() { calls++; throw new Error("failed"); }, render() { renders++; }, read: () => 9 }));
  const failed = dispatch({ kind: "event", name: "fail", eventJson: "{}", render: { name: "render", args: [] } });
  const next = dispatch({ kind: "call", name: "read", args: [] });
  await expect(failed).rejects.toThrow("failed");
  expect(await next).toBe(9);
  expect(calls).toBe(1); expect(renders).toBe(0);
});

test("missing and inherited exports reject while later commands remain usable", async () => {
  const dispatch = createRuntimeQueue(async () => ({ read: () => "ok" }));
  await expect(dispatch({ kind: "call", name: "missing", args: [] })).rejects.toThrow("Unknown runtime export");
  await expect(dispatch({ kind: "call", name: "toString", args: [] })).rejects.toThrow("Unknown runtime export");
  expect(await dispatch({ kind: "event", name: "read", eventJson: "{}", render: null })).toBeUndefined();
  expect(await dispatch({ kind: "call", name: "read", args: [] })).toBe("ok");
});

test("initialization failure is cached without retries", async () => {
  let attempts = 0;
  const dispatch = createRuntimeQueue(async () => { attempts++; throw new Error("init failed"); });
  await expect(dispatch({ kind: "init" })).rejects.toThrow("init failed");
  await expect(dispatch({ kind: "call", name: "read", args: [] })).rejects.toThrow("init failed");
  expect(attempts).toBe(1);
});

test("render serializes structured arguments in the queue, including event renders", async () => {
  const inputs: unknown[][] = [];
  const dispatch = createRuntimeQueue(async () => ({
    change() {},
    render(...args: unknown[]) { inputs.push(args); return args[0]; },
  }));
  const spec = { name: "render", args: [{ root: ["雪", null] }, 3], json: true };
  expect(await dispatch({ kind: "render", render: spec })).toBe('{"root":["雪",null]}');
  expect(await dispatch({ kind: "event", name: "change", eventJson: "{}", render: spec })).toBe('{"root":["雪",null]}');
  expect(inputs).toEqual([['{"root":["雪",null]}', "3"], ['{"root":["雪",null]}', "3"]]);
  await dispatch({ kind: "render", render: { name: "render", args: [3] } });
  expect(inputs[2]).toEqual([3]);
});

test("render rejection preserves completed handler state and releases the queue", async () => {
  let state = 0;
  const dispatch = createRuntimeQueue(async () => ({ change() { state++; }, render() { throw new Error("render failed"); }, read: () => state }));
  await expect(dispatch({ kind: "event", name: "change", eventJson: "{}", render: { name: "render", args: [] } })).rejects.toThrow("render failed");
  expect(await dispatch({ kind: "call", name: "read", args: [] })).toBe(1);
});
