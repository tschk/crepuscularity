# DSL Reference

**Also:** [Documentation home](README.md) · [Components](components.md) · [CLI](cli.md) · [Extensions](webext.md)

This page documents the two `.crepus` syntaxes — indentation and JSX/HTML tags — which compile to the same AST and work with every backend (GPUI, web, webext). Auto-detected by whether the first content line starts with `<`, or by a `.csx` / `.jsx` / `.tsx` extension.

The parser also accepts JSX, Svelte, Vue, Astro, and Angular templates through first-party Rust frontends into that same AST. See [Input frontends](frontends.md) for the filename selection rules, supported markup, and framework features that do not run.

## Indentation Syntax (Native)

Elements are `tag classes…` with indented children.

```
div w-full h-full bg-zinc-950 text-white flex flex-col gap-4
  div text-2xl font-bold
    "Hello {name}"
  if {score > 50}
    div text-green-400
      "High score!"
  else
    div text-red-400
      "Low score"
  for item in {list}
    div p-2 border rounded
      {item}
```

## JSX / HTML Tag Syntax

For developers familiar with React/TSX. Same semantics, angle-bracket style.

```jsx
<div class="w-full h-full bg-zinc-950 text-white flex flex-col gap-4">
  <div class="text-2xl font-bold">Hello {name}</div>
  <if condition={score > 50}>
    <div class="text-green-400">High score!</div>
    <else><div class="text-red-400">Low score</div></else>
  </if>
  <for let="item" in={list}>
    <div class="p-2 border rounded">{item}</div>
  </for>
</div>
```

`class={theme}` and `className={theme}` preserve dynamic class expressions;
empty braced expressions are parse errors. `id={name}` becomes a runtime
binding, while quoted IDs remain literal. An element may have only one ID,
including the `bind:id` alias.

Dynamic class strings are evaluated by web, TUI, native View IR, embedded,
LVGL, and GPUI rendering. GPUI `view!` runtime lists use the styler re-exported
by `crepuscularity-gpui`, so the calling crate must depend on that package
(as generated GPUI projects do). Static classes and `class:name={condition}`
continue to compile directly into GPUI builder calls. Runtime lists use the
runtime styler's supported utilities. `gpui:` and `gui:` apply to GPUI; other
target prefixes are excluded. The GPUI `view!` macro merges hover/focus/active
utilities from static classes and runtime lists, evaluating each list once.
The GPUI runtime provides `crepus_target="gpui"`, `is_gui=true`, and false
`is_tui`/`is_web` defaults, plus `crepus_platform` and OS flags. Explicit caller
values take precedence.

The hot-reload renderer also merges state utilities; active styles require an
explicit or bound element ID. Focus styles require focus tracking, which the
hot-reload renderer does not yet provide.

HTML output omits false/null boolean bindings such as `disabled={false}` and
`checked={false}`. String literals retain HTML presence semantics:
`disabled="false"` still emits the attribute. ARIA attributes retain their
string values, including `aria-disabled="false"`. These rules also apply to SSR
and hydration output.

`download={false}` and `download={null}` omit the download attribute.
`download={true}` enables downloading with the browser's default filename;
string values such as `download="false"` remain suggested filenames. Explicit
Angular `[attr.download]` bindings retain literal attribute presence semantics.

HTML void elements such as `img`, `input` and `br` reject nonempty child trees
in plain, SSR and hydration rendering, rather than silently discarding content.
Tag matching is case-insensitive; empty paired or self-closing void elements
remain supported. Inactive outer branches are not evaluated.

Control-flow tags: `<if condition={...}>`, `<else>`, `<else-if condition={...}>`, `<for let="var" in={list}>`, `<match on={expr}><case pattern="...">`.

`<if>`, `<else-if>`, `<for>`, and `<match>` require matching closing tags;
self-closing forms are parse errors. Empty `<else />` and `<case />` branches are supported.
`<for>` requires one nonempty literal loop variable (`let` or `var`) and one nonempty iterator expression (`in`). Missing or duplicate attributes and empty braced iterators are parse errors.
`<match>` requires exactly one subject attribute (`on` or `value`); missing, empty braced, and duplicate subjects are parse errors.

A `<case>` without a pattern is a wildcard. A supplied `pattern`, `match`, or `when` must be a nonempty literal or braced expression; duplicate aliases and bare boolean attributes are parse errors.

