import init, * as runtime from "./pkg/runtime.js";
import { createWorkerRuntime } from "./runtime-client.js";

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
let workerRuntime = null;
let bundleRevision = 0;
let rootRenderGeneration = 0;
let currentIslandManifest = { islands: {} };

let restoringFocus = false;
const islandDisposers = new WeakMap();
const slotRotationStops = new WeakMap();

function disposeSlotRotations(root) {
  slotRotationStops.get(root)?.();
  for (const el of root.querySelectorAll("[data-slot-words]")) {
    slotRotationStops.get(el)?.();
  }
}

function disposeIslands(root) {
  for (const el of root.querySelectorAll("[data-crepus-island]")) {
    const dispose = islandDisposers.get(el);
    if (!dispose) continue;
    islandDisposers.delete(el);
    try {
      Promise.resolve(dispose()).catch((error) => console.error("crepus island cleanup failed", error));
    } catch (error) { console.error("crepus island cleanup failed", error); }
  }
}

function preserveFocus(root, replace) {
  const active = document.activeElement;
  const inside = active !== root && root.contains(active);
  const path = [];
  if (inside) {
    for (let node = active; node !== root; node = node.parentElement) {
      path.unshift(Array.prototype.indexOf.call(node.parentElement.children, node));
    }
  }
  const selection = inside && typeof active.selectionStart === "number"
    ? [active.selectionStart, active.selectionEnd, active.selectionDirection] : null;
  const previous = restoringFocus;
  restoringFocus = true;
  try {
    disposeSlotRotations(root);
    disposeIslands(root);
    replace();
    if (!inside) return;
    let next;
    if (active.id) {
      next = Array.from(root.querySelectorAll("[id]")).find((el) => el.id === active.id);
    } else {
      // shortcut: positional identity requires stable structure; give reordered controls stable IDs.
      next = path.reduce((node, index) => node?.children[index], root);
    }
    if (!next || next.tagName !== active.tagName || next.type !== active.type ||
        next.getAttribute("name") !== active.getAttribute("name") || next.matches(":disabled")) return;
    next.focus({ preventScroll: true });
    if (document.activeElement === next && selection) next.setSelectionRange(...selection);
  } finally {
    restoringFocus = previous;
  }
}

