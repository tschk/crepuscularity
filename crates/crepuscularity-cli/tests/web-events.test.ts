import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import { readFileSync } from "node:fs";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const source = readFileSync(`${import.meta.dir}/../assets/web/app.js`, "utf8")
  .replaceAll("import.meta.url", JSON.stringify("http://localhost/app.js"));
const delegation = source.slice(source.indexOf("function findHandlerTarget("), source.indexOf("function initializeRuntime()"));
let root: HTMLElement;
let calls: string[];
let payloads: Record<string, unknown>[];
let scopes: string[];
let renders: number;
let replaceHtml: (root: HTMLElement, html: string) => void;
let init: (root: HTMLElement, ready?: Promise<void>) => void;

beforeEach(() => {
  GlobalRegistrator.register();
  document.body.innerHTML = '<div id="root"><div id="parent"><button id="child"><span id="inner">Press</span></button></div></div>';
  root = document.getElementById("root")!;
  calls = [];
  payloads = [];
  scopes = [];
  renders = 0;
  const runtime = Object.fromEntries(["root", "parent", "child"].map((name) => [name, (json: string) => { calls.push(name); payloads.push(JSON.parse(json)); }]));
  const events = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("let currentBundle"));
  const reconciliation = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── slot-rotate"));
  const api = new Function("runtime", "Element", "rerender", "rerenderScope", "workerRuntime", `${events}
${reconciliation}
${delegation}; return { initDelegatedEvents, setRootHtml };`)(runtime, Element, () => { renders++; }, (_: Element, scope: string) => { scopes.push(scope); }, null);
  init = api.initDelegatedEvents;
  replaceHtml = api.setRootHtml;
});
afterEach(() => GlobalRegistrator.unregister());

function bind(id: string, event: string, modifiers: string[] = []) {
  const element = document.getElementById(id)!;
  element.setAttribute(`data-on${event}`, id);
  if (modifiers.length) element.setAttribute(`data-crepus-modifiers-${event}`, JSON.stringify(modifiers));
  return element;
}
async function flush() { await new Promise((resolve) => setTimeout(resolve, 0)); }

test("nested handlers run in capture then bubble order", async () => {
  bind("root", "click", ["capture"]);
  bind("parent", "click");
  const child = bind("child", "click");
  init(root); init(root);
  child.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flush();
  expect(calls).toEqual(["root", "child", "parent"]);
});

test("prevent and stop cancel default and ancestor dispatch", async () => {
  bind("parent", "click");
  const child = bind("child", "click", ["prevent", "stop"]);
  init(root);
  const event = new MouseEvent("click", { bubbles: true, cancelable: true });
  expect(child.dispatchEvent(event)).toBe(false);
  expect(event.defaultPrevented).toBe(true);
  await flush();
  expect(calls).toEqual(["child"]);
});

test("self guards retain modifier order and allow ancestor handlers", async () => {
  bind("root", "click");
  const parent = bind("parent", "click", ["prevent", "self"]);
  init(root);
  const event = new MouseEvent("click", { bubbles: true, cancelable: true });
  document.getElementById("inner")!.dispatchEvent(event);
  await flush();
  expect(event.defaultPrevented).toBe(true);
  expect(calls).toEqual(["root"]);
  calls.length = 0;
  parent.setAttribute("data-crepus-modifiers-click", JSON.stringify(["self", "prevent"]));
  const guarded = new MouseEvent("click", { bubbles: true, cancelable: true });
  document.getElementById("inner")!.dispatchEvent(guarded);
  await flush();
  expect(guarded.defaultPrevented).toBe(false);
  expect(calls).toEqual(["root"]);
});

test("once applies per element and is consumed even when self rejects", async () => {
  const child = bind("child", "click", ["once"]);
  init(root);
  child.click(); child.click();
  await flush();
  expect(calls).toEqual(["child"]);
  const replacement = child.cloneNode(true) as HTMLElement;
  child.replaceWith(replacement);
  replacement.click();
  await flush();
  expect(calls).toEqual(["child", "child"]);
  bind("parent", "click", ["once", "self"]);
  replacement.click();
  document.getElementById("parent")!.click();
  await flush();
  expect(calls).toEqual(["child", "child"]);
});

test("keydown and keyup filter aliases and modifier chords", async () => {
  const child = bind("child", "keydown", ["ctrl+s", "cmd+Esc", "shift+ArrowUp", "alt+x", "Space"]);
  bind("child", "keyup", ["ctrl+s"]);
  init(root);
  for (const options of [
    { key: "s" }, { key: "x", ctrlKey: true },
    { key: "s", ctrlKey: true }, { key: "Escape", metaKey: true },
    { key: "ArrowUp", shiftKey: true }, { key: "x", altKey: true }, { key: " " },
  ]) child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, ...options }));
  child.dispatchEvent(new KeyboardEvent("keyup", { bubbles: true, key: "s" }));
  child.dispatchEvent(new KeyboardEvent("keyup", { bubbles: true, key: "s", ctrlKey: true }));
  await flush();
  expect(calls).toEqual(Array(6).fill("child"));
});

test("malformed modifier metadata never dispatches an unfiltered action", async () => {
  const child = bind("child", "keydown");
  init(root);
  for (const raw of ["broken", "null", '[false]', '"ctrl+s"']) {
    child.setAttribute("data-crepus-modifiers-keydown", raw);
    child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "s", ctrlKey: true }));
  }
  await flush();
  expect(calls).toEqual([]);
});

test("non-bubbling scroll reaches its target without bubbling to ancestors", async () => {
  bind("root", "scroll", ["capture"]);
  bind("parent", "scroll");
  const child = bind("child", "scroll");
  init(root);
  child.dispatchEvent(new Event("scroll"));
  await flush();
  expect(calls).toEqual(["root", "child"]);
});


test("capture stop prevents target and bubble handlers", async () => {
  bind("root", "click", ["capture", "stop"]);
  bind("parent", "click");
  const child = bind("child", "click");
  init(root);
  child.click();
  await flush();
  expect(calls).toEqual(["root"]);
});