Else-if and else branches are siblings inside the enclosing `<if>`, with
matching closing tags:

```jsx
<if condition={score > 80}>
  <p>high</p>
  <else-if condition={score > 50}><p>medium</p></else-if>
  <else><p>low</p></else>
</if>
```

Else-if chains are limited to 256 conditional branches.

## Pragmas

File-level directives go at the very top, before any elements.

```
google-font "Inter"
google-fonts "Inter" "JetBrains Mono"
```

`google-font` loads a single family; `google-fonts` loads multiple in one pragma. Both inject the appropriate `<link>` tag on the web path.

## Element Syntax

`tag classes… #id "inline-text"` on one line. `#id` emits `id="…"` on the element; an inline quoted literal becomes the first child text node.

```
section py-16 #hero "Hello"
  span text-sm
    "World"
```

## Native SwiftUI Component Tags

When `.crepus` is compiled through Aurorality `swiftgen`, templates can declare native SwiftUI-style components directly. This semantic layer is additive: utility-first layout (`div flex`, `div flex-col`, spacing classes, etc.) still works and can be mixed in the same tree.

### Supported semantic tags (swiftgen path)

| Category | Tags |
| --- | --- |
| Navigation | `navigationstack`, `navigationsplitview`, `sidebar`, `detail`, `content` |
| Layout containers | `vstack`, `hstack`, `zstack`, `group`, `form`, `section`, `list`, `item`, `scrollview`, `scroll`, `spacer`, `divider` |
| Controls | `button`, `toggle`, `picker`, `textfield`, `input`, `securefield`, `menu` |
| Content | `label`, `image`, `sf`, `sf-*`, `span`, `p` |

### Example: hybrid semantic + utility layout

```text
navigationsplitview
  sidebar
    vstack gap-2 p-3 w-72
      hstack
        label title="Inbox" symbol="tray"
        spacer
        button button-bordered @click="newConversation"
          sf-plus
      sidebar flex-grow
        for row in {rows}
          item @click="select:{row.id}"
            span "{row.title}"
  detail
    vstack gap-2 p-3
      textfield bind=draft placeholder="Message"
      hstack
        spacer
        button button-prominent @click="send"
          "Send"
```

### Notes

- `div flex` and `div flex-col` remain first-class and continue to map to `HStack` / `VStack`.
- Semantic tags are primarily for readability and explicit intent when building native shells.
- Backends may support different subsets; the table above applies to the Aurorality SwiftUI `swiftgen` path.

## Text Nodes

Quoted strings are text nodes. `{expr}` inside quotes interpolates. A bare `{expr}` on its own line renders the value directly, including expressions with string literals. `{=expr}` emits raw HTML on the web target. Both forms require a nonempty expression.

```
div
  "Hello {name}, score: {score * 10}"
  {username}
```

## Control Flow

Indentation control-flow headers require their arguments. Braced expressions must
be nonempty, have a closing brace, and have no trailing text. Quotes inside an
expression can contain braces. Orphan `else` branches and match arms are parse
errors; match arm patterns must be nonempty, while `""` remains a valid pattern.

Runtime renderers compare quoted match patterns with their decoded text value:
`"ready"` matches `ready`, not a value containing the surrounding quote characters.
Unquoted patterns compare literally, and `_` matches any value. The `view!` macro
emits Rust match patterns instead; bare identifiers follow Rust binding rules there.

Expressions support signed 64-bit integers, floating-point numbers, strings,
variables, property access, arithmetic, comparisons, and boolean operators.
Floating-point literals accept leading or trailing decimal points (`.5`, `2.`)
and `e` or `E` exponents, such as `1e3`, `.5e2`, and `2.5E-2`.
Malformed exponents and overflowing float literals are evaluation errors.
Integer addition, subtraction, multiplication, and remainder preserve exact
values; division produces a floating-point result. Integer overflow and
non-finite arithmetic results return evaluation errors. Division or remainder
by zero returns `null`.
There is no decrement operator: `count--2` means `count - (-2)`, and `--count`
means double negation. Generated web expressions preserve this without mutating
the model.
Boolean `&&` and `||` short-circuit unused branches; those branches are still
checked for valid expression syntax.

List and object equality (`==`, `!=`) compares stored fields and nested values.
List order matters; object field order and rendering-resource metadata do not.
Collections are unequal to scalar values, including their display summaries.
Collection ordering (`<`, `<=`, `>`, `>=`) is an evaluation error. Scalar
comparison behavior also applies to scalar fields within collections.

