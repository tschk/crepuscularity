import init, * as runtime from "./pkg/runtime.js";

const delegatedEvents = [
  "click",
  "input",
  "change",
  "keydown",
  "keyup",
  "mousedown",
  "mouseup",
  "mousemove",
  "scroll",
];

let currentBundle = null;
let currentRoot = null;
let currentIslandManifest = { islands: {} };

function setRootHtml(root, html) {
  const doc = new DOMParser().parseFromString(html, "text/html");
  root.replaceChildren(...doc.body.childNodes);
}

// ── slot-rotate ──────────────────────────────────────────────────────────────

function parseSlotWords(raw) {
  const t = (raw || "").trim();
  if (t.startsWith("[")) {
    try {
      const w = JSON.parse(t);
      return Array.isArray(w) ? w.map(String).filter(Boolean) : [];
    } catch {
      return [];
    }
  }
  return t.split("|").map((s) => s.trim()).filter(Boolean);
}

function initSlotRotate(root) {
  const el = root.querySelector("[data-slot-words]");
  if (!el) return;
  const raw = el.getAttribute("data-slot-words") || "";
  const words = parseSlotWords(raw);
  if (words.length < 2) return;
  el.classList.add("crepus-slot");
  el.setAttribute("aria-live", "polite");
  el.innerHTML = "";
  const inner = document.createElement("span");
  inner.className = "crepus-slot__inner";
  inner.textContent = words[0];
  el.appendChild(inner);
  let i = 0;
  const intervalAttr = el.getAttribute("data-slot-interval");
  const duration = intervalAttr ? Number(intervalAttr) || 3200 : 3200;
  const tick = () => {
    i = (i + 1) % words.length;
    inner.style.transition = "opacity 0.32s ease, filter 0.32s ease";
    inner.style.opacity = "0";
    inner.style.filter = "blur(6px)";
    window.setTimeout(() => {
      inner.textContent = words[i];
      requestAnimationFrame(() => {
        inner.style.opacity = "1";
        inner.style.filter = "blur(0)";
      });
    }, 280);
  };
  window.setInterval(tick, duration);
}

// ── Web Islands ──────────────────────────────────────────────────────────────

function parseIslandProps(el) {
  const raw = el.getAttribute("data-crepus-island-props") || "{}";
  try {
    const value = JSON.parse(raw);
    return value && typeof value === "object" ? value : {};
  } catch {
    return {};
  }
}

async function loadIslandManifest() {
  try {
    const res = await fetch("./crepus-islands.json", { cache: "no-store" });
    if (!res.ok) return { islands: {} };
    const manifest = await res.json();
    return manifest && typeof manifest === "object" ? manifest : { islands: {} };
  } catch {
    return { islands: {} };
  }
}

function resolveIslandModule(src) {
  const entry = currentIslandManifest?.islands?.[src];
  return entry?.module || src;
}

function initWebIslands(root) {
  const islands = root.querySelectorAll("[data-crepus-island]");
  for (const el of islands) {
    if (el.dataset.crepusIslandMounted === "true") continue;
    el.dataset.crepusIslandMounted = "true";

    const src = el.getAttribute("data-crepus-island-src");
    if (!src) continue;

    const adapter = el.getAttribute("data-crepus-island-adapter") || "module";
    const props = parseIslandProps(el);

    if (adapter === "rust") {
      if (!el.id) {
        el.id = "crepus-island-" + Math.random().toString(36).slice(2, 8);
      }
      const scopeName = el.getAttribute("data-crepus-scope") || src;
      const renderFn = runtime["render_" + scopeName.replace(/[^a-zA-Z0-9]/g, "_")];
      if (typeof renderFn === "function") {
        Promise.resolve()
          .then(() => renderFn(JSON.stringify(props)))
          .then((html) => { if (html) el.innerHTML = html; })
          .catch((error) => {
            console.error(`crepus rust island render failed: ${scopeName}`, error);
          });
      } else {
        console.warn(`Missing WASM export: render_${scopeName}`);
      }
      continue;
    }

    const modulePath = resolveIslandModule(src);
    import(modulePath)
      .then((mod) => {
        const mount = typeof mod.mount === "function" ? mod.mount : mod.default;
        if (typeof mount === "function") {
          return mount(el, props, { runtime, rerender });
        }
        throw new Error(`Missing island mount export: ${src}`);
      })
      .catch((error) => {
        console.error(`crepus island failed: ${src}`, error);
        el.textContent = "";
        const pre = document.createElement("pre");
        pre.style.cssText = "white-space:pre-wrap;color:#ef4444;font-family:monospace";
        pre.textContent = String(error);
        el.appendChild(pre);
      });
  }
}

// ── Event delegation ─────────────────────────────────────────────────────────

function findHandlerTarget(root, eventName, startNode) {
  const attr = `data-on${eventName}`;
  let node = startNode instanceof Element ? startNode : startNode?.parentElement;
  while (node && node !== root) {
    if (node.hasAttribute?.(attr)) {
      return node;
    }
    node = node.parentElement;
  }
  return root?.hasAttribute?.(attr) ? root : null;
}

function findIslandScope(startNode) {
  let node = startNode instanceof Element ? startNode : startNode?.parentElement;
  while (node) {
    if (node.hasAttribute?.("data-crepus-scope")) {
      return node;
    }
    node = node.parentElement;
  }
  return null;
}

