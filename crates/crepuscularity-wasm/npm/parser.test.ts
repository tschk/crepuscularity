import { expect, test } from "bun:test";
import { IR_VERSION, parseCrepus, parseTemplate } from "./index";
import { parse_template_json as parseLegacyTemplate } from "../pkg/crepuscularity_wasm.js";

test.each([
  ["App.crepus", 'if {true}\n  p\n    "ok"'],
  ["App.tsx", '<if condition={true}><p>ok</p></if>'],
  ["App.svelte", '{#if true}<p>ok</p>{/if}'],
  ["App.vue", '<template><p v-if="true">ok</p></template>'],
  ["App.astro", '{true && <p>ok</p>}'],
  ["App.component.html", '@if (true) { <p>ok</p> }'],
])("%s selects its frontend through WASM", (filename, source) => {
  const ir = parseTemplate(source, filename);
  expect(ir.version).toBe(IR_VERSION);
  expect(ir.root[0].kind).toBe("if");
  expect(JSON.stringify(ir)).toContain("ok");
});

test("JSX else-if branches survive the JavaScript bridge", () => {
  const ir = parseTemplate(
    '<if condition={a}><p>A</p><else-if condition={b}><p>B</p></else-if><else><p>C</p></else></if>',
    "App.tsx",
  );
  expect(ir.root[0]).toMatchObject({
    kind: "if",
    condition: "a",
    elseChildren: [{ kind: "if", condition: "b" }],
  });
});

test("quoted event names and invalid handlers cross the WASM bridge correctly", () => {
  for (const filename of ["App.tsx", "App.svelte", "App.astro"]) {
    for (const attribute of ['@click="save"', 'onClick="save"']) {
      expect(parseTemplate(`<button ${attribute}>Save</button>`, filename).root[0])
        .toMatchObject({ kind: "button", onClick: "save" });
    }
    for (const attribute of ['@click=""', "onClick", "@keydown||Enter={save}"]) {
      expect(() => parseTemplate(`<button ${attribute} />`, filename)).toThrow();
    }
  }
});

test("Vue event directives require handlers through WASM", () => {
  for (const directive of ["@click", "v-on:click"]) {
    expect(parseTemplate(`<template><button ${directive}="save">Save</button></template>`, "App.vue").root[0])
      .toMatchObject({ kind: "button", onClick: "save" });
    for (const suffix of ["", '.stop', '=""', '="   "']) {
      expect(() => parseTemplate(`<template><button ${directive}${suffix} /></template>`, "App.vue"))
        .toThrow("nonempty handler");
    }
  }
});

test("malformed Unicode markup throws a parser error through WASM", () => {
  expect(() => parseTemplate(`<p /></${"界".repeat(20)}>`, "App.tsx"))
    .toThrow("unexpected trailing markup");
});

test("quoted braces survive the JavaScript bridge", () => {
  const ir = parseTemplate('<p>{"}"}</p>', "App.tsx");
  expect(JSON.stringify(ir)).toContain('"content":"}"');
});


test("legacy pkg bindings expose the same frontend entry point", () => {
  const ir = JSON.parse(parseLegacyTemplate('{true && <p>ok</p>}', "App.astro"));
  expect(ir.version).toBe(IR_VERSION);
  expect(ir.root[0].kind).toBe("if");
});

test("context booleans, zero, and null retain their truthiness", () => {
  for (const enabled of [false, 0, null]) {
    const ir = parseTemplate('<p class:active={enabled}>ok</p>', "App.svelte", { enabled });
    expect(JSON.stringify(ir)).not.toContain("active");
  }
  const ir = parseTemplate('<p class:active={enabled}>ok</p>', "App.svelte", { enabled: true });
  expect(JSON.stringify(ir)).toContain("active");
});

test("context numbers remain numeric in expressions", () => {
  for (const count of [2, 2.5]) {
    const ir = parseTemplate('<p>{count + 1}</p>', "App.tsx", { count });
    expect(JSON.stringify(ir)).toContain(`"content":"${count + 1}"`);
  }
});


test("context strings retain string semantics", () => {
  const ir = parseTemplate('<p>{count + 1}</p>', "App.tsx", { count: "2" });
  expect(JSON.stringify(ir)).toContain('"content":"21"');
});