test("new event types added after rendering bind once", async () => {
  init(root);
  const child = bind("child", "ready-modifiers", ["once"]);
  init(root); init(root);
  child.dispatchEvent(new Event("ready-modifiers", { bubbles: true }));
  child.dispatchEvent(new Event("ready-modifiers", { bubbles: true }));
  await flush();
  expect(calls).toEqual(["child"]);
});

test("submit handlers can prevent form submission", async () => {
  const parent = bind("parent", "submit", ["prevent"]);
  init(root);
  const event = new Event("submit", { bubbles: true, cancelable: true });
  expect(parent.dispatchEvent(event)).toBe(false);
  await flush();
  expect(calls).toEqual(["parent"]);
});

test("once consumes its native event even when key guards reject", async () => {
  const child = bind("child", "keydown", ["once", "ctrl+s"]);
  init(root);
  child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "x" }));
  child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "s", ctrlKey: true }));
  await flush();
  expect(calls).toEqual([]);
});

test("modifier metadata stays out of scoped action payloads", async () => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "widget");
  const child = bind("child", "click", ["prevent"]);
  child.setAttribute("data-user-modifiers", "kept");
  init(root);
  child.click();
  await flush();
  expect(payloads).toEqual([{ "data-user-modifiers": "kept" }]);
  expect(scopes).toEqual(["widget"]);
});

test("each bubbling handler uses its own island scope", async () => {
  root.setAttribute("data-crepus-scope", "outer");
  bind("parent", "click");
  const child = bind("child", "click");
  child.setAttribute("data-crepus-scope", "inner");
  init(root);
  child.click();
  await flush();
  expect(calls).toEqual(["child", "parent"]);
  expect(scopes).toEqual(["inner", "outer"]);
});

test("a bubbling editor event inserts only one automatic pair", async () => {
  const parent = bind("parent", "keydown");
  parent.setAttribute("data-crepus-scope", "editor");
  const child = bind("child", "keydown");
  child.setAttribute("contenteditable", "true");
  const selection = window.getSelection()!;
  const range = document.createRange();
  range.selectNodeContents(child);
  range.collapse(false);
  selection.addRange(range);
  init(root);
  child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, key: "(" }));
  await flush();
  expect(child.textContent).toBe("Press()");
  expect(calls).toEqual(["parent"]);
});

test("prevented editor keys reach their action without automatic insertion", async () => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "editor");
  const child = bind("child", "keydown", ["prevent"]);
  child.setAttribute("contenteditable", "true");
  init(root);
  child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, key: "(" }));
  await flush();
  expect(child.textContent).toBe("Press");
  expect(calls).toEqual(["child"]);
});

test("control modifiers still apply when an action export is unavailable", async () => {
  const child = bind("child", "click", ["prevent", "stop"]);
  child.setAttribute("data-onclick", "missing_export");
  bind("parent", "click");
  init(root);
  const event = new MouseEvent("click", { bubbles: true, cancelable: true });
  expect(child.dispatchEvent(event)).toBe(false);
  await flush();
  expect(calls).toEqual([]);
});


test("early SSR actions wait for WASM while modifiers apply immediately", async () => {
  let resolve!: () => void;
  const ready = new Promise<void>((done) => { resolve = done; });
  const child = bind("child", "click", ["prevent", "stop", "once"]);
  init(root, ready);
  init(root);
  const event = new MouseEvent("click", { bubbles: true, cancelable: true });
  expect(child.dispatchEvent(event)).toBe(false);
  child.click();
  await flush();
  expect(calls).toEqual([]);
  resolve();
  await flush();
  expect(calls).toEqual(["child"]);
});

test("scoped actions retain capture order across WASM initialization", async () => {
  let resolve!: () => void;
  const ready = new Promise<void>((done) => { resolve = done; });
  root.setAttribute("data-crepus-scope", "widget");
  bind("root", "click", ["capture"]);
  const child = bind("child", "click");
  init(root, ready);
  child.click();
  await flush();
  expect(calls).toEqual([]);
  resolve();
  await flush();
  expect(calls).toEqual(["root", "child"]);
  expect(scopes).toEqual(["widget", "widget"]);
});

test("root keyboard actions receive key state and owner data", async () => {
  const child = bind("child", "keydown");
  child.setAttribute("data-id", "item-2");
  init(root);
  child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "x", altKey: true, ctrlKey: true, shiftKey: true, metaKey: true }));
  await flush();
  expect(payloads).toEqual([{ _key: "x", _alt: true, _ctrl: true, _shift: true, _meta: true, "data-id": "item-2" }]);
});

