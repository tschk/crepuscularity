# Input Frontends

**Also:** [Documentation home](README.md) · [DSL reference](dsl.md) · [View IR contract](view-ir-contract.md) · [Polyglot plugins](polyglot.md)

The core parser has **six frontends**. They accept six different syntaxes and
produce **one shared AST** (`crepuscularity_core::ast::Node`, `crates/crepuscularity-core/src/ast.rs`).
Lowering that AST is a separate step: `render_nodes_to_ir`
(`crates/crepuscularity-native/src/render.rs`) turns it into the single
[View IR](view-ir-contract.md), which is what SwiftUI, Jetpack Compose, the
Moonshine TSX emit, the C ABI, Tauri, and the polyglot plugins consume.
The GPUI, TUI, embedded, LVGL, and web renderers walk the shared AST directly instead.

None of this depends on the frameworks whose syntax it accepts. There is no
`svelte`, `vue`, `astro`, `@angular`, or JSX toolchain in any `Cargo.toml` — all
six frontends are hand-written Rust.

## Selection

`parse_template_with_path(source, path)` picks the frontend from the file
extension or filename suffix (`crates/crepuscularity-core/src/parser/mod.rs`), in this order:

| Extension                     | Frontend | Module               |
| ----------------------------- | -------- | -------------------- |
| `.vue`                        | Vue SFC  | `parser/vue/`        |
| `.svelte`                     | Svelte   | `parser/svelte/`     |
| `.astro`                      | Astro    | `parser/astro/`      |
| `.component.html`, `.ng.html`, `.ng` | Angular | `parser/angular/` |
| `.csx`, `.jsx`, `.tsx`        | JSX      | `parser/jsx/`        |
| other paths (`.crepus`, …) | JSX if content matches; otherwise indent | `parser/jsx/`, `parser/indent/` |

With no frontend-specific path, the JSX frontend activates when the first non-blank,
non-`#`, non-`$:` line starts with `<`; otherwise the indent frontend runs. This
also applies to `.crepus` and plain `.html` paths. The
`.svelte`, `.vue`, `.astro`, and Angular frontends are **path-driven only** —
there is no content heuristic for them.

From JavaScript, `parseTemplate(source, filename)` in
`@tschk/crepuscularity-wasm` is the one entry point for all six; the filename
is what selects the frontend.

```ts
import { parseTemplate } from "@tschk/crepuscularity-wasm";

parseTemplate(indentSource, "App.crepus");
parseTemplate(jsxSource, "App.tsx");
parseTemplate(svelteSource, "Counter.svelte");
parseTemplate(vueSource, "Counter.vue");
parseTemplate(astroSource, "Card.astro");
parseTemplate(angularSource, "hero.component.html");
```

All six return the same `ViewIr` shape.

## What the Svelte, Vue, Astro, and Angular frontends actually compile

**The template only.** Svelte and Vue script/style blocks and Astro frontmatter
are not executed by this shared template path. Angular component classes are
never read. Everything that gives these frameworks their
component semantics lives there, so none of it runs:

- **Svelte:** runes (`$state`, `$derived`, `$effect`, `$props`), `$:` reactive
  statements, stores and `$store` access, imports, lifecycle (`onMount`,
  `onDestroy`, `tick`), context API.
- **Vue:** the Composition API (`ref`, `reactive`, `computed`, `watch`,
  `defineProps`, `defineEmits`), the Options API, lifecycle hooks, provide /
  inject.
- **Astro:** frontmatter imports, `Astro.props`, top-level `await`, and
  component resolution. The `---` fence is blanked before parsing, so markup is
  read with the JSX scanning machinery and expressions are evaluated by the
  shared crepuscularity evaluator.
- **Angular:** `@Input`/`@Output`, pipes, dependency injection, and template
  reference variables. `*ngIf`, `*ngFor`, `@if`, and `@for` lower to shared
  control-flow nodes. `@switch` and `@defer` are rejected. `{{ … }}` reuses the
  Vue text scanner.

These rules describe `parse_template_with_path` and the WASM `parseTemplate`
entry point. Separate authored-module parser APIs do not make scripts execute
through these template-only entry points.

Expressions in the markup are evaluated by the crepuscularity template
evaluator against the crepuscularity template context, not by a JavaScript
engine. `<style scoped>` is recorded but scoping is not implemented.

Unsupported markup constructs are **hard parse errors**, not silent drops. A
template that uses them fails to compile rather than rendering something
subtly wrong.

### Svelte

Supported: HTML elements and nesting, comments, `{expr}` interpolation,
`{#if}` / `{:else if}` / `{:else}`, `{#each list as item}`, `{@html expr}`,
static `class`, `class:foo={cond}`, `on:click={h}` with modifiers, Svelte 5
`onclick={h}` for the known DOM event names, `bind:value|checked|group|files`,
and `{value}` attribute shorthand.