test("JSX match rejects tag names that only start with case", () => {
  for (const tag of ["casework", "case-widget", "case界"]) {
    expect(() => parseTemplate(`<match on={value}><${tag} /></match>`, "App.tsx"))
      .toThrow();
  }
});


test("self-closing JSX control tags cannot consume following content", () => {
  for (const source of [
    '<if condition={ok} /><p /></if>',
    '<for let="item" in={items} /><p /></for>',
    '<match on={value} /><case /></match>',
    '<if condition={a}><else-if condition={b} /><p /></else-if></if>',
  ]) {
    expect(() => parseTemplate(source, "App.tsx")).toThrow("cannot be self-closing");
  }
});


test("unterminated JSX attributes throw parser errors instead of trapping", () => {
  for (const source of ["<p title='", "<p title='界", '<p title="', '<p title="界']) {
    expect(() => parseTemplate(source, "App.tsx")).toThrow("unclosed quoted JSX attribute");
  }
});

test("unsupported Svelte event semantics fail through the WASM parser", () => {
  for (const modifier of ["passive", "nonpassive"]) {
    expect(() => parseTemplate(`<button on:click|${modifier}={save} />`, "App.svelte"))
      .toThrow("unsupported Svelte event modifier");
  }
});

test("View IR rejects unsupported immediate-stop semantics", () => {
  expect(() => parseTemplate('<button on:click|stopImmediatePropagation={save} />', "App.svelte"))
    .toThrow("View IR does not support event modifiers");
});

test("View IR rejects unsupported trusted semantics", () => {
  expect(() => parseTemplate('<button on:click|trusted={save} />', "App.svelte")).toThrow("View IR does not support event modifiers");
});


test("View IR does not silently discard extra event handlers", () => {
  expect(() => parseTemplate('<button on:click={first} on:click={second} />', "App.svelte"))
    .toThrow("View IR supports only one");
});

test("View IR file-picker event aliases cannot hide another handler", () => {
  expect(() => parseTemplate('<file-picker @pick={first} @click={second} />', "Picker.jsx"))
    .toThrow("only one `pick` handler");
});

test("View IR preserves native long-press actions", () => {
  expect(parseTemplate('<button onLongPress={hold} />', "Gesture.jsx").root[0])
    .toMatchObject({ kind: "button", onLongPress: "hold" });
  expect(() => parseTemplate('<button @longpress={first} onLongPress={second} />', "Gesture.jsx"))
    .toThrow("only one `longpress` handler");
});


test("View IR rejects bare events that its nodes cannot preserve", () => {
  for (const [filename, source] of [
    ["App.crepus", "button @keydown=save"],
    ["App.jsx", "<button onKeyDown={save} />"],
    ["App.svelte", "<button on:keydown={save} />"],
    ["App.vue", '<template><button @keydown="save" /></template>'],
    ["App.astro", "<button onKeyDown={save} />"],
    ["App.ng", '<button (keydown)="save"></button>'],
  ]) {
    expect(() => parseTemplate(source, filename)).toThrow("does not support `keydown` handlers");
  }
});


test("View IR rejects event handlers in flattened label metadata", () => {
  for (const [filename, source] of [
    ["App.crepus", 'button\n  "First"\n  span @click=lost\n    "Later"'],
    ["App.jsx", "<button>First<span onClick={lost}>Later</span></button>"],
    ["App.svelte", "<button>First<span on:click={lost}>Later</span></button>"],
    ["App.vue", '<template><button>First<span @click="lost">Later</span></button></template>'],
    ["App.astro", "<button>First<span onClick={lost}>Later</span></button>"],
    ["App.ng", '<button>First<span (click)="lost">Later</span></button>'],
  ]) {
    expect(() => parseTemplate(source, filename)).toThrow("metadata-only children");
  }
});


test("checkbox input types lower to native checkbox controls", () => {
  expect(parseTemplate('<input type="checkbox" checked bind={enabled} onChange={save} />', "App.jsx").root[0])
    .toMatchObject({kind: "checkbox", checked: true, bind: "enabled", onChange: "save"});
});