test("input payloads snapshot live values before WASM readiness", async () => {
  let resolve!: () => void;
  const ready = new Promise<void>((done) => { resolve = done; });
  const parent = bind("parent", "input");
  parent.innerHTML = '<input id="value" value="initial">';
  const input = document.getElementById("value") as HTMLInputElement;
  init(root, ready);
  for (const value of ["first", "second"]) {
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
  resolve();
  await flush();
  expect(payloads).toEqual([{ _value: "first" }, { _value: "second" }]);
});

test("scoped checkbox changes preserve false and the live value", async () => {
  const parent = bind("parent", "change");
  parent.setAttribute("data-crepus-scope", "widget");
  parent.innerHTML = '<input id="check" type="checkbox" value="choice" checked>';
  const input = document.getElementById("check") as HTMLInputElement;
  init(root);
  input.checked = false;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await flush();
  expect(payloads).toEqual([{ _value: "choice", _checked: false, "data-crepus-scope": "widget" }]);
});

test.each([
  ['<textarea>edited</textarea>', 'edited'],
  ['<input type="number" value="0">', '0'],
  ['<input value="">', ''],
  ['<select><option value="chosen" selected>Choice</option></select>', 'chosen'],
])("form controls forward live values: %s", async (html, value) => {
  const parent = bind("parent", "change");
  parent.innerHTML = html;
  init(root);
  parent.firstElementChild!.dispatchEvent(new Event("change", { bubbles: true }));
  await flush();
  expect(payloads).toEqual([{ _value: value }]);
});

test("multiple selections snapshot every selected value", async () => {
  const parent = bind("parent", "change");
  parent.innerHTML = '<select multiple><option value="a" selected>A</option><option value="b" selected>B</option></select>';
  init(root);
  parent.firstElementChild!.dispatchEvent(new Event("change", { bubbles: true }));
  await flush();
  expect(payloads).toEqual([{ _value: "a", _values: ["a", "b"] }]);
});

test("rerender preserves a named input's focus and selection after reordering", () => {
  root.innerHTML = '<input id="value" value="typed"><button>Save</button>';
  const input = root.querySelector("input")!;
  input.focus(); input.setSelectionRange(1, 4, "backward");
  replaceHtml(root, '<button>Save</button><input id="value" value="typed">');
  const replacement = root.querySelector("input")!;
  expect(document.activeElement).toBe(replacement);
  expect([replacement.selectionStart, replacement.selectionEnd, replacement.selectionDirection]).toEqual([1, 4, "backward"]);
});

test("static idless textarea keeps its caret, clamped to the new value", () => {
  root.innerHTML = '<label>Value<textarea>abcdef</textarea></label>';
  const input = root.querySelector("textarea")!;
  input.focus(); input.setSelectionRange(5, 6);
  replaceHtml(root, '<label>Value<textarea>abc</textarea></label>');
  const replacement = root.querySelector("textarea")!;
  expect(document.activeElement).toBe(replacement);
  expect([replacement.selectionStart, replacement.selectionEnd]).toEqual([3, 3]);
});

test("rerender does not steal outside focus or refocus removed or disabled controls", () => {
  const outside = document.createElement("input"); document.body.append(outside); outside.focus();
  replaceHtml(root, '<input id="value">');
  expect(document.activeElement).toBe(outside);
  root.querySelector("input")!.focus();
  replaceHtml(root, '<input id="different">');
  expect(document.activeElement).not.toBe(root.querySelector("input"));
  root.querySelector("input")!.focus();
  replaceHtml(root, '<input id="different" disabled>');
  expect(document.activeElement).not.toBe(root.querySelector("input"));
});

test("restoring focus does not dispatch another framework focus action", async () => {
  root.innerHTML = '<input id="child" data-onfocus="child" data-onfocusin="parent">';
  init(root);
  root.querySelector("input")!.focus();
  await flush(); expect(calls).toEqual(["child", "parent"]);
  calls.length = 0;
  replaceHtml(root, '<input id="child" data-onfocus="child" data-onfocusin="parent">');
  await flush(); expect(calls).toEqual([]);
  expect(document.activeElement).toBe(root.querySelector("input"));
});

test("scoped rendering preserves the active descendant's text selection", () => {
  root.innerHTML = '<section data-crepus-scope="editor"><textarea id="editor-value">typed</textarea></section>';
  const scope = root.querySelector("section")!;
  const input = scope.querySelector("textarea")!;
  input.focus(); input.setSelectionRange(2, 2);
  const reconciliation = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── slot-rotate"));
  const scoped = source.slice(source.indexOf("function parseIslandProps("), source.indexOf("async function loadIslandManifest("))
    + source.slice(source.indexOf("function rerenderScope("), source.indexOf("function initInteractive("));
  const render = new Function("runtime", "currentRoot", "rerender", "initDelegatedEvents", "initInteractive", `let workerRuntime = null; ${reconciliation}\n${scoped}; return rerenderScope;`)(
    { render_editor: () => '<textarea id="editor-value">typed</textarea>' }, root, () => {}, init, () => {},
  );
  render(scope, "editor");
  const replacement = scope.querySelector("textarea")!;
  expect(document.activeElement).toBe(replacement);
  expect(replacement.selectionStart).toBe(2);
});

test.each(["submit", "change", "input", "dblclick", "keydown", "keyup"])("scoped %s actions refresh rendered state", async (eventName) => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "widget");
  const child = bind("child", eventName);
  init(root);
  child.dispatchEvent(eventName.startsWith("key")
    ? new KeyboardEvent(eventName, { bubbles: true, key: "Enter" })
    : new Event(eventName, { bubbles: true }));
  await flush();
  expect(calls).toEqual(["child"]);
  expect(scopes).toEqual(["widget"]);
});

test.each(["input", "beforeinput", "keydown", "keyup", "compositionstart", "compositionupdate", "compositionend"])("rich-text %s actions preserve browser-owned editing", async (eventName) => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "editor");
  const child = bind("child", eventName);
  child.setAttribute("contenteditable", "true");
  init(root);
  child.dispatchEvent(eventName.startsWith("key")
    ? new KeyboardEvent(eventName, { bubbles: true, key: "x" })
    : new Event(eventName, { bubbles: true }));
  await flush();
  expect(calls).toEqual(["child"]);
  expect(scopes).toEqual([]);
});

test.each(["", "plaintext-only", "TRUE"])("rich-text mode %s preserves keyboard editing", async (mode) => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "editor");
  const child = bind("child", "input"); child.setAttribute("contenteditable", mode);
  init(root); child.dispatchEvent(new Event("input", { bubbles: true }));
  await flush(); expect(scopes).toEqual([]);
});

test("a noneditable descendant can refresh an editable scope", async () => {
  const parent = document.getElementById("parent")!;
  parent.setAttribute("data-crepus-scope", "editor"); parent.setAttribute("contenteditable", "true");
  const child = bind("child", "keydown"); child.setAttribute("contenteditable", "false");
  init(root); child.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Enter" }));
  await flush(); expect(scopes).toEqual(["editor"]);
});