The Crepuscularity `<include src="parts/card.crepus" title={title} />`
extension also works in Svelte input. Non-self-closing includes pass their
child markup as the caller's slot. This does not resolve Svelte imports or
execute source scripts.

Event modifiers `preventDefault`, `stopPropagation`, and
`stopImmediatePropagation` lower to shared `prevent`, `stop`, and `immediate`
semantics. `trusted`, `self`, `once`, and `capture` retain their names.

Rejected with an error:

- Other `bind:` directives, including `bind:this`, media, and measurement bindings.
- Event forwarding without a handler, `passive`, `nonpassive`, unknown modifiers,
  and shared aliases such as `prevent` in place of Svelte's `preventDefault`.
- Every block except `{#if}` and `{#each}` — `{#await}`, `{#key}`, `{#snippet}`.
- Every `{@…}` tag except `{@html}` — `{@render}`, `{@const}`, `{@debug}`.
- `{#each}` without `as`, with an index (`as item, i`), with destructuring
  (`as {a, b}`), or with an `{:else}` branch. The shared `ForBlock` binds one
  item variable and has no empty-list branch.
- The directives `use:`, `transition:`, `in:`, `out:`, `animate:`, `let:`,
  `style:` — so no actions, transitions, animations, slot props, or inline
  style directives.
- Spread attributes `{...props}`.
- `<slot>` — use crepuscularity `include` slots instead.
- Any capitalised tag. This frontend performs no Svelte module resolution;
  file composition uses the explicit Crepuscularity include extension.
- `<svelte:window>` and the other `<svelte:*>` special elements.

Silently dropped: the key expression in `{#each … as item (key)}`.

### Vue

Supported: SFC splitting, `{{ expr }}` mustaches, `v-if` / `v-else-if` /
`v-else`, `v-for` (`item in items`, `(item) in items`, `of` form), `v-show`
(lowered to a conditional `hidden` class), `v-html`, `v-text`, plain `v-model`,
`v-bind:x` / `:x` including `:class` with string, object, and array syntax,
`v-on:evt` / `@evt` with the modifiers `prevent`, `stop`, `self`, `once`,
`capture`, `<template>` as a structural wrapper, void elements, comments.

Rejected with an error:

- Object spread `v-bind="obj"` — bind properties individually.
- `v-for` index/key bindings and destructuring. The shared loop AST binds one item variable.
- Empty property bindings, content directives, branch conditions, or event handlers.
- Duplicate IDs, branch directives, `v-for`, or competing `v-text`/`v-html` bindings.
- JSX-style `class={expr}`. Vue class expressions use `:class="expr"`.
- `:style` — use utility classes.
- `v-bind` modifiers (`:x.prop`), dynamic binding arguments (`:[key]`),
  dynamic event names (`@[evt]`).
- `v-model` modifiers (`v-model.number`, `.trim`, `.lazy`).
- Any event modifier outside the five above — including **all key modifiers**,
  so `@keyup.enter` is an error.
- Every other directive: `v-once`, `v-memo`, `v-pre`, `v-cloak`, `v-slot`, the
  `#name` slot shorthand, and any custom directive.
- More than one top-level `<template>`, unclosed `<template>`, missing
  `</script>`, trailing markup after the template.

Unlike Svelte, a capitalised tag
is **not** an error — it becomes an element with that tag name, and since no
module resolution happens, custom components render as unknown elements.

### Astro

Leading `---` frontmatter requires both fences and is not executed. The public
`split_astro_source` API returns borrowed frontmatter text, its original UTF-8
byte range, the template slice, and its start offset. Splitting does not validate
the template or establish authored-module support.

Supported: HTML elements, comments, `{expr}` interpolation, `{value}` attribute
shorthand, `class:list`, `set:html`, `set:text`, conditional expressions (`&&`
and ternaries), and `.map(item => markup)` lowered to the shared loop AST.
Quoted `set:html` and `set:text` values are literal strings with HTML attribute
entities decoded. Braced values remain expressions.

Rejected: component tags, slots, script/style raw-text elements, spread attributes,
`transition:*`, `define:vars`, and map callbacks with an index, destructuring, or
a block body. `client:*` directives become bindings but do not implement Astro hydration.

### Angular

Supported: mustaches, property and event bindings, `[(ngModel)]`, `*ngIf`,
`*ngFor`, `@if` / `@else if` / `@else`, `@for`, class bindings, and
`<ng-container>` as a structural wrapper. Tracking expressions are ignored
because the shared AST has no DOM-diffing keys.

Rejected: `<ng-template>`, `<ng-content>`, template references, `*ngSwitch`,
`*ngIf` with an `else` reference, style bindings, event key modifiers, global
event targets, two-way bindings other than `ngModel`, loop index bindings,
`@if` aliases, `@switch`, `@defer`, and `@empty`.

## Limits

The tag-based frontends (JSX, Svelte, Vue, Astro, Angular) cap nesting depth at
256 and error beyond it. Internal `RawParseError` diagnostics carry a message
and an optional byte offset.