Numeric comparisons preserve integer precision, including mixed integer/float
comparisons. Float equality is exact. The numeric properties `.abs`, `.floor`,
`.ceil`, and `.round` reject overflow or invalid conversions rather than clamp
the result. Quoted braces inside interpolation expressions are ordinary string
content. Unclosed placeholders, incomplete expressions, out-of-range literals,
and unsupported expression characters return errors. The expression evaluator
returns `null` for missing variables or empty input; template directives reject
empty expressions as described above.

```
if {score > 100}
  div text-green-400
    "High score!"
else if {score > 50}
  div text-yellow-400
    "Medium"
else
  div text-red-400
    "Low"

for item in {items}
  div p-2 border-b
    {item.name}

match {status}
  "active" =>
    div text-green-400
      "Active"
  _ =>
    div text-gray-400
      "Unknown"
```

## Variables

JSX declarations use `<let name="count" value={1} />` or
`<let-default name="count" value={1} />`. Each requires exactly one nonempty
literal `name` and one nonempty `value` expression. Declaration tags must be
self-closing and cannot contain children. Empty string values, zero,
and false are valid. Defaults preserve values already provided in the context.

`$: let` computes a local variable from an expression; `$: default` sets a variable only when it is not already in context (canonical way to declare optional component props). Both require a variable name, `=`, and a nonempty value expression. In HTML rendering, an element's child declarations apply in order to later
siblings and their descendants. Nested elements keep their own local overrides;
leaving the element restores the enclosing scope. Includes receive explicit props,
while supplied slot content retains its caller's lexical scope. Invalid evaluated
declarations return an error; defaults skip evaluation when the name already exists.

```
$: let total = {price * quantity}
$: default variant = "primary"
```

## Attributes

Static values use `key=value`, dynamic values use `key={expr}`. Event handlers use `@event="fn_name"` (on the web/WASM path this emits `data-onclick="fn_name"` and dispatches to an exported Rust function). Conditional classes use `class:name={expr}`.

Indentation `id={name}` is a dynamic binding; `id="{name}"` keeps the literal
braces. An element can have only one ID across `#id`, `id=`, and `bind:id=`.
`class={expr}` preserves the whole expression, including internal whitespace.
Quoted indentation class lists also preserve interpolation spans:
`class="w-{1 + 2} flex"` resolves to `w-3 flex`. Spaces inside an expression stay
inside its class token; Unicode whitespace outside expressions separates tokens.
The same splitting applies to `when:{enabled}="w-{1 + 2} flex"`. Conditional class
values are evaluated only when selected. Nested/quoted braces use the normal
interpolation rules; malformed spans retain their original evaluator diagnostics.
Bindings and conditional classes require nonempty names and expressions.
Events require a name, handler, and nonempty entries after any `|` modifiers.
Web handlers preserve `prevent`, `stop`, `immediate`, `trusted`, `self`, `once`, and `capture` modifiers.
SSR content remains interactive while WASM initializes: action calls wait for
the runtime, while modifiers apply immediately to the browser event.
Scoped and root actions receive JSON with the handler element's data attributes.
Keyboard events include `_key`, `_meta`, `_ctrl`, `_shift`, and `_alt`. Input
and change events include the control's live `_value`, plus `_checked` for
checkboxes/radios and `_values` for multiple selections. Values are captured
when the event arrives, before any wait for WASM initialization.
Submit events include `_form`, an ordered array of `[name, value]` pairs from
native `FormData`, including repeated names and the submitting button. Disabled
and unchecked controls follow browser submission rules. File values carry
`name`, `type`, `size`, and `lastModified` metadata, without binary contents.
The form snapshot is captured before any wait for WASM initialization.
A textarea `value` binding renders escaped child text, taking precedence over
its template children. Empty values and leading newlines are preserved.
Textarea child interpolation also preserves leading newlines and avoids SSR
wrappers appearing as literal control text.
Root and scoped rerenders preserve focus and input/textarea selection. Give
controls stable IDs when their layout can change; idless controls use their
position in the DOM. Rich-text (`contenteditable`) selection is not restored.
Focus events caused by restoration do not invoke framework actions again.
Scoped actions refresh their rendered state after every handled event, except
rich-text input, keyboard, and composition events, which preserve browser-owned
editing. Those rich-text handlers must update their editable DOM explicitly.
Input and keyboard events marked `isComposing` still call actions but defer
DOM replacement, preserving IME composition until a noncomposing event arrives.
A scoped or Rust island render returning an empty string clears its children;
`null` or `undefined` leaves the existing children unchanged. Newly rendered
rotating widgets and nested islands initialize within the updated scope.
Rust island scope rerenders retain authored island props. A newer scope render
supersedes any pending initial island render on that element: stale initial work
cannot call its queued export or overwrite newer content. Other islands remain
independent. Explicit scope exports still return HTML synchronously.
Modifier order matters for `prevent` and `self`; `once` follows the native
listener lifetime even when a guard rejects the event. Key-down and key-up
handlers accept alternative key chords such as `@keydown|ctrl+s|Esc=save`.
Unsupported web modifiers fail rendering. Capture handlers run from parent to
child, and bubble handlers run from child to parent until `stop` is applied.
Multiple handlers for the same event keep their own modifiers and `once`
lifetimes. `stop` blocks ancestors while retaining sibling listeners;
`immediate` calls `stopImmediatePropagation` and blocks later sibling listeners.
`trusted` calls actions only for events whose `isTrusted` flag is true.
Empty literal attributes such as `title=""` remain valid.

