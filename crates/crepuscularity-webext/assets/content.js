(async () => {
  const runtimeApi = globalThis.browser?.runtime ?? globalThis.chrome?.runtime;
  if (!runtimeApi) {
    throw new Error("browser runtime API is unavailable");
  }

  const topFrame = (() => {
    try {
      return globalThis.top === globalThis;
    } catch (_) {
      return false;
    }
  })();

  const cacheKey = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const runtimeUrl = `${runtimeApi.getURL("vendor/runtime.js")}?v=${cacheKey}`;
  const wasmUrl = `${runtimeApi.getURL("vendor/runtime_bg.wasm")}?v=${cacheKey}`;

  let browserApi;
  let wasmModule;

  try {
    [{ browserApi }, wasmModule] = await Promise.all([
      import(`${runtimeApi.getURL("src/browser-shim.js")}?v=${cacheKey}`),
      import(runtimeUrl)
    ]);

    const wasmBytes = await fetch(wasmUrl, { cache: "no-store" }).then((response) => response.arrayBuffer());
    await wasmModule.default({ module_or_path: wasmBytes });
  } catch (error) {
    if (topFrame) throw error;
    if (!(error instanceof WebAssembly.CompileError)) throw error;
    return;
  }

  const settingsResponse = await browserApi.runtime.sendMessage({ type: "settings:get" });
  const settings = settingsResponse?.settings ?? { enabled: true, autoRender: false };
  if (!settings.enabled) {
    return;
  }

  const unocssSource = await fetch(`${runtimeApi.getURL("vendor/unocss.js")}?v=${cacheKey}`)
    .then((response) => response.text())
    .catch(() => "");

  if (typeof wasmModule.browser_program_data === "function") {
    runBrowserProgramData(browserApi, JSON.parse(wasmModule.browser_program_data()))
      .catch((error) => console.error("crepuscularity browser program failed", error));
  }

  const mounted = new WeakSet();
  const pending = new Set();
  let flushScheduled = false;

  function normalizeWidgetText(text) {
    return text
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">")
      .replace(/&amp;/gi, "&");
  }

  function hasAnywhereContent(node) {
    const text = normalizeWidgetText(node.textContent || "");
    const html = node.innerHTML || "";
    return (
      text.includes("```") ||
      text.includes("<ai-anywhere") ||
      html.includes("<ai-anywhere") ||
      html.includes("&lt;ai-anywhere")
    );
  }

  function widgetTextFromPre(pre) {
    const code = pre.querySelector("code");
    const el = code ?? pre;
    let text = normalizeWidgetText(el.textContent || "");
    if (!text.includes("ai-anywhere") && el.innerHTML) {
      text = normalizeWidgetText(
        el.innerHTML
          .replace(/<br\s*\/?>/gi, "\n")
          .replace(/<[^>]+>/g, "")
      );
    }
    return stripOuterCodeFence(text);
  }

  function wasmArray(value) {
    if (Array.isArray(value)) {
      return value;
    }
    if (value && typeof value.length === "number") {
      return Array.from(value);
    }
    return [];
  }

  function stripOuterCodeFence(text) {
    const trimmed = text.trim();
    const match = /^```[\w-]*\s*\n([\s\S]*?)\n```\s*$/i.exec(trimmed);
    return match ? match[1].trim() : trimmed;
  }

  function collectWidgetPres() {
    const pres = [];
    const nodes = document.getElementsByTagName("pre");
    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      if (node instanceof HTMLElement && hasAnywhereContent(node)) {
        pres.push(node);
      }
    }
    return pres.filter(
      (pre) => !pres.some((other) => other !== pre && other.contains(pre))
    );
  }

  function dedupeNodes(nodes) {
    const list = [...nodes];
    return list.filter((node) => !list.some((other) => other !== node && other.contains(node)));
  }

  function wasmField(value, key) {
    if (value == null) return undefined;
    if (value instanceof Map) return value.get(key);
    if (typeof value === "object") return value[key];
    return undefined;
  }

  function wasmPlain(value) {
    if (value == null) return value;
    if (value instanceof Map) {
      return Object.fromEntries([...value.entries()].map(([k, v]) => [k, wasmPlain(v)]));
    }
    if (Array.isArray(value)) {
      return value.map((item) => wasmPlain(item));
    }
    return value;
  }

  async function runBrowserProgramData(api, program) {
    const vars = {};

    function resolveExpr(expr) {
      if (expr !== null && typeof expr === "object" && "$var" in expr) return vars[expr.$var];
      return expr;
    }
    function resolveObj(obj) {
      if (typeof obj !== "object" || obj === null) return obj;
      return Object.fromEntries(Object.entries(obj).map(([k, v]) => [k, resolveExpr(v)]));
    }

    for (const b of (program.bindings ?? [])) {
      if (b.type === "storage_get") {
        const res = await api.storage[b.area].get({ [b.key]: undefined }).catch(() => ({}));
        vars[b.name] = res[b.key];
      } else if (b.type === "runtime_message") {
        vars[b.name] = await api.runtime.sendMessage(resolveObj(b.payload)).catch(() => null);
      }
    }

    for (const s of (program.statements ?? [])) {
      if (s.type === "storage_set") {
        await api.storage[s.area].set({ [s.key]: resolveExpr(s.value) }).catch(() => {});
      } else if (s.type === "runtime_send") {
        await api.runtime.sendMessage(resolveObj(s.payload)).catch(() => {});
      } else if (s.type === "console_log") {
        console.log(...(s.args ?? []).map(resolveExpr));
      }
    }
  }

  function codeBlockReplaceTarget(pre) {
    return pre;
  }

  function mountWidgetShell(pre, shell) {
    if (!(pre instanceof HTMLElement) || mounted.has(pre)) {
      return;
    }
    mounted.add(pre);

    const wrapper = document.createElement("div");
    wrapper.className = "aa-mount";
    wrapper.append(shell);

    const replaceTarget = codeBlockReplaceTarget(pre);
    if (replaceTarget?.isConnected) {
      replaceTarget.replaceWith(wrapper);
      return;
    }
    console.warn("crepuscularity: code block not connected, skip mount");
  }

  function frameSrcdoc(frameDoc) {
    const plain = wasmPlain(frameDoc);
    if (typeof plain?.srcdoc === "string" && plain.srcdoc.length > 0) {
      return plain.srcdoc;
    }
    const direct = wasmField(frameDoc, "srcdoc");
    if (typeof direct === "string" && direct.length > 0) {
      return direct;
    }
    if (frameDoc && typeof frameDoc === "object") {
      try {
        const reflected = Reflect.get(frameDoc, "srcdoc");
        if (typeof reflected === "string" && reflected.length > 0) {
          return reflected;
        }
      } catch (_) {
        // ignore
      }
    }
    return null;
  }

  const INLINE_HOST_CSS = `
:host {
  display: block;
  background: #fffdf8;
  color: #111;
  font-family: "IBM Plex Sans", system-ui, "Segoe UI", sans-serif;
}
.aa-widget-root {
  padding: 16px;
}
`;

  function attachFrameResize(frame) {
    window.addEventListener("message", (event) => {
      if (event.source !== frame.contentWindow) {
        return;
      }
      if (event.data?.type !== "anywhere-resize") {
        return;
      }
      const height = Number(event.data.height);
      if (Number.isFinite(height) && height > 0) {
        frame.style.height = `${Math.ceil(height)}px`;
      }
    });
  }

  function setIframeDocument(frame, srcdoc) {
    frame.removeAttribute("src");
    try {
      const blob = new Blob([srcdoc], { type: "text/html;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      frame.src = url;
      frame.addEventListener(
        "load",
        () => {
          URL.revokeObjectURL(url);
        },
        { once: true }
      );
    } catch (_) {
      frame.srcdoc = srcdoc;
    }
  }

  function createIframeMount(renderFrameDoc) {
    let frameDoc;
    try {
      frameDoc = renderFrameDoc();
    } catch (error) {
      console.error("crepuscularity: failed to render widget frame", error);
      return null;
    }
    const srcdoc = frameSrcdoc(frameDoc);
    if (!srcdoc) {
      console.error("crepuscularity: frame render returned no srcdoc", frameDoc);
      return null;
    }

    const mount = document.createElement("div");
    mount.className = "aa-shell";
    const frame = document.createElement("iframe");
    frame.className = "aa-widget-frame";
    frame.setAttribute("scrolling", "no");
    frame.sandbox = "allow-scripts";
    setIframeDocument(frame, srcdoc);
    attachFrameResize(frame);
    mount.append(frame);
    return mount;
  }

  function sanitizeHTML(html) {
    // ponytail: build DOM nodes directly, no innerHTML serialization = no mXSS
    const parser = new DOMParser();
    const doc = parser.parseFromString(html, "text/html");

    const ALLOWED_TAGS = new Set([
      "DIV", "SPAN", "P", "B", "I", "EM", "STRONG", "A", "UL", "OL", "LI",
      "H1", "H2", "H3", "H4", "H5", "H6", "BR", "HR", "TABLE", "THEAD",
      "TBODY", "TR", "TH", "TD", "BLOCKQUOTE", "PRE", "CODE"
    ]);

    function cleanNode(node) {
      if (node.nodeType === Node.TEXT_NODE) return node.cloneNode();
      if (node.nodeType !== Node.ELEMENT_NODE) return null;

      const nodeName = node.nodeName.toUpperCase();
      if (!ALLOWED_TAGS.has(nodeName)) return null;

      const clone = document.createElement(nodeName);
      for (const attr of node.attributes) {
        const name = attr.name.toLowerCase();
        const value = attr.value.trim().toLowerCase();

        if (name.startsWith("on")) continue;

        if (name === "href" || name === "src") {
          const isSafeUrl = value.startsWith("http://") ||
                            value.startsWith("https://") ||
                            value.startsWith("mailto:") ||
                            value.startsWith("#") ||
                            value.startsWith("/") && !value.startsWith("//");
          if (!isSafeUrl) continue;
        }
        clone.setAttribute(attr.name, attr.value);
      }

      for (const child of node.childNodes) {
        const cleaned = cleanNode(child);
        if (cleaned) clone.appendChild(cleaned);
      }
      return clone;
    }

    const fragment = document.createDocumentFragment();
    for (const child of doc.body.childNodes) {
      const cleaned = cleanNode(child);
      if (cleaned) fragment.appendChild(cleaned);
    }
    return fragment;
  }

  function createInlineAnywhereMount(widget) {
    let parts;
    try {
      parts = wasmPlain(
        wasmModule.render_anywhere_parts({ widget, unocss: unocssSource })
      );
    } catch (error) {
      console.error("crepuscularity: failed to render anywhere parts", error);
      return null;
    }
    const html = parts?.html;
    if (typeof html !== "string" || html.length === 0) {
      console.error("crepuscularity: anywhere parts returned no html", parts);
      return null;
    }
    if (parts.needs_iframe || (typeof parts.js === "string" && parts.js.length > 0)) {
      return createIframeMount(() =>
        wasmModule.render_anywhere_frame_doc({ widget, unocss: unocssSource })
      );
    }

    const host = document.createElement("div");
    host.className = "aa-shell aa-inline-widget";
    const shadow = host.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    style.textContent = `${INLINE_HOST_CSS}${parts.css || ""}`;
    const root = document.createElement("div");
    root.className = "aa-widget-root";
    root.appendChild(sanitizeHTML(html));
    shadow.append(style, root);
    return host;
  }

  function createShell(spec) {
    const plain = wasmPlain(spec);
    return createIframeMount(() =>
      wasmModule.render_frame_doc({ ...plain, unocss: unocssSource })
    );
  }

  function createAnywhereShell(widgetValue) {
    const widget = wasmPlain(widgetValue);
    return createInlineAnywhereMount(widget);
  }

  function shellForPreText(text) {
    const normalized = stripOuterCodeFence(normalizeWidgetText(text));
    if (normalized.includes("<ai-anywhere")) {
      let widgets;
      try {
        widgets = wasmModule.extract_widgets(normalized);
      } catch (error) {
        console.error("crepuscularity: failed to parse <ai-anywhere> tags", error);
        return null;
      }
      const widgetList = wasmArray(widgets);
      if (widgetList.length > 0) {
        return createAnywhereShell(wasmPlain(widgetList[0]));
      }
      console.warn("crepuscularity: <ai-anywhere> found in text but extract_widgets returned none");
    }

    if (normalized.includes("```")) {
      let specs;
      try {
        specs = wasmModule.extract_specs(normalized);
      } catch (error) {
        console.error("crepuscularity: failed to parse widget blocks", error);
        return null;
      }
      const specList = wasmArray(specs);
      if (specList.length > 0) {
        return createShell(wasmPlain(specList[0]));
      }
    }

    return null;
  }

  function enhanceCodeBlock(pre) {
    if (!(pre instanceof HTMLElement) || mounted.has(pre)) {
      return;
    }

    const shell = shellForPreText(widgetTextFromPre(pre));
    if (!shell) {
      return;
    }

    mountWidgetShell(pre, shell);
  }

  function queueNode(node) {
    if (!(node instanceof HTMLElement)) {
      return;
    }
    if (node.matches("pre") && hasAnywhereContent(node)) {
      pending.add(node);
      return;
    }
    const nodes = node.getElementsByTagName("pre");
    for (let i = 0; i < nodes.length; i++) {
      const pre = nodes[i];
      if (pre instanceof HTMLElement && hasAnywhereContent(pre)) {
        pending.add(pre);
      }
    }
  }

  function flushPending() {
    flushScheduled = false;
    const nodes = dedupeNodes([...pending]);
    pending.clear();
    for (const node of nodes) {
      enhanceCodeBlock(node);
    }
  }

  function scheduleFlush() {
    if (flushScheduled) {
      return;
    }
    flushScheduled = true;
    requestAnimationFrame(flushPending);
  }

  for (const pre of collectWidgetPres()) {
    pending.add(pre);
  }
  scheduleFlush();

  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => queueNode(node));
    }
    if (pending.size > 0) {
      scheduleFlush();
    }
  });
  observer.observe(document.documentElement, { subtree: true, childList: true });

  if (typeof wasmModule.content_main === "function") {
    wasmModule.content_main();
  }
})();
