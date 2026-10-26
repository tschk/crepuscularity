import { expect, test } from "bun:test";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

test("pending frame URLs release on removal, load and failed assignment exactly once", async () => {
  GlobalRegistrator.register();
  const originals = {
    fetch: globalThis.fetch,
    chrome: globalThis.chrome,
    create: URL.createObjectURL,
    revoke: URL.revokeObjectURL,
  };
  const created = [];
  const revoked = [];
  let observer;
  try {
    globalThis.chrome = { runtime: { getURL: path => path } };
    globalThis.fetch = async () => ({ arrayBuffer: async () => new ArrayBuffer(0), text: async () => "" });
    URL.createObjectURL = () => {
      const url = `blob:test-${created.length}`;
      created.push(url);
      return url;
    };
    URL.revokeObjectURL = url => revoked.push(url);
    let source = await Bun.file(new URL("../assets/content.js", import.meta.url)).text();
    source = source.replace('import(`${runtimeApi.getURL("src/browser-shim.js")}?v=${cacheKey}`)', "Promise.resolve({browserApi:hostApi})")
      .replace("import(runtimeUrl)", "Promise.resolve(runtime)")
      .replace("  function createIframeMount(renderFrameDoc) {", "  expose(setIframeDocument);\n  function createIframeMount(renderFrameDoc) {")
      .replace("  observer.observe(document.documentElement", "  captureObserver(observer);\n  observer.observe(document.documentElement");
    let setDocument;
    await new Function("hostApi", "runtime", "expose", "captureObserver", `return ${source}`)(
      { runtime: { sendMessage: async () => ({ settings: { enabled: true } }) } },
      { default: async () => {} }, fn => { setDocument = fn; }, value => { observer = value; });
    const frame = () => {
      const node = document.createElement("iframe");
      Object.defineProperty(node, "src", { set() {} });
      return node;
    };
    const removed = frame();
    setDocument(removed, "pending");
    const wrapper = document.createElement("div");
    wrapper.append(removed);
    document.body.append(wrapper);
    wrapper.remove();
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(revoked).toEqual([created[0]]);
    removed.dispatchEvent(new Event("load"));
    expect(revoked).toHaveLength(1);
    const loaded = frame();
    document.body.append(loaded);
    setDocument(loaded, "loaded");
    loaded.dispatchEvent(new Event("load"));
    loaded.remove();
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(revoked).toEqual(created);
    const failed = document.createElement("iframe");
    Object.defineProperty(failed, "src", {
      set() { throw new Error("assignment failed"); },
    });
    setDocument(failed, "fallback");
    expect(failed.srcdoc).toBe("fallback");
    expect(revoked).toEqual(created);

    // Hold load delivery to exercise a still-pending document across reassignment
    // and a synchronous move; Happy DOM otherwise emits its own initial load.
    const moving = frame();
    moving.addEventListener("load", event => event.stopImmediatePropagation());
    document.body.append(moving);
    setDocument(moving, "first pending");
    const previousUrl = created.at(-1);
    setDocument(moving, "second pending");
    expect(revoked).toContain(previousUrl);
    const currentUrl = created.at(-1);
    const destination = document.createElement("div");
    document.body.append(destination);
    destination.append(moving);
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(revoked).not.toContain(currentUrl);
    destination.remove();
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(revoked).toEqual(created);

  } finally {
    observer?.disconnect();
    URL.createObjectURL = originals.create;
    URL.revokeObjectURL = originals.revoke;
    globalThis.fetch = originals.fetch;
    globalThis.chrome = originals.chrome;
    GlobalRegistrator.unregister();
  }
});