```
input type="text" value={input_value} placeholder="Enter text"
button @click="handle_submit"
  "Submit"
div class:hidden={!visible} class:active={selected}
  "Content"
```

## Slot-Rotate

`slot-rotate` is a built-in widget for cycling through text children at a fixed interval. Give it an `interval={ms}` attribute and optionally a class-alias name to style the active item. Each indented string child is one rotation slot. Web pages can contain multiple independently rotating widgets. In the HTML/WASM runtime, rerendering replaces their timers; removing one stops its timer without stopping its siblings. Generated framework components retain timers across ordinary parent updates and clean up when their nodes are removed.

```
slot-rotate interval={3200} slot-lede
  "a GPUI-first template pipeline"
  "a .crepus DSL with hot reload"
  "one syntax for GPUI, web, and extensions"
```

The HTML/WASM runtime keeps changing text with reduced motion enabled, but disables fading and blur. Generated framework components instead retain the first phrase while reduced motion is enabled, and resume rotation when the preference changes. See [framework output](frontends.md) for host lifecycle details.

Web rotation intervals must be whole milliseconds from 1 through 2147483647; invalid values use the 3200 ms default.

## Include

`include` embeds another component. Props are passed as `key=value` or `key={expr}`. Indented children under the include call become the `slot` content inside the component.

```
include components/card.crepus title={t} subtitle={sub}
  div p-4
    "Slot content rendered inside the component"
```

Indentation-based `include` and `embed` directives require a source path and
whitespace between props; spaces and tabs are accepted. Quote paths containing
spaces, for example `include "components/card view.crepus" title="Card"`.
The surrounding quotes are removed from the resolved path. Prop names use letters,
digits, hyphens, or underscores and must be unique. Quoted values preserve literal
backslashes, and braced expressions ignore braces inside quoted strings.
Missing values, unclosed quotes or braces, empty braced expressions, and trailing
tokens are parse errors. Use `title=""` to pass an empty string.

## Web embed

`embed` mounts a browser module as a client island in `crepus web build` and `crepus web serve`. Props are evaluated from the current context and passed to the island module's `mount(el, props, ctx)` export.

```
embed ./islands/wave.ts adapter="module" title={title} intensity={level}
```

JSX/tag syntax uses `island`:

```jsx
<island src="./islands/wave.ts" adapter="module" title={title} intensity={level} />
```

JSX `include`, `island`, and `crepus-island` tags require exactly one nonempty
literal `src` or `path` attribute. Duplicate attributes are parse errors.
Islands must be self-closing; their optional `adapter` must be a nonempty literal
string. Paired `include` tags accept child content as a slot.

Use `adapter="react"` to mount an authored React default component, including
TSX imports and hooks, through ReactDOM:

```crepus
embed ./islands/Counter.tsx adapter="react" start={2}
```