test("an empty scoped render clears obsolete children while a missing result preserves them", () => {
  const reconciliation = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── slot-rotate"));
  const scoped = source.slice(source.indexOf("function parseIslandProps("), source.indexOf("async function loadIslandManifest("))
    + source.slice(source.indexOf("function rerenderScope("), source.indexOf("function initInteractive("));
  const render = new Function("runtime", "currentRoot", "rerender", "initDelegatedEvents", "initInteractive", `let workerRuntime = null; ${reconciliation}\n${scoped}; return rerenderScope;`)(
    { render_empty: () => "", render_unchanged: () => undefined }, root, () => {}, init, () => {},
  );
  render(root, "unchanged"); expect(root.children.length).toBe(1);
  render(root, "empty"); expect(root.children.length).toBe(0);
});

test("a Rust island's empty HTML clears its server-rendered placeholder", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="empty" data-crepus-island-adapter="rust"><p>Loading</p></div>';
  const reconciliation = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── slot-rotate"));
  const islands = source.slice(source.indexOf("function parseIslandProps("), source.indexOf("// ── Event delegation"));
  const mount = new Function("runtime", "initDelegatedEvents", "initInteractive", "currentRoot", `let workerRuntime = null; let currentIslandManifest = { islands: {} };\n${reconciliation}\n${islands}; return initWebIslands;`)(
    { render_empty: () => "" }, init, () => {}, root,
  );
  mount(root); await flush();
  expect(root.firstElementChild!.children.length).toBe(0);
});

test("Rust island rendering discovers handlers absent from the server placeholder", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="widget" data-crepus-island-adapter="rust">Loading</div>';
  const reconciliation = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── slot-rotate"));
  const islands = source.slice(source.indexOf("function parseIslandProps("), source.indexOf("// ── Event delegation"));
  const mount = new Function("runtime", "initDelegatedEvents", "initInteractive", "currentRoot", `let workerRuntime = null; let currentIslandManifest = { islands: {} };\n${reconciliation}\n${islands}; return initWebIslands;`)(
    { render_widget: () => '<form data-onsubmit="child"></form>' }, init, () => {}, root,
  );
  init(root); mount(root); await flush();
  root.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true }));
  await flush(); expect(calls).toEqual(["child"]);
});

test("slot rotation cancels previous intervals on rerender and removal", () => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  const intervals = new Map<number, () => void>();
  let next = 0;
  const clock = {
    setInterval: (tick: () => void) => { intervals.set(++next, tick); return next; },
    clearInterval: (id: number) => intervals.delete(id),
    setTimeout: () => 0,
  };
  const mount = new Function("window", `${slots}; return initSlotRotate;`)(clock);
  root.innerHTML = '<span data-slot-words="first|second"></span>';
  mount(root); expect(intervals.size).toBe(1);
  root.innerHTML = '<span data-slot-words="new|next"></span>';
  mount(root); expect(intervals.size).toBe(1);
  root.innerHTML = "";
  mount(root); expect(intervals.size).toBe(0);
});

test("pending slot transitions cannot update obsolete content", () => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  const intervals: (() => void)[] = [], delays: (() => void)[] = [], frames: (() => void)[] = [];
  const clock = { setInterval: (fn: () => void) => { intervals.push(fn); return intervals.length; }, clearInterval: () => {},
    setTimeout: (fn: () => void) => { delays.push(fn); return delays.length; } };
  const mount = new Function("window", "requestAnimationFrame", `${slots}; return initSlotRotate;`)(clock, (fn: () => void) => frames.push(fn));
  root.innerHTML = '<span data-slot-words="first|second"></span>';
  mount(root);
  const old = root.querySelector(".crepus-slot__inner")!;
  intervals[0]();
  root.innerHTML = '<span data-slot-words="new|next"></span>';
  mount(root); delays[0]();
  expect(old.textContent).toBe("first"); expect(frames.length).toBe(0);
  intervals[1](); delays[1]();
  const current = root.querySelector(".crepus-slot__inner") as HTMLElement;
  expect(current.textContent).toBe("next");
  root.innerHTML = ""; mount(root); frames[0]();
  expect(current.style.opacity).toBe("0");
});

test("removing a slot from its scope stops its interval on the next tick", () => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  let tick: () => void = () => {}; let cleared = false;
  const clock = { setInterval: (fn: () => void) => { tick = fn; return 1; }, clearInterval: () => { cleared = true; }, setTimeout: () => 0 };
  const mount = new Function("window", `${slots}; return initSlotRotate;`)(clock);
  root.innerHTML = '<span data-slot-words="first|second"></span>';
  mount(root); root.replaceChildren(); tick();
  expect(cleared).toBe(true);
});

test("all rotating slots start and removing one leaves its sibling running", () => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  const intervals = new Map<number, () => void>(); const delays: (() => void)[] = [];
  let next = 0;
  const clock = { setInterval: (fn: () => void) => { intervals.set(++next, fn); return next; },
    clearInterval: (id: number) => intervals.delete(id), setTimeout: (fn: () => void) => { delays.push(fn); return 0; } };
  const mount = new Function("window", "requestAnimationFrame", `${slots}; return initSlotRotate;`)(clock, (fn: () => void) => fn());
  root.innerHTML = '<span data-slot-words="only">only</span><span id="one" data-slot-words="a|b"></span><span id="two" data-slot-words="x|y"></span>';
  mount(root); expect(intervals.size).toBe(2);
  for (const tick of intervals.values()) tick();
  for (const delay of delays) delay();
  expect(root.querySelector("#one")!.textContent).toBe("b");
  expect(root.querySelector("#two")!.textContent).toBe("y");
  root.querySelector("#one")!.remove();
  for (const tick of intervals.values()) tick();
  expect(intervals.size).toBe(1);
  mount(root); expect(intervals.size).toBe(1);
  root.innerHTML = ""; mount(root); expect(intervals.size).toBe(0);
});

test.each(["-1", "0", "Infinity", "NaN", "nope", "1.5", "2147483648", "1e100"])("invalid slot interval %s uses the default delay", (interval) => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  let delay = 0;
  const clock = { setInterval: (_: () => void, ms: number) => { delay = ms; return 1; }, clearInterval: () => {} };
  const mount = new Function("window", `${slots}; return initSlotRotate;`)(clock);
  root.innerHTML = '<span data-slot-words="first|second"></span>';
  root.firstElementChild!.setAttribute("data-slot-interval", interval);
  mount(root); expect(delay).toBe(3200);
});