function setRootHtml(root, html) {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const fragment = doc.createDocumentFragment();
  // Avoid argument limits without Range.extractContents' large-list cost in WebKit.
  while (doc.body.firstChild) fragment.appendChild(doc.body.firstChild);
  preserveFocus(root, () => root.replaceChildren(fragment));
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

const slotRotations = new WeakMap();

function initSlotRotate(root) {
  for (const stop of slotRotations.get(root) || []) stop();
  slotRotations.delete(root);
  const cleanups = new Set();
  slotRotations.set(root, cleanups);
  for (const el of root.querySelectorAll("[data-slot-words]")) {
    slotRotationStops.get(el)?.();
    const raw = el.getAttribute("data-slot-words") || "";
    const words = parseSlotWords(raw);
    if (words.length < 2) continue;
    el.classList.add("crepus-slot");
    el.setAttribute("aria-live", "polite");
    el.innerHTML = "";
    const inner = document.createElement("span");
    inner.className = "crepus-slot__inner";
    inner.textContent = words[0];
    el.appendChild(inner);
    let i = 0;
    let active = true;
    const intervalAttr = el.getAttribute("data-slot-interval");
    const requested = Number(intervalAttr);
    // Browser timers use a signed 32-bit delay; overflow can become an immediate timer.
    const duration = Number.isInteger(requested) && requested > 0 && requested <= 2147483647
      ? requested : 3200;
    const tick = () => {
      if (!active) return;
      if (!el.isConnected) {
        stop();
        return;
      }
      i = (i + 1) % words.length;
      const word = words[i];
      inner.style.transition = "opacity 0.32s ease, filter 0.32s ease";
      inner.style.opacity = "0";
      inner.style.filter = "blur(6px)";
      window.setTimeout(() => {
        if (!active) return;
        inner.textContent = word;
        requestAnimationFrame(() => {
          if (!active) return;
          inner.style.opacity = "1";
          inner.style.filter = "blur(0)";
        });
      }, 280);
    };
    const timer = window.setInterval(tick, duration);
    const stop = () => {
      active = false;
      window.clearInterval(timer);
      slotRotationStops.delete(el);
      cleanups.delete(stop);
      if (!cleanups.size) slotRotations.delete(root);
    };
    cleanups.add(stop);
    slotRotationStops.set(el, stop);
  }
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

const islandRenderGenerations = new WeakMap();

function beginIslandRender(el) {
  const generation = (islandRenderGenerations.get(el) || 0) + 1;
  islandRenderGenerations.set(el, generation);
  return generation;
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
        if (workerRuntime) {
          renderInWorker(el, scopeName)
            .catch((error) => console.error(`crepus rust island render failed: ${scopeName}`, error));
          continue;
        }
        const generation = beginIslandRender(el);
        Promise.resolve()
          .then(() => {
            if (!el.isConnected || !root.contains(el) || islandRenderGenerations.get(el) !== generation) return;
            return renderFn(JSON.stringify(props));
          })
          .then((html) => {
            if (!el.isConnected || !root.contains(el) || islandRenderGenerations.get(el) !== generation) return;
            if (typeof html === "string") {
              preserveFocus(el, () => { el.innerHTML = html; });
              initDelegatedEvents(currentRoot || root);
              initInteractive(el);
            }
          })
          .catch((error) => {
            console.error(`crepus rust island render failed: ${scopeName}`, error);
          });
      } else {
        console.warn(`Missing WASM export: render_${scopeName}`);
      }
      continue;
    }

    const modulePath = resolveIslandModule(src);
    const modulePromise = workerRuntime
      ? Promise.reject(new Error('JavaScript islands require <meta name="crepus-runtime-thread" content="main"> for synchronous runtime exports.'))
      : import(modulePath);
    modulePromise.then((mod) => {
        if (!el.isConnected || !root.contains(el)) return;
        const mount = typeof mod.mount === "function" ? mod.mount : mod.default;
        if (typeof mount === "function") {
          return Promise.resolve(mount(el, props, { runtime, rerender })).then((dispose) => {
            if (typeof dispose !== "function") return;
            if (!el.isConnected || !root.contains(el)) return dispose();
            islandDisposers.set(el, dispose);
          });
        }
        throw new Error(`Missing island mount export: ${src}`);
      })
      .catch((error) => {
        if (!el.isConnected || !root.contains(el)) return;
        console.error(`crepus island failed: ${src}`, error);
        disposeSlotRotations(el);
        disposeIslands(el);
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

const controlModifiers = new Set(["prevent", "stop", "immediate", "trusted", "self", "once", "capture"]);
const onceHandlers = new WeakMap();
const boundEventRoots = new WeakMap();

function readEventModifiers(target, eventName) {
  const raw = target.getAttribute(`data-crepus-modifiers-${eventName}`);
  if (raw === null) return [];
  try {
    const modifiers = JSON.parse(raw);
    return Array.isArray(modifiers) && modifiers.every((value) => typeof value === "string")
      ? modifiers : null;
  } catch { return null; }
}

function readEventHandlers(target, eventName) {
  const raw = target.getAttribute(`data-crepus-handlers-${eventName}`);
  if (raw === null) return [{ handler: target.getAttribute(`data-on${eventName}`), modifiers: readEventModifiers(target, eventName) }];
  try {
    const handlers = JSON.parse(raw);
    return Array.isArray(handlers) && handlers.every((entry) => entry && typeof entry.handler === "string" &&
      Array.isArray(entry.modifiers) && entry.modifiers.every((modifier) => typeof modifier === "string")) ? handlers : [];
  } catch { return []; }
}

function matchesKeyChord(chord, event) {
  const aliases = { esc: "escape", arrowup: "up", arrowdown: "down", arrowleft: "left", arrowright: "right", " ": "space" };
  const normalize = (key) => aliases[key.toLowerCase()] || key.toLowerCase();
  const parts = chord.toLowerCase().split("+");
  const key = parts.pop();
  if (!key || normalize(event.key || "") !== normalize(key)) return false;
  const fields = { ctrl: "ctrlKey", cmd: "metaKey", shift: "shiftKey", alt: "altKey", opt: "altKey" };
  return parts.every((modifier) => fields[modifier] && event[fields[modifier]]);
}

function dispatchDelegatedEvent(eventName, event, target, modifiers, runtimeReady, handlerName, index) {
  const handler = handlerName ? runtime[handlerName] : null;

  const onceKey = `${eventName}:${modifiers.includes("capture")}:${index}:${handlerName}`;
  if (modifiers.includes("once")) {
    const seen = onceHandlers.get(target) || new Set();
    if (seen.has(onceKey)) return false;
    seen.add(onceKey);
    onceHandlers.set(target, seen);
  }
  const keys = modifiers.filter((modifier) => !controlModifiers.has(modifier));
  if (keys.length && (!(eventName === "keydown" || eventName === "keyup") ||
      !keys.some((key) => matchesKeyChord(key, event)))) return false;
  let stopped = false;
  for (const modifier of modifiers) {
    if (modifier === "prevent") event.preventDefault();
    if (modifier === "stop") { event.stopPropagation(); if (stopped !== "immediate") stopped = true; }
    if (modifier === "immediate") { event.stopImmediatePropagation(); stopped = "immediate"; }
    if (modifier === "trusted" && !event.isTrusted) return stopped;
    if (modifier === "self" && event.target !== target) return stopped;
  }
  if (typeof handler !== "function") return stopped;

  const scopeEl = findIslandScope(target);
  const scopeName = scopeEl ? scopeEl.getAttribute("data-crepus-scope") : null;

  const composing = event.isComposing === true;
  const eventData = {};
  if (eventName === "keydown" || eventName === "keyup") {
    eventData["_key"] = event.key;
    eventData["_meta"] = event.metaKey;
    eventData["_ctrl"] = event.ctrlKey;
    eventData["_shift"] = event.shiftKey;
    eventData["_alt"] = event.altKey;
  }
  if (eventName === "input" || eventName === "change") {
    const control = event.target;
    if (["INPUT", "TEXTAREA", "SELECT"].includes(control?.tagName)) {
      eventData["_value"] = control.value;
      if (control.tagName === "SELECT" && control.multiple) {
        eventData["_values"] = Array.from(control.selectedOptions, (option) => option.value);
      }
      if (control.type === "checkbox" || control.type === "radio") {
        eventData["_checked"] = control.checked;
      }
    }
  }
  if (eventName === "submit" && event.target?.tagName === "FORM") {
    eventData["_form"] = Array.from(new FormData(event.target, event.submitter), ([name, value]) => [
      name,
      typeof value === "string" ? value : {
        name: value.name, type: value.type, size: value.size, lastModified: value.lastModified,
      },
    ]);
  }
  for (const attr of target.attributes || []) {
    if (attr.name.startsWith("data-") && attr.name !== "data-onclick" && attr.name !== "data-on" + eventName && !attr.name.startsWith("data-crepus-modifiers-") && !attr.name.startsWith("data-crepus-handlers-")) {
      eventData[attr.name] = attr.value;
    }
  }
  if (scopeName && scopeEl) {

    // Auto-close brackets and quotes in editable scopes
    if (eventName === "keydown" && !composing && !event.defaultPrevented && !event.metaKey && !event.ctrlKey) {
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
          return stopped;
        }
      }
    }

    // Keep the browser-owned rich-text caret and composition intact during editing.
    const editable = event.target.closest?.("[contenteditable]");
    const editing = editable && ["", "true", "plaintext-only"].includes(editable.getAttribute("contenteditable").toLowerCase());
    const skipRerender = composing || (editing && ["input", "beforeinput", "keydown", "keyup",
      "compositionstart", "compositionupdate", "compositionend"].includes(eventName));
    if (workerRuntime) {
      Promise.resolve(runtimeReady)
        .then(() => renderInWorker(scopeEl, scopeName, handlerName, eventData, skipRerender))
        .catch((error) => console.error(`crepus scope handler failed: ${handlerName}`, error));
      return stopped;
    }
    Promise.resolve(runtimeReady)
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
    if (workerRuntime) {
      Promise.resolve(runtimeReady)
        .then(() => renderInWorker(null, null, handlerName, eventData, composing))
        .catch((error) => console.error(`crepus event handler failed: ${handlerName}`, error));
      return stopped;
    }
    Promise.resolve(runtimeReady)
      .then(() => handler(JSON.stringify(eventData)))
      .then(() => { if (!composing) return rerender(); })
      .catch((error) => {
        console.error(`crepus event handler failed: ${handlerName}`, error);
      });
  }
  return stopped;
}

function initDelegatedEvents(root, runtimeReady) {
  if (!root) return;
  root.dataset.crepusEventsBound = "true";
  const binding = boundEventRoots.get(root) || { events: new Set(), ready: undefined };
  if (runtimeReady !== undefined) binding.ready = runtimeReady;
  boundEventRoots.set(root, binding);
  const bound = binding.events;
  const events = new Set(delegatedEvents);
  for (const element of [root, ...root.querySelectorAll("*")]) {
    for (const attribute of element.attributes) {
      if (attribute.name.startsWith("data-on")) {
        const name = attribute.name.slice("data-on".length);
        if (name) events.add(name);
      }
    }
  }
  for (const eventName of events) {
    if (bound.has(eventName)) continue;
    bound.add(eventName);
    for (const capture of [true, false]) {
      root.addEventListener(eventName, (event) => {
        if (restoringFocus && ["focus", "blur", "focusin", "focusout"].includes(eventName)) return;
        const targets = [];
        let target = findHandlerTarget(root, eventName, event.target);
        while (target) {
          targets.push(target);
          if (target === root) break;
          target = findHandlerTarget(root, eventName, target.parentElement);
        }
        if (capture) targets.reverse();
        for (const target of targets) {
          const targetOnly = capture && !event.bubbles && event.target === target && target !== root;
          const handlers = readEventHandlers(target, eventName);
          for (const phase of targetOnly ? [true, false] : [capture]) {
            let stopped = false;
            for (const [index, entry] of handlers.entries()) {
              const modifiers = entry.modifiers;
              if (!modifiers || modifiers.includes("capture") !== phase) continue;
              const outcome = dispatchDelegatedEvent(eventName, event, target, modifiers, binding.ready, entry.handler, index);
              if (outcome === "immediate") return;
              stopped = outcome || stopped;
            }
            if (stopped) return;
          }
        }
      }, capture);
    }
  }
}

function initializeRuntime() {
  const mode = document.querySelector('meta[name="crepus-runtime-thread"]')?.content;
  if (mode && mode !== "main" && mode !== "worker") {
    throw new Error("crepus-runtime-thread must be main or worker");
  }
  if (mode === "worker" || (mode !== "main" && typeof runtime.crepus_worker_runtime === "function")) {
    workerRuntime = createWorkerRuntime(new Worker(new URL("./runtime-worker.js", import.meta.url), { type: "module" }));
    window.addEventListener("pagehide", (event) => {
      if (!event.persisted) workerRuntime.dispose();
    });
    return workerRuntime.ready;
  }
  return init();
}

async function renderInWorker(scopeEl = null, scopeName = null, handlerName = null, eventData = null, skipRender = false) {
  const root = currentRoot;
  const scopedName = "render_" + (scopeName || "").replace(/[^a-zA-Z0-9]/g, "_");
  const scoped = scopeEl && typeof runtime[scopedName] === "function";
  const generation = scoped ? beginIslandRender(scopeEl) : ++rootRenderGeneration;
  const rootGeneration = rootRenderGeneration;
  const revision = bundleRevision;
  const spec = skipRender ? null : {
    name: scoped ? scopedName : "crepus_render",
    args: [scoped ? parseIslandProps(scopeEl) : currentBundle],
    json: true,
  };
  const html = await (handlerName
    ? workerRuntime.event(handlerName, JSON.stringify(eventData), spec)
    : workerRuntime.render(spec));
  if (!spec || typeof html !== "string" || root !== currentRoot ||
      rootGeneration !== rootRenderGeneration || revision !== bundleRevision) return;
  if (scoped) {
    if (!scopeEl.isConnected || !root.contains(scopeEl)) {
      // A root commit may replace the scope after its handler was queued.
      if (handlerName) return renderInWorker();
      return;
    }
    if (islandRenderGenerations.get(scopeEl) !== generation) return;
    preserveFocus(scopeEl, () => { scopeEl.innerHTML = html; });
    initDelegatedEvents(root);
    initInteractive(scopeEl);
  } else {
    setRootHtml(root, html);
    initDelegatedEvents(root);
    initInteractive(root);
  }
}

function rerender() {
  if (!currentRoot || !currentBundle) return;
  if (workerRuntime) return renderInWorker();
  setRootHtml(currentRoot, runtime.crepus_render(JSON.stringify(currentBundle)));
  initDelegatedEvents(currentRoot);
  initInteractive(currentRoot);
}

function rerenderScope(scopeEl, scopeName) {
  if (workerRuntime) return renderInWorker(scopeEl, scopeName);
  beginIslandRender(scopeEl);
  const fnName = "render_" + scopeName.replace(/[^a-zA-Z0-9]/g, "_");
  const renderFn = runtime[fnName];
  if (typeof renderFn !== "function") {
    return rerender();
  }
  const html = renderFn(JSON.stringify(parseIslandProps(scopeEl)));
  if (typeof html === "string") {
    preserveFocus(scopeEl, () => { scopeEl.innerHTML = html; });
    initDelegatedEvents(currentRoot);
    initInteractive(scopeEl);
  }
}

function initInteractive(root) {
  initSlotRotate(root);
  initWebIslands(root);
}

// ── Bootstrap ────────────────────────────────────────────────────────────────

window.addEventListener("crepus-update-bundle", (event) => {
  if (!currentRoot) return;
  currentBundle = event.detail;
  bundleRevision++;
  Promise.resolve(rerender()).catch((error) => console.error("crepus bundle render failed", error));
  event.preventDefault();
});

window.addEventListener("crepus-rerender-editor", () => {
  const scopeEl = currentRoot?.querySelector("[data-crepus-scope='editor']");
  if (scopeEl) {
    Promise.resolve(rerenderScope(scopeEl, "editor")).catch((error) => console.error("crepus editor render failed", error));
  }
});

async function main() {
  // ponytail: inline bundle from SSR HTML — saves one HTTP round trip
  const bundleEl = document.getElementById("__crepus_bundle__");
  const wasmPromise = initializeRuntime();
  // Observe early failure while fetching; the later await still reports the original error.
  wasmPromise.catch(() => {});
  let bundle;
  if (bundleEl) {
    bundle = JSON.parse(bundleEl.textContent);
    bundleEl.remove();
  } else {
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
      initDelegatedEvents(root, wasmPromise);
      root.style.opacity = "1";
      // WASM needed for islands, signal bindings, and heading animations
      await wasmPromise;
      initInteractive(root);
      if (window.__unocss_runtime) window.__unocss_runtime.extractAll();
    } else {
      await wasmPromise;
      if (workerRuntime) await renderInWorker();
      else setRootHtml(root, runtime.crepus_render(JSON.stringify(bundle)));
      initDelegatedEvents(root, wasmPromise);
      initInteractive(root);
      if (window.__unocss_runtime) window.__unocss_runtime.extractAll();
      root.style.opacity = "1";
    }
  }
}

main().catch((e) => {
  workerRuntime?.dispose();
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