test("native slider initial values stay inside declared bounds", () => {
  expect(parseTemplate('<slider min={10} max={20} />', "App.jsx").root[0])
    .toMatchObject({kind: "slider", min: 10, max: 20, value: 10});
  expect(() => parseTemplate('<slider min={-3e38} max={3e38} />', "App.jsx"))
    .toThrow("range width must be finite");
});


test("abbreviated decimal expressions reach numeric View IR properties", () => {
  expect(parseTemplate('<slider min={-.5} max={2.} value={.5e0} step={.25} />', "App.jsx").root[0])
    .toMatchObject({kind: "slider", min: -0.5, max: 2, value: 0.5, step: 0.25});
});

test("public IR metadata survives the WASM package bridge", () => {
  const ir = parseTemplate(
    'input type=number name=age aria-label="Age" required value=20 min=18 max=99 step=2 @input=edit\nbutton "Hello {name}"',
    "Fields.crepus",
    { name: "Ada" },
  );
  const input = ir.root[0];
  expect(input.kind).toBe("input");
  if (input.kind !== "input") throw new Error("expected input");
  expect(input.onInput).toBe("edit");
  expect(input.style).toMatchObject({
    ariaLabel: "Age",
    formName: "age",
    formValue: "20",
    inputType: "number",
    required: true,
    formMin: "18",
    formMax: "99",
    formStep: "2",
  });
  const button = ir.root[1];
  expect(button.kind).toBe("button");
  if (button.kind !== "button") throw new Error("expected button");
  expect(button.labelParts).toEqual([
    { kind: "literal", value: "Hello " },
    { kind: "expr", value: "name" },
  ]);
});

test.each([
  ["App.crepus", 'checkbox checked={null}\ncheckbox checked={missing.path}\nfile-picker multiple={null} class=multiple'],
  ["App.jsx", '<checkbox checked={null}/><checkbox checked={missing.path}/><file-picker multiple={null} class="multiple"/>'],
  ["App.svelte", '<checkbox checked={null}/><checkbox checked={missing.path}/><file-picker multiple={null} class="multiple"/>'],
  ["App.vue", '<template><checkbox :checked="null"/><checkbox :checked="missing.path"/><file-picker :multiple="null" class="multiple"/></template>'],
  ["App.astro", '<checkbox checked={null}/><checkbox checked={missing.path}/><file-picker multiple={null} class="multiple"/>'],
  ["App.ng", '<checkbox [checked]="null"/><checkbox [checked]="missing.path"/><file-picker [multiple]="null" class="multiple"/>'],
])("%s keeps null and missing control booleans false through WASM", (filename, source) => {
  const ir = parseTemplate(source, filename);
  expect(ir.root[0]).toMatchObject({ kind: "checkbox", checked: false });
  expect(ir.root[1]).toMatchObject({ kind: "checkbox", checked: false });
  expect(ir.root[2]).toMatchObject({ kind: "filePicker", multiple: false });
});

test.each([
  ["App.crepus", 'input type=file accept="image/*,.txt" multiple=true required=true name=files @change=files.pick'],
  ["App.jsx", '<input type="file" accept="image/*,.txt" multiple required name="files" onChange={files.pick}/>'],
  ["App.svelte", '<input type="file" accept="image/*,.txt" multiple required name="files" on:change={files.pick}/>'],
  ["App.vue", '<template><input type="file" accept="image/*,.txt" multiple required name="files" @change="files.pick"/></template>'],
  ["App.astro", '<input type="file" accept="image/*,.txt" multiple required name="files" onChange={files.pick}/>'],
  ["App.ng", '<input type="file" accept="image/*,.txt" multiple required name="files" (change)="files.pick"/>'],
])("%s lowers standard file inputs through the public WASM API", (filename, source) => {
  expect(parseTemplate(source, filename).root[0]).toMatchObject({
    kind: "filePicker",
    accept: ["image/*", ".txt"],
    multiple: true,
    onPick: "files.pick",
    style: { formName: "files", required: true },
  });
});

test("the WASM View IR boundary rejects duplicate consumed bindings", () => {
  for (const [source, property] of [
    ['<checkbox checked={true} checked={false}/>', "checked"],
    ['<file-picker multiple={true} multiple={false}/>', "multiple"],
    ['<input placeholder="first" placeholder="second"/>', "placeholder"],
    ['<input bind="first.path" bind="second.path"/>', "bind"],
  ]) {
    expect(() => parseTemplate(source, "App.jsx")).toThrow(`duplicate ${property} binding`);
  }
});