The site must provide `react` and `react-dom` dependencies. Bun bundles the module
for the browser without executing it during compilation. Props become React
component props; shell replacement unmounts the React root and runs effect cleanup.
An authored named `mount` export still takes precedence and owns its cleanup.
A source path cannot use conflicting adapters in the same site. In `crepus web dev`,
saving authored island code or dependencies rebuilds the islands and reloads the
page. Build errors appear in the dev overlay; a successful save recovers. The
watcher covers authored site files broadly and excludes generated artifacts,
package installations, and the Rust runtime directory. This mounts a
React component; it does not translate its script to other framework targets.

A module's `mount` may return a cleanup function (or a promise resolving to one).
The runtime calls it once before a framework rerender removes that island.
Replacing a failed parent island's contents with diagnostics also cleans up its
mounted descendants first. Cleanup failures are reported without blocking the
replacement. If an async mount finishes after removal, its cleanup runs immediately. Rendering
does not wait for asynchronous cleanup. Direct DOM
removal outside framework renders remains the module's responsibility.

## Animations

`animate:property={duration timing-function}` attaches a CSS transition or entry animation to the element. Settings can also be quoted and can end with `repeat`. Missing property names, extra settings, and an unsupported third setting are parse errors.

The GPUI runtime gives ID-less animations distinct identities based on tree
position, including loop iterations and included content. Use explicit IDs for
reorderable nodes, since positional identities change when siblings move.
Unchanged animation settings retain progress across redraws. Changing the ordered
animation settings restarts that element's animation state, including after hot reload;
the element's interaction ID is preserved.

```
div animate:opacity={300ms ease-in-out}
  "Fades in on mount"
```

For web builds, you can also add trailing raw CSS (including `@keyframes`, class rules, and media queries) directly in the `.crepus` file without wrapping it in `<style>`:

```css
@keyframes sunset {
  0% { opacity: 0.6; }
  100% { opacity: 1; }
}

.animate-sunset {
  animation: sunset 10s ease-in-out infinite alternate;
}
```

That CSS is collected and injected into the generated document head automatically by `crepus web build` and `crepus web serve`.

Note: class-alias lines (`.alias ...`) still belong at the very bottom of the file. Put trailing CSS above alias lines (or at the end if you have no aliases).

## Class Aliases

Lines at the bottom of a `.crepus` file starting with `.name` define reusable class groups. Any element with that name in its class list expands to the aliased classes at render time.

```
.slot-lede text-zinc-100 font-medium
.footer-row flex flex-col sm:flex-row justify-between gap-4 text-sm text-zinc-500
```

Usage: `div footer-row` expands to `div flex flex-col sm:flex-row justify-between gap-4 text-sm text-zinc-500`.

## Comments

Lines starting with `#` are comments:

```text
# This is a comment
div
  # Another comment
  "Hello"
```

## Expression Operators

| Operator | Example |
|----------|---------|
| Arithmetic | `{a + b}`, `{x * y}`, `{n / 2}`, `{n % 2}` |
| Comparison | `{a > b}`, `{x == y}`, `{n != 0}`, `{a >= b}`, `{a <= b}` |
| Logical | `{a && b}`, `{x \|\| y}`, `{!flag}` |
| Property access | `{user.name}`, `{item.price}` |
| Parentheses | `{(a + b) * c}` |

GPUI runtime classes also interpolate embedded expressions for ordinary utilities:
`w-{columns + 1}`, `h-[{height + 2}px]`, and `hover:p-{gap}` resolve against the
current context on each render. Existing dynamic hex-color and opacity handling
is preserved. Resolved values are not recursively reinterpreted as expressions;
malformed expressions keep the runtime styler's no-op behavior.

GPUI currently ignores `fixed`, `sticky`, `select-none`, `pointer-events-none`,
and `whitespace-pre` in static and dynamic class lists. The pinned GPUI API has
no corresponding
utility methods; these classes do not change positioning, selection, hit testing,
or preformatted whitespace behavior.

GPUI ring utilities compose with ordinary shadows. `ring-inset` works before or
after the ring width; `ring-0` clears the ring width and `shadow-none` clears
ordinary shadows while preserving the ring.
Runtime state expressions such as `hover:{variant}` preserve this composition
with base styles. Resolved utilities apply in order, including explicit resets;
embedded expressions have the same behavior without recursive interpolation.

`$: default` requires a runtime template context. Compile-time GPUI `view!`
rejects it because Rust cannot test whether a caller binding exists; resolve
optional values in Rust before invoking the macro. Ordinary `$: let` declarations
are supported by `view!` and follow sibling order.