test.each([1, 400, 2147483647])("valid slot interval %d reaches the timer unchanged", (interval) => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  let delay = 0;
  const clock = { setInterval: (_: () => void, ms: number) => { delay = ms; return 1; }, clearInterval: () => {} };
  const mount = new Function("window", `${slots}; return initSlotRotate;`)(clock);
  root.innerHTML = '<span data-slot-words="first|second"></span>';
  root.firstElementChild!.setAttribute("data-slot-interval", String(interval));
  mount(root); expect(delay).toBe(interval);
});

test("overlapping slot transitions display each scheduled word in order", () => {
  const slots = source.slice(source.indexOf("let restoringFocus"), source.indexOf("// ── Web Islands"));
  const delays: (() => void)[] = []; let tick: () => void = () => {};
  const clock = { setInterval: (fn: () => void) => { tick = fn; return 1; }, clearInterval: () => {},
    setTimeout: (fn: () => void) => { delays.push(fn); return 0; } };
  const mount = new Function("window", "requestAnimationFrame", `${slots}; return initSlotRotate;`)(clock, (fn: () => void) => fn());
  root.innerHTML = '<span data-slot-words="first|second|third" data-slot-interval="100"></span>';
  mount(root); tick(); tick(); tick();
  const inner = root.querySelector(".crepus-slot__inner")!;
  delays[0](); expect(inner.textContent).toBe("second");
  delays[1](); expect(inner.textContent).toBe("third");
  delays[2](); expect(inner.textContent).toBe("first");
});

test("scoped renders initialize new slots and nested Rust islands without duplicate event roots", async () => {
  root.innerHTML = '<span data-slot-words="old|next"></span><section id="widget" data-crepus-scope="widget"></section>';
  const intervals = new Map<number, () => void>(); let id = 0;
  const clock = { setInterval: (fn: () => void) => { intervals.set(++id, fn); return id; }, clearInterval: (id: number) => intervals.delete(id), setTimeout: () => 0 };
  const runtime = {
    render_widget: () => '<span data-slot-words="one|two"></span><div data-crepus-island data-crepus-island-src="nested" data-crepus-island-adapter="rust"></div>',
    render_nested: () => '<span data-slot-words="red|blue"></span><form data-onsubmit="child" data-crepus-modifiers-submit="[&quot;prevent&quot;]"></form>',
    child: () => calls.push("child"),
  };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  const start = new Function("runtime", "window", "Element", `${subsystem}; return root => { currentRoot = root; initDelegatedEvents(root); initInteractive(root); return rerenderScope; };`)(runtime, clock, Element);
  const render = start(root);
  expect(intervals.size).toBe(1);
  render(root.querySelector("#widget"), "widget"); await flush();
  expect(intervals.size).toBe(3);
  root.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await flush(); expect(calls).toEqual(["child"]);
});

test("a delayed Rust island result cannot initialize a removed element", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="delayed" data-crepus-island-adapter="rust">Loading</div>';
  const island = root.firstElementChild!;
  let resolve!: (html: string) => void;
  const pending = new Promise<string>(done => { resolve = done; });
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  let intervals = 0;
  const clock = { setInterval: () => { intervals++; return 1; }, clearInterval: () => {} };
  const mount = new Function("runtime", "window", "Element", `${subsystem}; return root => { currentRoot = root; initWebIslands(root); };`)(
    { render_delayed: () => pending }, clock, Element,
  );
  mount(root); await flush(); island.remove();
  resolve('<span data-slot-words="one|two"></span>'); await flush();
  expect(island.textContent).toBe("Loading");
  expect(intervals).toBe(0);
});

test("a Rust island removed before its render starts does not call the export", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="unused" data-crepus-island-adapter="rust"></div>';
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  let rendered = false;
  const mount = new Function("runtime", "Element", `${subsystem}; return root => { currentRoot = root; initWebIslands(root); };`)(
    { render_unused: () => { rendered = true; return ""; } }, Element,
  );
  mount(root); root.replaceChildren(); await flush();
  expect(rendered).toBe(false);
});

test("a module imported after removal does not mount the obsolete island", async () => {
  let release!: (module: { mount: () => void }) => void;
  const pending = new Promise<{ mount: () => void }>(resolve => { release = resolve; });
  let imported = false, mounted = false;
  const island = document.createElement("div");
  island.setAttribute("data-crepus-island", "");
  island.setAttribute("data-crepus-island-src", "./delayed.js");
  root.replaceChildren(island);
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const mount = new Function("runtime", "Element", "loadModule", `${subsystem}; return initWebIslands;`)(
    {}, Element, () => { imported = true; return pending; },
  );
  mount(root); expect(imported).toBe(true);
  island.remove(); release({ mount: () => { mounted = true; } }); await flush();
  expect(mounted).toBe(false);
});


test.each([false, true])("module load failure diagnostics respect removal=%s", async (removed) => {
  let reject!: (error: Error) => void;
  const pending = new Promise<never>((_, fail) => { reject = fail; });
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="./failure.js">Loading</div>';
  const island = root.firstElementChild!;
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const mount = new Function("runtime", "Element", "loadModule", `${subsystem}; return initWebIslands;`)({}, Element, () => pending);
  const diagnostic = spyOn(console, "error").mockImplementation(() => {});
  try {
    mount(root); if (removed) island.remove();
    reject(new Error("Unable to load island")); await flush();
    expect(diagnostic.mock.calls.length).toBe(removed ? 0 : 1);
    expect(island.textContent).toBe(removed ? "Loading" : "Error: Unable to load island");
  } finally { diagnostic.mockRestore(); }
});