test("the WASM View IR boundary rejects unrepresented control state", () => {
  for (const attribute of ["disabled", "readonly", "readOnly"]) {
    expect(() => parseTemplate(`<input ${attribute}/>`, "App.jsx"))
      .toThrow("unsupported View IR control state");
    expect(() => parseTemplate(`<button ${attribute}={false}>Action</button>`, "App.jsx"))
      .toThrow("unsupported View IR control state");
  }
});

test("public parser APIs reject nonfinite context numbers before JSON null conversion", () => {
  for (const value of [NaN, Infinity, -Infinity]) {
    const context = { value };
    for (const parse of [
      () => parseCrepus('span "{value}"', context),
      () => parseTemplate('<span>{value}</span>', "App.jsx", context),
    ]) {
      expect(parse).toThrow(TypeError);
      expect(parse).toThrow("context numbers must be finite");
    }
    expect(Object.is(context.value, value)).toBe(true);
  }
});

test("public parser context serialization preserves finite values and reads getters once", () => {
  for (const value of [0, -7, 1.25, null]) {
    for (const kind of ["crepus", "template"]) {
      let reads = 0;
      const context = { get value() { reads++; return value; } };
      const before = Object.getOwnPropertyDescriptor(context, "value");
      const ir = kind === "crepus"
        ? parseCrepus('span "{value}"', context)
        : parseTemplate('<span>{value}</span>', "App.jsx", context);
      expect(ir.root[0]).toMatchObject({ kind: "text", content: value === null ? "" : String(value) });
      expect(reads).toBe(1);
      expect(Object.getOwnPropertyDescriptor(context, "value")).toEqual(before);
      expect(Object.keys(context)).toEqual(["value"]);
    }
  }
});

test.each([
  ["App.svelte", "{@html markup}"],
  ["App.svelte", "<button>{@html markup}</button>"],
  ["App.svelte", '<button label="Explicit">{@html markup}</button>'],
  ["App.vue", '<template><p v-html="markup"/></template>'],
  ["App.vue", '<template><button v-html="markup"/></template>'],
  ["App.vue", '<template><button label="Explicit" v-html="markup"/></template>'],
])("%s rejects raw HTML instead of escaping or dropping it in public View IR", (filename, source) => {
  expect(() => parseTemplate(source, filename, { markup: "<b>bold</b>" }))
    .toThrow("View IR does not support raw HTML");
});

test("Vue v-text still preserves markup-looking values as ordinary text", () => {
  expect(parseTemplate('<template><p v-text="markup"/></template>', "App.vue", {
    markup: "<b>bold</b>",
  }).root[0]).toMatchObject({ kind: "text", content: "<b>bold</b>" });
});

test("View IR rejects browser islands and preserves standalone webview embeds", () => {
  for (const source of [
    'embed ./widget.ts',
    'embed ./widget.ts adapter="module"',
    'embed ./Widget.tsx adapter="react"',
  ]) {
    expect(() => parseCrepus(source)).toThrow("View IR does not support embedded islands");
  }
  expect(() => parseTemplate(
    '<button><island src="https://example.com" adapter="webview"/></button>', "App.jsx",
  )).toThrow("View IR does not support embedded islands");
  expect(parseCrepus('embed https://example.com adapter="webview"').root[0])
    .toMatchObject({ kind: "webView", src: "https://example.com" });
});

test("Vue v-text survives flattened button labels as a live expression", () => {
  const ir = parseTemplate('<template><button><span v-text="caption"/></button></template>',
    "App.vue", { caption: "<b>literal</b>" });
  expect(ir.root[0]).toMatchObject({
    kind: "button", label: "<b>literal</b>",
    labelParts: [{ kind: "expr", value: "caption" }],
  });
});

test("indentation text expressions survive flattened button labels", () => {
  expect(parseCrepus('button\n  {caption}', { caption: "<b>literal</b>" }).root[0])
    .toMatchObject({ kind: "button", label: "<b>literal</b>",
      labelParts: [{ kind: "expr", value: "caption" }] });
});