function initDelegatedEvents(root) {
  if (!root || root.dataset.crepusEventsBound === "true") return;
  root.dataset.crepusEventsBound = "true";

  for (const eventName of delegatedEvents) {
    root.addEventListener(eventName, (event) => {
      const target = findHandlerTarget(root, eventName, event.target);
      if (!target) return;

      const handlerName = target.getAttribute(`data-on${eventName}`);
      const handler = handlerName ? runtime[handlerName] : null;
      if (typeof handler !== "function") {
        return;
      }

      const scopeEl = findIslandScope(event.target);
      const scopeName = scopeEl ? scopeEl.getAttribute("data-crepus-scope") : null;

      if (scopeName && scopeEl) {
        const eventData = {};
        if (eventName === "keydown" || eventName === "keyup") {
          eventData["_key"] = event.key;
          eventData["_meta"] = event.metaKey;
          eventData["_ctrl"] = event.ctrlKey;
          eventData["_shift"] = event.shiftKey;
        }

        // Auto-close brackets and quotes in editable scopes
        if (eventName === "keydown" && !event.metaKey && !event.ctrlKey) {
          const pairs = { '(': ')', '[': ']', '{': '}', '"': '"', "'": "'", '`': '`' };
          const closeChar = pairs[event.key];
          if (closeChar) {
            const editable = event.target.closest?.('[contenteditable="true"]') || event.target;
            if (editable.isContentEditable || editable.getAttribute?.("contenteditable") === "true") {
              event.preventDefault();
              const sel = window.getSelection();
              if (sel.rangeCount) {
                const range = sel.getRangeAt(0);
                const pair = event.key + closeChar;
                const textNode = document.createTextNode(pair);
                range.deleteContents();
                range.insertNode(textNode);
                range.setStartAfter(textNode);
                range.collapse(true);
                // Move cursor back one character
                const backRange = document.createRange();
                backRange.setStart(textNode, 1);
                backRange.collapse(true);
                sel.removeAllRanges();
                sel.addRange(backRange);
              }
              // Skip calling the WASM handler since we handled the character insertion
              return;
            }
          }
        }

        for (const attr of target.attributes || []) {
          if (attr.name.startsWith("data-") && attr.name !== "data-onclick" && attr.name !== "data-on" + eventName) {
            eventData[attr.name] = attr.value;
          }
        }
        const skipRerender = eventName !== "click";
        Promise.resolve()
          .then(() => handler(JSON.stringify(eventData)))
          .then(() => {
            if (!skipRerender) {
              return rerenderScope(scopeEl, scopeName);
            }
          })
          .catch((error) => {
            console.error(`crepus scope handler failed: ${handlerName}`, error);
          });
      } else {
        Promise.resolve()
          .then(() => handler("{}"))
          .then(() => rerender())
          .catch((error) => {
            console.error(`crepus event handler failed: ${handlerName}`, error);
          });
      }
    });
  }
}

function rerender() {
  if (!currentRoot || !currentBundle) return;
  setRootHtml(currentRoot, runtime.crepus_render(JSON.stringify(currentBundle)));
  initDelegatedEvents(currentRoot);
  initInteractive(currentRoot);
}

function rerenderScope(scopeEl, scopeName) {
  const fnName = "render_" + scopeName.replace(/[^a-zA-Z0-9]/g, "_");
  const renderFn = runtime[fnName];
  if (typeof renderFn !== "function") {
    return rerender();
  }
  const html = renderFn("{}");
  if (html) {
    scopeEl.innerHTML = html;
  }
}

function initInteractive(root) {
  initSlotRotate(root);
  initWebIslands(root);
}

// ── Bootstrap ────────────────────────────────────────────────────────────────

window.addEventListener("crepus-rerender-editor", () => {
  const scopeEl = currentRoot?.querySelector("[data-crepus-scope='editor']");
  if (scopeEl) {
    rerenderScope(scopeEl, "editor");
  }
});

async function main() {
  // ponytail: inline bundle from SSR HTML — saves one HTTP round trip
  const bundleEl = document.getElementById("__crepus_bundle__");
  let bundle, wasmPromise;
  if (bundleEl) {
    bundle = JSON.parse(bundleEl.textContent);
    bundleEl.remove();
    wasmPromise = init(); // start WASM init, no fetch needed
  } else {
    // start both in parallel
    wasmPromise = init();
    const res = await fetch("./crepus-bundle.json", { cache: "no-store" });
    if (!res.ok) throw new Error(`crepus-bundle.json: HTTP ${res.status}`);
    bundle = await res.json();
  }
  currentBundle = bundle;
  currentIslandManifest = await loadIslandManifest();

  const root = document.getElementById("crepus-root");
  if (root) {
    currentRoot = root;
    const hasSsr = root.querySelector("[data-crepus-root]") || root.querySelector("[data-crepus-id]");
    if (hasSsr) {
      // ponytail: reveal SSR content immediately — UnoCSS already scanned before app.js loads
      initDelegatedEvents(root);
      root.style.opacity = "1";
      // WASM needed for islands, signal bindings, and heading animations
      await wasmPromise;
      initInteractive(root);
      if (window.__unocss_runtime) window.__unocss_runtime.extractAll();
    } else {
      await wasmPromise;
      setRootHtml(root, runtime.crepus_render(JSON.stringify(bundle)));
      initDelegatedEvents(root);
      initInteractive(root);
      if (window.__unocss_runtime) window.__unocss_runtime.extractAll();
      root.style.opacity = "1";
    }
  }
}

main().catch((e) => {
  console.error(e);
  const root = document.getElementById("crepus-root");
  if (root) {
    root.textContent = "";
    root.style.opacity = "1";
    const wrap = document.createElement("div");
    wrap.style.cssText = "padding:2rem;font-family:monospace;color:#ef4444";
    const strong = document.createElement("strong");
    strong.textContent = "WASM boot failed";
    const pre = document.createElement("pre");
    pre.style.whiteSpace = "pre-wrap";
    pre.textContent = String(e);
    wrap.appendChild(strong);
    wrap.appendChild(pre);
    root.appendChild(wrap);
  }
});