test("framework replacement disposes mounted modules exactly once before detaching", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="./cleanup.js">Loading</div>';
  const island = root.firstElementChild!;
  let disposed = 0, attached = false;
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const api = new Function("runtime", "Element", "loadModule", `${subsystem}; return { initWebIslands, setRootHtml };`)(
    {}, Element, () => Promise.resolve({ mount: () => () => { disposed++; attached = island.isConnected; } }),
  );
  api.initWebIslands(root); await flush();
  api.setRootHtml(root, "<p>Replacement</p>"); api.setRootHtml(root, "");
  expect(disposed).toBe(1); expect(attached).toBe(true);
});

test("an async mount completing after replacement disposes immediately", async () => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="./async.js"></div>';
  let release!: (dispose: () => void) => void;
  const pending = new Promise<() => void>(resolve => { release = resolve; });
  let disposed = 0;
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const api = new Function("runtime", "Element", "loadModule", `${subsystem}; return { initWebIslands, setRootHtml };`)(
    {}, Element, () => Promise.resolve({ mount: () => pending }),
  );
  api.initWebIslands(root); await flush(); api.setRootHtml(root, "");
  release(() => { disposed++; }); await flush();
  expect(disposed).toBe(1);
});

test.each([false, true])("a failing island cleanup does not block replacement (async=%s)", async (asyncFailure) => {
  root.innerHTML = '<div data-crepus-island data-crepus-island-src="./bad.js"></div><div data-crepus-island data-crepus-island-src="./good.js"></div>';
  let disposed = 0;
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const api = new Function("runtime", "Element", "loadModule", `${subsystem}; return { initWebIslands, setRootHtml };`)(
    {}, Element, (path: string) => Promise.resolve({ mount: () => path === "./bad.js" ? () => { if (asyncFailure) return Promise.reject(new Error("Cleanup failed")); throw new Error("Cleanup failed"); } : () => { disposed++; } }),
  );
  const diagnostic = spyOn(console, "error").mockImplementation(() => {});
  try {
    api.initWebIslands(root); await flush(); api.setRootHtml(root, "<p>Updated</p>"); await flush();
    expect(disposed).toBe(1); expect(root.textContent).toBe("Updated"); expect(diagnostic.mock.calls.length).toBe(1);
  } finally { diagnostic.mockRestore(); }
});


test.each([false, true])("composition inputs defer rerender while forwarding state (scoped=%s)", async (scoped) => {
  if (scoped) document.getElementById("parent")!.setAttribute("data-crepus-scope", "widget");
  const child = bind("child", "input");
  init(root);
  child.dispatchEvent(new InputEvent("input", { bubbles: true, isComposing: true }));
  await flush(); expect(calls).toEqual(["child"]);
  expect(renders).toBe(0); expect(scopes).toEqual([]);
  child.dispatchEvent(new InputEvent("input", { bubbles: true, isComposing: false }));
  await flush();
  expect(renders).toBe(scoped ? 0 : 1); expect(scopes).toEqual(scoped ? ["widget"] : []);
});

test.each(["keydown", "keyup"])("composing %s does not replace the root", async (eventName) => {
  const child = bind("child", eventName); init(root);
  child.dispatchEvent(new KeyboardEvent(eventName, { bubbles: true, isComposing: true, key: "Process" }));
  await flush(); expect(calls).toEqual(["child"]); expect(renders).toBe(0);
});

test("rich-text bracket insertion does not intercept composition keys", async () => {
  document.getElementById("parent")!.setAttribute("data-crepus-scope", "editor");
  const child = bind("child", "keydown"); child.setAttribute("contenteditable", "true"); init(root);
  const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, isComposing: true, key: "(" });
  child.dispatchEvent(event); await flush();
  expect(event.defaultPrevented).toBe(false); expect(calls).toEqual(["child"]);
});

test("submit actions snapshot named fields, duplicates, checked controls, and the submitter", async () => {
  root.innerHTML = '<form id="child"><input name="title" value="draft"><input name="tag" value="a"><input name="tag" value="b"><input name="hidden" value="skip" disabled><input type="checkbox" name="yes" value="1" checked><input type="checkbox" name="no" value="0"><button name="intent" value="save">Save</button></form>';
  const form = bind("child", "submit", ["prevent"]); init(root);
  form.dispatchEvent(new SubmitEvent("submit", { bubbles: true, cancelable: true, submitter: form.querySelector("button") }));
  await flush();
  expect(payloads[0]._form).toEqual([["title", "draft"], ["tag", "a"], ["tag", "b"], ["yes", "1"], ["intent", "save"]]);
});

test("form submission snapshots precede WASM readiness", async () => {
  root.innerHTML = '<form id="child"><textarea name="body">original</textarea></form>';
  const form = bind("child", "submit", ["prevent"]);
  let ready!: () => void; init(root, new Promise<void>(resolve => { ready = resolve; }));
  form.dispatchEvent(new SubmitEvent("submit", { bubbles: true, cancelable: true }));
  form.querySelector("textarea")!.value = "later";
  expect(calls).toEqual([]); ready(); await flush();
  expect(payloads[0]._form).toEqual([["body", "original"]]);
});

test("form file fields serialize metadata without file contents", async () => {
  root.innerHTML = '<form id="child"><input type="file" name="attachment"></form>';
  const form = bind("child", "submit", ["prevent"]);
  const transfer = new DataTransfer(); transfer.items.add(new File(["contents"], "note.txt", { type: "text/plain", lastModified: 123 }));
  form.querySelector("input")!.files = transfer.files;
  init(root); form.dispatchEvent(new SubmitEvent("submit", { bubbles: true, cancelable: true })); await flush();
  expect(payloads[0]._form).toEqual([["attachment", { name: "note.txt", type: "text/plain", size: 8, lastModified: 123 }]]);
});

test("multiple handlers preserve capture order and omit handler metadata from payloads", async () => {
  const child = bind("child", "click");
  child.setAttribute("data-crepus-handlers-click", JSON.stringify([{ handler: "child", modifiers: [] }, { handler: "parent", modifiers: ["capture"] }]));
  init(root); child.click(); await flush();
  expect(calls).toEqual(["parent", "child"]); expect(payloads).toEqual([{}, {}]);
});

