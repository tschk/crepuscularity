import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

let frames;
let observers;
let originalFetch;
let originalChrome;
let originalRaf;
let originalObserver;
let originalWarn;

beforeEach(() => {
  GlobalRegistrator.register();
  frames = [];
  observers = [];
  originalFetch = globalThis.fetch;
  originalChrome = globalThis.chrome;
  originalRaf = globalThis.requestAnimationFrame;
  originalObserver = globalThis.MutationObserver;
  originalWarn = console.warn;
  console.warn = () => {};
  globalThis.MutationObserver = class extends originalObserver {
    constructor(callback) {
      super(callback);
      observers.push(this);
    }
  };
  globalThis.requestAnimationFrame = (callback) => frames.push(callback);
  globalThis.chrome = { runtime: { getURL: (path) => path } };
  globalThis.fetch = async () => ({
    arrayBuffer: async () => new ArrayBuffer(0),
    text: async () => "",
  });
});

afterEach(() => {
  for (const observer of observers) observer.disconnect();
  globalThis.fetch = originalFetch;
  globalThis.chrome = originalChrome;
  globalThis.requestAnimationFrame = originalRaf;
  globalThis.MutationObserver = originalObserver;
  console.warn = originalWarn;
  GlobalRegistrator.unregister();
});

async function flush() {
  // Happy DOM delivers mutation records asynchronously. RAF itself is driven
  // explicitly so assertions observe completed observer and render batches.
  await new Promise((resolve) => setTimeout(resolve, 10));
  for (const callback of frames.splice(0)) callback();
  await new Promise((resolve) => setTimeout(resolve, 10));
}

async function start(runtime, browserApi = {
  runtime: { sendMessage: async () => ({ settings: { enabled: true } }) },
}) {
  let source = await Bun.file(new URL("../assets/content.js", import.meta.url)).text();
  source = source
    .replace('import(`${runtimeApi.getURL("src/browser-shim.js")}?v=${cacheKey}`)', "Promise.resolve({browserApi:hostApi})")
    .replace("import(runtimeUrl)", "Promise.resolve(runtime)");
  await new Function("hostApi", "runtime", `return ${source}`)(browserApi, {
    default: async () => {},
    ...runtime,
  });
  await flush();
}

test("browser program bindings preserve special names and do not inherit missing variables", async () => {
  const messages = [];
  const program = {
    bindings: [
      { type: "storage_get", area: "local", key: "value", name: "__proto__" },
      { type: "runtime_message", name: "constructor", payload: { request: true } },
    ],
    statements: [{ type: "runtime_send", payload: {
      stored: { $var: "__proto__" }, response: { $var: "constructor" },
      missing: { $var: "toString" },
    } }],
  };
  await start({ browser_program_data: () => JSON.stringify(program) }, {
    storage: { local: { get: async () => ({ value: 42 }) } },
    runtime: { sendMessage: async (message) => {
      if (message.request) return "reply";
      messages.push(message);
      return { settings: { enabled: true } };
    } },
  });
  const message = messages.find((value) => Object.hasOwn(value, "stored"));
  expect(message).toEqual({ stored: 42, response: "reply", missing: undefined });
});

test("browser program member expressions retain literal property names and distinct dotted bindings", async () => {
  const messages = [];
  const writes = [];
  const member = { $member: ["object", "a-b"] };
  const program = {
    bindings: [
      { type: "storage_get", area: "local", key: "object", name: "object" },
      { type: "storage_get", area: "local", key: "dotted", name: "object.a-b" },
    ],
    statements: [
      { type: "storage_set", area: "local", key: "selected", value: member },
      { type: "runtime_send", payload: { member, dotted: { $var: "object.a-b" } } },
    ],
  };
  await start({ browser_program_data: () => JSON.stringify(program) }, {
    storage: { local: {
      get: async () => ({ object: { "a-b": 7 }, dotted: 9 }),
      set: async (value) => writes.push(value),
    } },
    runtime: { sendMessage: async (message) => {
      messages.push(message);
      return { settings: { enabled: true } };
    } },
  });
  expect(writes).toEqual([{ selected: 7 }]);
  expect(messages.find((value) => Object.hasOwn(value, "member"))).toEqual({ member: 7, dotted: 9 });
});

test("iframe resize uses one weakly owned listener across removal and remount", async () => {
  const addListener = spyOn(window, "addEventListener");
  // Happy DOM cannot navigate Blob URLs; use the existing srcdoc fallback here.
  const createObjectURL = spyOn(URL, "createObjectURL").mockImplementation(() => {
    throw new Error("Blob navigation unavailable in this DOM test");
  });
  const addWidget = () => {
    const pre = document.createElement("pre");
    pre.textContent = "<ai-anywhere complete></ai-anywhere>";
    document.body.append(pre);
  };
  try {
    for (let i = 0; i < 20; i++) addWidget();
    await start({
      extract_widgets: () => [{}],
      render_anywhere_parts: () => ({ html: "<p>Frame</p>", needs_iframe: true }),
      render_anywhere_frame_doc: () => ({ srcdoc: "<p>Frame</p>" }),
    });
    const mountedFrames = [...document.querySelectorAll("iframe.aa-widget-frame")];
    expect(mountedFrames).toHaveLength(20);
    const messageListeners = () => addListener.mock.calls.filter(([type]) => type === "message");
    expect(messageListeners()).toHaveLength(1);
    const resize = (frame, height, type = "anywhere-resize") => window.dispatchEvent(
      new MessageEvent("message", { source: frame.contentWindow, data: { type, height } }),
    );
    const [first, second] = mountedFrames;
    resize(first, 12.5);
    expect(first.style.height).toBe("13px");
    expect(second.style.height).toBe("");
    for (const value of [0, -1, Infinity, NaN, "invalid"]) resize(first, value);
    resize(first, 50, "unrelated");
    window.dispatchEvent(new MessageEvent("message", { data: { type: "anywhere-resize", height: 99 } }));
    expect(first.style.height).toBe("13px");
    const removedWindow = first.contentWindow;
    for (const frame of mountedFrames) frame.remove();
    window.dispatchEvent(new MessageEvent("message", { source: removedWindow, data: { type: "anywhere-resize", height: 88 } }));
    expect(first.style.height).toBe("13px");
    addWidget();
    await flush();
    const next = document.querySelector("iframe.aa-widget-frame");
    expect(next).not.toBeNull();
    resize(next, 42);
    expect(next.style.height).toBe("42px");
    expect(messageListeners()).toHaveLength(1);
  } finally {
    addListener.mockRestore();
    createObjectURL.mockRestore();
  }
});