test("View IR rejects control flow that flattened labels or options cannot retain", () => {
  for (const source of [
    '<button>{#if true}Visible{:else}Hidden{/if}</button>',
    '<picker>{#if true}<option value="one">One</option>{/if}</picker>',
  ]) {
    expect(() => parseTemplate(source, "App.svelte"))
      .toThrow("View IR does not support control flow in metadata-only children");
  }
});

test("initially empty nested button labels retain their live expression", () => {
  const contexts: Record<string, string | null>[] = [{}, { caption: null }, { caption: "" }];
  for (const context of contexts) {
    const source = '<template><button><span v-text="caption"/><span>Fallback</span></button></template>';
    expect(parseTemplate(source, "App.vue", context).root[0]).toMatchObject({
      kind: "button", label: "", labelParts: [{ kind: "expr", value: "caption" }],
    });
    expect(parseCrepus('button\n  span\n    {caption}', context).root[0]).toMatchObject({
      kind: "button", label: "", labelParts: [{ kind: "expr", value: "caption" }],
    });
  }
});

test("empty explicit labels preserve the child expression selected for display", () => {
  for (const label of ['""', '"  "', "{null}", "{missing}"]) {
    expect(parseTemplate(`<button label=${label}>{caption}</button>`, "App.jsx", {
      caption: "Child",
    }).root[0]).toMatchObject({
      kind: "button", label: "Child", labelParts: [{ kind: "expr", value: "caption" }],
    });
  }
});

test("button form behavior cannot silently become an ordinary button through WASM", () => {
  for (const type of ["submit", "reset", "menu", ""]) {
    expect(() => parseTemplate(`<button type="${type}">Action</button>`, "App.jsx"))
      .toThrow("unsupported View IR button type");
    expect(() => parseLegacyTemplate(`<button type="${type}">Action</button>`, "App.jsx"))
      .toThrow("unsupported View IR button type");
  }
  for (const source of ['<button>Action</button>', '<button type="button">Action</button>']) {
    expect(parseTemplate(source, "App.jsx").root[0].kind).toBe("button");
  }
});

test("standard checkbox checked attributes use HTML presence without changing custom controls", () => {
  for (const [source, checked] of [
    ['<input type="checkbox" checked="false"/>', true],
    ['<input type="checkbox" checked="checked"/>', true],
    ['<input type="checkbox" checked={false}/>', false],
    ['<input type="checkbox" checked={null}/>', false],
    ['<input type="checkbox"/>', false],
    ['<checkbox checked="false"/>', false],
  ] as const) {
    expect(parseTemplate(source, "App.jsx").root[0]).toMatchObject({ kind: "checkbox", checked });
  }
});

test("multiple selection cannot silently become a single-value picker", () => {
  for (const attribute of ["multiple", 'multiple="false"', "multiple={true}"]) {
    expect(() => parseTemplate(`<select ${attribute}><option value="one">One</option></select>`, "App.jsx"))
      .toThrow("View IR pickers do not support multiple selection");
    expect(() => parseLegacyTemplate(`<select ${attribute}><option value="one">One</option></select>`, "App.jsx"))
      .toThrow("View IR pickers do not support multiple selection");
  }
  for (const source of [
    '<select><option value="one">One</option></select>',
    '<select multiple={false}><option value="one">One</option></select>',
    '<select multiple={null}><option value="one">One</option></select>',
    '<picker multiple="false"><option value="one">One</option></picker>',
  ]) {
    expect(parseTemplate(source, "App.jsx").root[0].kind).toBe("picker");
  }
});

test("picker option groups and selected defaults cannot disappear through WASM", () => {
  for (const [source, diagnostic] of [
    ['<select><optgroup label="Group"><option value="one">One</option></optgroup></select>', "unsupported View IR picker child"],
    ['<select><option value="one" selected>One</option></select>', "selected defaults"],
    ['<select><option value="one" selected="false">One</option></select>', "selected defaults"],
  ]) {
    expect(() => parseTemplate(source, "App.jsx")).toThrow(diagnostic);
    expect(() => parseLegacyTemplate(source, "App.jsx")).toThrow(diagnostic);
  }
  for (const attribute of ["", "selected={false}", "selected={null}"]) {
    expect(parseTemplate(`<select><option value="one" ${attribute}>One</option></select>`, "App.jsx").root[0])
      .toMatchObject({ kind: "picker", options: [{ value: "one", label: "One" }] });
  }
});