test("stop preserves sibling listeners but blocks ancestor handlers", async () => {
  bind("root", "click"); const child = bind("child", "click");
  child.setAttribute("data-crepus-handlers-click", JSON.stringify([{ handler: "child", modifiers: ["stop"] }, { handler: "parent", modifiers: [] }]));
  init(root); child.click(); await flush(); expect(calls).toEqual(["child", "parent"]);
});

test("identical once handlers each retain their own lifetime", async () => {
  const child = bind("child", "click");
  child.setAttribute("data-crepus-handlers-click", JSON.stringify([{ handler: "child", modifiers: ["once"] }, { handler: "child", modifiers: ["once"] }]));
  init(root); child.click(); child.click(); await flush(); expect(calls).toEqual(["child", "child"]);
});

test("malformed multi-handler metadata does not fall back to a partial listener", async () => {
  const child = bind("child", "click"); child.setAttribute("data-crepus-handlers-click", '[{"handler":"child","modifiers":true}]');
  init(root); child.click(); await flush(); expect(calls).toEqual([]);
});

test.each([false, true])("nonbubbling target listeners use native phase order (capture stop=%s)", async (stop) => {
  const child = bind("child", "focus");
  child.setAttribute("data-crepus-handlers-focus", JSON.stringify([
    { handler: "child", modifiers: [] },
    { handler: "parent", modifiers: stop ? ["capture", "stop"] : ["capture"] },
    { handler: "root", modifiers: ["capture"] },
  ]));
  init(root); child.dispatchEvent(new Event("focus", { bubbles: false })); await flush();
  expect(calls).toEqual(stop ? ["parent", "root"] : ["parent", "root", "child"]);
});

test.each([["immediate"], ["immediate", "stop"], ["stop", "immediate"]])("immediate-stop chains block subsequent listeners: %j", async (...modifiers) => {
  bind("root", "click"); const child = bind("child", "click");
  child.setAttribute("data-crepus-handlers-click", JSON.stringify([{ handler: "child", modifiers }, { handler: "parent", modifiers: [] }]));
  init(root); let native = false; root.addEventListener("click", () => { native = true; });
  child.click(); await flush(); expect(calls).toEqual(["child"]); expect(native).toBe(false);
});

test("immediate-stop and self modifiers preserve their declared order", async () => {
  const parent = bind("parent", "click", ["immediate", "self"]); bind("root", "click"); init(root);
  document.getElementById("inner")!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flush(); expect(calls).toEqual([]);
  parent.setAttribute("data-crepus-modifiers-click", JSON.stringify(["self", "immediate"]));
  document.getElementById("inner")!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flush(); expect(calls).toEqual(["root"]);
});

test("trusted handlers reject synthetic events and accept trusted browser events", async () => {
  const child = bind("child", "click", ["trusted"]); init(root);
  child.dispatchEvent(new MouseEvent("click", { bubbles: true })); await flush(); expect(calls).toEqual([]);
  const trusted = new MouseEvent("click", { bubbles: true });
  Object.defineProperty(trusted, "isTrusted", { value: true });
  child.dispatchEvent(trusted); await flush(); expect(calls).toEqual(["child"]);
});

test("once lifetime is consumed before the trusted guard", async () => {
  const child = bind("child", "click", ["once", "trusted"]); init(root);
  child.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  const trusted = new MouseEvent("click", { bubbles: true }); Object.defineProperty(trusted, "isTrusted", { value: true });
  child.dispatchEvent(trusted); await flush(); expect(calls).toEqual([]);
});

test("Rust island props survive delegated and explicit scope rerenders while ordinary scopes receive empty props", async () => {
  root.innerHTML = '<section data-crepus-island data-crepus-island-src="editor" data-crepus-island-adapter="rust" data-crepus-scope="editor"></section><section id="ordinary" data-crepus-scope="ordinary"></section>';
  const island = root.firstElementChild!;
  island.setAttribute("data-crepus-island-props", JSON.stringify({ label: 'Authored 雪 <&"', count: 3 }));
  const received: unknown[] = [];
  const runtime = {
    render_editor: (json: string) => {
      const props = JSON.parse(json);
      received.push(props);
      return '<button data-onclick="update">Updated</button>';
    },
    render_ordinary: (json: string) => { received.push(JSON.parse(json)); return "ordinary"; },
    update: () => calls.push("update"),
  };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  const explicitEvents = source.slice(source.indexOf('window.addEventListener("crepus-update-bundle"'), source.indexOf("async function main()"));
  const render = new Function("runtime", "window", "Element", `${subsystem}\n${explicitEvents}\nreturn root => { currentRoot = root; initDelegatedEvents(root); initInteractive(root); return rerenderScope; };`)(runtime, window, Element)(root);
  await flush();
  island.querySelector("button")!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flush();
  window.dispatchEvent(new Event("crepus-rerender-editor"));
  render(root.querySelector("#ordinary"), "ordinary");
  expect(calls).toEqual(["update"]);
  expect(received).toEqual([
    { label: 'Authored 雪 <&"', count: 3 },
    { label: 'Authored 雪 <&"', count: 3 },
    { label: 'Authored 雪 <&"', count: 3 },
    {},
  ]);
});

test("a pending initial Rust island result cannot overwrite a newer attached scope render", async () => {
  root.innerHTML = '<section data-crepus-island data-crepus-island-src="widget" data-crepus-island-adapter="rust" data-crepus-scope="widget">Loading</section>';
  const island = root.firstElementChild!;
  let release!: (html: string) => void;
  const initial = new Promise<string>(resolve => { release = resolve; });
  let renders = 0;
  const runtime = { render_widget: () => ++renders === 1 ? initial : "<p>NEW</p>" };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  const render = new Function("runtime", "window", "Element", `${subsystem}; return root => { currentRoot = root; initInteractive(root); return rerenderScope; };`)(runtime, window, Element)(root);
  await flush();
  render(island, "widget");
  expect(island.textContent).toBe("NEW");
  release("<p>OLD</p>");
  await flush();
  expect(island.isConnected).toBe(true);
  expect(island.textContent).toBe("NEW");
  expect(renders).toBe(2);
});