test("standard option text values preserve HTML semantics", () => {
  for (const [source, option] of [
    ['<select><option>Hello World!</option></select>', { value: "Hello World!", label: "Hello World!" }],
    ['<select><option label="Shown">Actual Text</option></select>', { value: "Actual Text", label: "Shown" }],
    ['<select><option>one<span>two</span>three</option></select>', { value: "onetwothree", label: "onetwothree" }],
    ['<select><option>one<script>ignored()</script>two</option></select>', { value: "onetwo", label: "onetwo" }],
    ['<select><option>one<template>ignored</template>two</option></select>', { value: "onetwo", label: "onetwo" }],
    ['<select><option value="">Actual Text</option></select>', { value: "", label: "Actual Text" }],
    ['<picker><option>Hello World!</option></picker>', { value: "hello_world", label: "Hello World!" }],
  ] as const) {
    for (const ir of [parseTemplate(source, "App.jsx"), JSON.parse(parseLegacyTemplate(source, "App.jsx"))]) {
      expect(ir.root[0]).toMatchObject({ kind: "picker", options: [option] });
    }
  }
});

test("explicit null option values keep Crepus scalar attribute semantics", () => {
  for (const expression of ["null", "missing"]) {
    const source = `<select><option value={${expression}}>Actual Text</option></select>`;
    for (const ir of [parseTemplate(source, "App.jsx"), JSON.parse(parseLegacyTemplate(source, "App.jsx"))]) {
      expect(ir.root[0]).toMatchObject({ kind: "picker", options: [{ value: "", label: "Actual Text" }] });
    }
  }
});

test("dynamic URL provenance and resolved snapshots cross the WASM bridge", () => {
  const ir = parseTemplate(
    'img src={photo}\na href={destination}\n  span "Open"',
    "Urls.crepus",
    { photo: '/image.png?name=雪&quote="', destination: "/help?topic=雪&next=1" },
  );
  expect(ir.root).toMatchObject([
    { kind: "image", src: '/image.png?name=雪&quote="', srcBind: "photo" },
    { kind: "link", href: "/help?topic=雪&next=1", hrefBind: "destination" },
  ]);
});

test("npm parser entry points reject unpaired UTF-16 source and filename units", () => {
  for (const malformed of ["\uD800", "\uDFFF", "x\uD800y", "\uD800\uD800", "\uDC00\uD800", "😀\uDC00"]) {
    expect(() => parseCrepus(`span "${malformed}"`)).toThrow(TypeError);
    expect(() => parseTemplate(`<p>${malformed}</p>`, "App.tsx")).toThrow(TypeError);
    expect(() => parseTemplate("<p>ok</p>", `${malformed}.tsx`)).toThrow(TypeError);
  }
});

test("valid astral pairs and literal replacement characters remain unchanged", () => {
  const text = "雪😀\uD800\uDC00\uDBFF\uDFFF�";
  for (const ir of [parseCrepus(`span "${text}"`), parseTemplate(`<p>${text}</p>`, "😀�.tsx")]) {
    expect(ir.root[0]).toMatchObject({ kind: "text", content: text });
  }
});


test("shared Svelte unsupported bindings fail through both WASM payloads", () => {
  for (const parse of [parseTemplate, (source: string, filename: string) => JSON.parse(parseLegacyTemplate(source, filename))]) {
    expect(() => parse('<div bind:clientWidth={width}/>', "App.svelte"))
      .toThrow("unsupported Svelte binding");
  }
});

test("Astro quoted content directives stay literal through WASM", () => {
  const source = '<p set:text="label"/>';
  const context = { label: "context-value" };
  expect(parseTemplate(source, "App.astro", context).root[0])
    .toMatchObject({ content: "label" });
  expect(JSON.parse(parseLegacyTemplate(source, "App.astro", JSON.stringify(context))).root[0])
    .toMatchObject({ content: "label" });
  expect(() => parseTemplate('<p set:html="label"/>', "App.astro", context))
    .toThrow("View IR does not support raw HTML");
});