test("a superseded queued Rust island mount skips its export without canceling another island", async () => {
  root.innerHTML = '<section id="widget" data-crepus-island data-crepus-island-src="widget" data-crepus-island-adapter="rust" data-crepus-scope="widget">Loading</section><section id="other" data-crepus-island data-crepus-island-src="other" data-crepus-island-adapter="rust">Other loading</section>';
  const rendered: string[] = [];
  const runtime = {
    render_widget: () => { rendered.push("widget"); return "<p>NEW</p>"; },
    render_other: () => { rendered.push("other"); return "<p>OTHER</p>"; },
  };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  const render = new Function("runtime", "window", "Element", `${subsystem}; return root => { currentRoot = root; initInteractive(root); return rerenderScope; };`)(runtime, window, Element)(root);
  render(root.querySelector("#widget"), "widget");
  await flush();
  expect(rendered).toEqual(["widget", "other"]);
  expect(root.querySelector("#widget")!.textContent).toBe("NEW");
  expect(root.querySelector("#other")!.textContent).toBe("OTHER");
});

test.each(["normal", "throw", "reject"])("a parent island failure disposes its mounted child before detaching (cleanup=%s)", async (cleanup) => {
  root.innerHTML = '<section data-crepus-island data-crepus-island-src="./parent.js"><section id="nested-child" data-crepus-island data-crepus-island-src="./child.js"></section></section>';
  const parent = root.firstElementChild!;
  const child = root.querySelector("#nested-child")!;
  let rejectParent!: (error: Error) => void;
  const pendingParent = new Promise<never>((_, reject) => { rejectParent = reject; });
  let mounted = 0, disposed = 0, attachedAtCleanup = false;
  const loadModule = (path: string) => path === "./parent.js" ? pendingParent : Promise.resolve({
    mount: () => {
      mounted++;
      return () => {
        disposed++;
        attachedAtCleanup = child.isConnected;
        if (cleanup === "throw") throw new Error("Cleanup failed synchronously");
        if (cleanup === "reject") return Promise.reject(new Error("Cleanup failed asynchronously"));
      };
    },
  });
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const api = new Function("runtime", "Element", "loadModule", `${subsystem}; return { initWebIslands, setRootHtml };`)({}, Element, loadModule);
  const diagnostic = spyOn(console, "error").mockImplementation(() => {});
  try {
    api.initWebIslands(root);
    await flush();
    expect(mounted).toBe(1);
    rejectParent(new Error("Parent failed"));
    await flush();
    expect(disposed).toBe(1);
    expect(attachedAtCleanup).toBe(true);
    expect(child.isConnected).toBe(false);
    expect(parent.querySelector("pre")!.textContent).toBe("Error: Parent failed");
    expect(diagnostic.mock.calls.length).toBe(cleanup === "normal" ? 1 : 2);
    api.setRootHtml(root, "");
    await flush();
    expect(disposed).toBe(1);
  } finally { diagnostic.mockRestore(); }
});

test("scoped replacement stops outer-owned slot intervals immediately and preserves siblings", () => {
  root.innerHTML = '<section id="widget"><span data-slot-words="old|next" data-slot-interval="2147483647"></span></section><aside><span data-slot-words="sibling|next" data-slot-interval="2147483647"></span></aside>';
  const intervals = new Map<number, { tick: () => void; delay: number }>();
  let id = 0;
  const clock = {
    setInterval(tick: () => void, delay: number) { intervals.set(++id, { tick, delay }); return id; },
    clearInterval(timer: number) { intervals.delete(timer); },
    setTimeout: () => 0,
  };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"));
  const api = new Function("runtime", "window", "Element", `${subsystem}; return {
    start(root) { currentRoot = root; initInteractive(root); },
    render: rerenderScope,
    init: initSlotRotate
  };`)( { render_widget: () => '<span data-slot-words="new|next" data-slot-interval="2147483647"></span>' }, clock, Element);
  api.start(root);
  const originalTimers = [...intervals.keys()];
  expect(originalTimers).toHaveLength(2);
  expect([...intervals.values()].map(timer => timer.delay)).toEqual([2147483647, 2147483647]);
  const scope = root.querySelector("#widget")!;
  api.render(scope, "widget");
  expect(intervals.has(originalTimers[0])).toBe(false);
  expect(intervals.has(originalTimers[1])).toBe(true);
  expect(intervals.size).toBe(2);
  api.init(scope);
  api.init(root);
  expect(intervals.size).toBe(2);
  api.render(scope, "widget");
  expect(intervals.size).toBe(2);
});

test("failed module island immediately stops descendant slot rotation before replacing content", async () => {
  root.innerHTML = '<section data-crepus-island data-crepus-island-src="./pending.js"><span data-slot-words="old|next" data-slot-interval="2147483647"></span></section>';
  const parent = root.firstElementChild!;
  let rejectModule!: (error: Error) => void;
  const loading = new Promise<never>((_, reject) => { rejectModule = reject; });
  const intervals = new Set<number>();
  let id = 0;
  const clock = {
    setInterval: () => { intervals.add(++id); return id; },
    clearInterval: (timer: number) => intervals.delete(timer),
  };
  const subsystem = source.slice(source.indexOf("const delegatedEvents"), source.indexOf("// ── Bootstrap"))
    .replace("import(modulePath)", "loadModule(modulePath)");
  const mount = new Function("runtime", "window", "Element", "loadModule", `${subsystem}; return initInteractive;`)({}, clock, Element, () => loading);
  const diagnostic = spyOn(console, "error").mockImplementation(() => {});
  try {
    mount(root);
    expect(intervals.size).toBe(1);
    rejectModule(new Error("Module failed"));
    await flush();
    expect(intervals.size).toBe(0);
    expect(parent.querySelector("pre")!.textContent).toBe("Error: Module failed");
  } finally { diagnostic.mockRestore(); }
});
