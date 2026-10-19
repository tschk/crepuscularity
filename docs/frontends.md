# Input Frontends

**Also:** [Documentation home](README.md) · [DSL reference](dsl.md) · [View IR contract](view-ir-contract.md) · [Polyglot plugins](polyglot.md)

The core parser has **six frontends**. They accept six different syntaxes and
produce **one shared AST** (`crepuscularity_core::ast::Node`, `crates/crepuscularity-core/src/ast.rs`).
Lowering that AST is a separate step: `render_nodes_to_ir`
(`crates/crepuscularity-native/src/render.rs`) turns it into the single
[View IR](view-ir-contract.md), which is what SwiftUI, Jetpack Compose, the
Moonshine TSX emit, the C ABI, Tauri, and the polyglot plugins consume.
The GPUI, TUI, embedded, LVGL, and web renderers walk the shared AST directly instead.

HTML site bundles collect supported template frontend files so includes can resolve
without native filesystem fallback. This is shared-AST template support; frontend
scripts remain extraction-only. Web and native file-map rendering derive the
include base from each entry’s directory, using the map root for root-level
entries. The caller context’s base directory does not override that selection.

None of this depends on the frameworks whose syntax it accepts. There is no
`svelte`, `vue`, `astro`, `@angular`, or JSX toolchain in any `Cargo.toml` — all
six frontends are hand-written Rust.

Markup frontends reject stray `/` or `=` where an attribute name is required;
those characters remain valid inside attribute values. An `=` must be followed by
a value; use `title=""` or `title=''` for an explicitly empty string. Attribute
bindings require a nonempty expression: `title={}` is invalid, while `title={''}`
and explicit `null` or `false` expressions remain valid. Spread attributes
(`{...props}`) are rejected by all markup frontends; bind properties individually.
JSX constant ID string expressions use the same escape rules as other expression
attributes for both quote styles. Ordinary quoted ID attributes remain literal text.

Horizontal spaces at nested text boundaries and between tags are retained,
including around interpolations and comments. Newline-containing indentation at
text-region edges and whitespace-only formatting lines is removed; root padding
is ignored. Vue's conditional-branch separators and Angular's curly-block edge
padding remain syntax rather than rendered text. This shared whitespace policy
does not execute source framework whitespace settings. Indentation-template
quoted text keeps its existing behavior.

Indentation quoted `class` and `when` lists split whitespace outside interpolation
spans. For example, `class="w-{1 + 2} flex"` preserves `w-{1 + 2}` as one AST token
and `flex` as another. Static class tokens and Unicode separators retain their
existing behavior; this does not execute source-framework scripts.

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

Ordinary includes select their frontend from the authored include filename,
including accepted symlink aliases. `crepus_refs!` and `template_refs!` use the
same frontend identity when collecting IDs. Canonical targets still control filesystem
admission, reads and nested include directories. Parsed include caches distinguish
the physical source and frontend identity; named `#Component` files retain their
component-file parsing contract.
Generated TUI templates retain the absolute authored entry path for parsing and
reloads while resolving relative includes from the canonical entry directory.

Framework source emission accepts these named input frontends through `--entry`:

```sh
crepus web build --site ./site --entry src/App.svelte --emit react
crepus web build --site ./site --entry src/App.vue --emit moonshine-dom
```

The source-emission collector includes the suffixes above, keeps relative paths
for includes, and skips build output and dependency directories. Plain `.html`
files use JSX/HTML parsing when their content starts with markup; use an Angular
suffix for Angular input. Literal fullwidth braces remain text. Shared Vue and
Angular templates decode HTML entities once in literal text and ordinary static
attributes, after recognizing authored interpolation. Encoded braces stay literal;
runtime values, raw carriage returns, and the existing literal ID/class policy
remain unchanged. The authored native Vue compiler retains its separate decoding
contract. Source scripts remain unexecuted. Imported `.svelte.ts` and `.svelte.js` companion modules support
synchronous function-local `$state`, `$state.raw`, `$derived`, and `$derived.by`
identifier declarations, including factories exported as functions or arrows.
Getters, methods, and closures retain live access to each factory's private state.
Standalone `$effect(callback)` registrations inside synchronous companion
factories use the caller's DOM owner, run after DOM updates, and clean up before
replacement or disposal. Calling such a factory without a live DOM owner fails.
SSR erases the entire registration, including callback-factory evaluation.
SSR derived values created during rendering cache their computation; values
created outside rendering recompute on each read. JavaScript companions emit
JavaScript without generated type syntax.
When a component path conflicts with a companion file or TypeScript's `.js`/`.jsx`
import substitution, the component uses a deterministic `.__crepusN.ts` suffix.
Imports and the entry facade follow that mapping; companion filenames stay
unchanged. Required output directories are reserved too, so generated component
files cannot replace module directories. Existing stale `.ts`/`.tsx` files that
would shadow a JavaScript module cause a prewrite error; use an empty output
directory. The compiler does not delete those files.
Top-level rune cells and effect registrations, companion effect variants
(`pre`, `root`, and `tracking`), snapshots, `$props`, `$bindable`, destructured rune
declarations, async/generator factories, and type queries of rewritten cells fail
explicitly. Actual runtime bindings with rune names remain ordinary JavaScript;
ambient rune signatures do not shadow the built-ins. Comments, strings, and
unbound type-only references remain unchanged. DOM derived callbacks can run for
intermediate synchronous writes where Svelte batches those writes, so callback
execution counts can differ. The HTML/WASM site's existing `.crepus` collection
is unchanged.

Vue SFC block attributes use literal values rather than JavaScript expressions.
Bare `lang` and `lang=""` select the default language. Assigned braces and boolean
words remain literal language names; native Vue scripts accept only JS or TS.
Boolean block flags such as `setup` and `scoped` use attribute presence.
Vue SFC templates accept ordinary HTML markup with the default language or an
explicit `lang="html"`. Template preprocessors such as Pug fail explicitly; their
source is never treated as HTML markup. Duplicate attributes on SFC template,
script, or style blocks are parse errors, before block metadata is selected.

Template parsing accepts a leading UTF-8 BOM before selecting the frontend.
Diagnostic locations remain relative to the original source, including the BOM
and surrounding Vue SFC blocks. Multi-component files also accept the marker
before frontmatter or the first named component section.

With no path, the JSX frontend still activates when the first non-blank,
non-`#`, non-`$:` line starts with `<`; otherwise the indent frontend runs. Leading
`#` comment lines, including comments between `$:` declarations, are ignored
before the first JSX root node. Hash-prefixed text inside elements or after
root content remains literal text. JSX `$:` lines must be `let` or `default`
declarations with a nonempty name and expression; malformed or unsupported
declarations produce parse errors. Declaration names must be variable names
supported by the evaluator (ASCII letters or `_`, then ASCII letters, digits,
or `_`, excluding literal keywords).

JSX comment-only containers (`{/* ... */}`
and line comments ending before the closing brace) are ignored at root, nested,
and inline text positions. Braces and markup inside comments remain comment
content. Quoted comment syntax stays literal; comments mixed with template
expressions are rejected. Adjacent literal and expression text parts stay grouped
so generated button labels retain the full label and react to model updates.

The Svelte, Vue, Astro, and Angular frontends are **path-driven only** — there
is no content heuristic for them. Plain `.html` files use the JSX content
heuristic; Angular requires one of the filename patterns above.

From JavaScript, `parseTemplate(source, filename)` in
`@tschk/crepuscularity-wasm` is the one entry point for all six; the filename
is what selects the frontend.

```ts
import { parseTemplate } from "@tschk/crepuscularity-wasm";

parseTemplate(indentSource, "App.crepus");
parseTemplate(jsxSource, "App.tsx");
parseTemplate(svelteSource, "Counter.svelte");
parseTemplate(vueSource, "Counter.vue");
parseTemplate(astroSource, "Page.astro");
parseTemplate(angularSource, "App.component.html");
```

All six return the same `ViewIr` shape. That target currently rejects event
modifiers and multiple handlers for the same event because its handler fields
cannot preserve those semantics. Parsing to the shared AST and rendering to
HTML support the richer web event contract described below.

JSX DOM event aliases use canonical names (`onKeyDown` → `keydown`,
`onDoubleClick` → `dblclick`). A `Capture` suffix adds the `capture` modifier;
`onGotPointerCapture` is the pointer-capture event, not a phase suffix.
These are native event bindings rather than React synthetic events.
Shared JSX templates require named attributes such as `title={value}`. Bare
`{value}` attribute shorthand is a parse error; Svelte and Astro retain their
separate shorthand support.
The same rule applies to JSX structural branches such as `<else-if>` and `<case>`.
Text-field and textarea `input` handlers now survive View IR lowering separately
from `change`: use `@input` in `.crepus`, `onInput` in JSX, and the corresponding
frontend event syntax. Generated web components dispatch live input events and
committed change events independently; neither implicitly writes the host model.

View IR text inputs, textareas and checkbox submission values accept `defaultValue`
and `default-value` as aliases of the initial `value` snapshot. Only one spelling
may be authored per element: the IR cannot represent separate live/reset defaults.
Existing `bind` precedence remains unchanged, and these aliases do not set checkbox
checked state. File inputs reject all initial-value spellings.

View IR checkbox/toggle/switch controls accept `defaultChecked` and
`default-checked` as aliases of the initial `checked` snapshot, including bare
indentation attributes. Distinct checked spellings cannot be combined. HTML
checkbox inputs retain presence semantics; custom controls retain their existing
boolean conversion. Live `bind` precedence is unchanged.

The Solid View IR emitter preserves unbound textarea defaults in both DOM and
server output, including leading newlines and literal HTML-like text. Client edits
remain uncontrolled, and form reset restores the default. Bound textarea values
retain Solid's live DOM property path and serialize once as escaped server body
content. Nullish values become empty text; other values use DOM string conversion.

Vue unbound textarea defaults preserve leading LF characters in client rendering
and parsed server HTML. Client edits remain uncontrolled and form reset restores
the original default; the leading-LF path also preserves edits made before hydration.
Existing bound-textarea handling and HTML carriage-return normalization are unchanged.

View IR `select`/`picker` selection uses `bind`. Parent `value`, `defaultValue`,
and `default-value` attributes fail explicitly instead of silently losing the
initial selection, matching the existing rejection of selected-option defaults.

View IR image `src` and link `href` expressions retain optional `srcBind` and
`hrefBind` metadata. React, Svelte, Vue, Solid, Imba and Moonshine DOM/SSR emitters
resolve them against live root/loop scope. Static URLs retain their literal output;
missing values use ordinary text coercion to blank (React may omit a blank image
`src`). Captured declaration/include/component-default URLs remain snapshots.
Other media attributes retain their existing snapshot contracts.

Generated web components preserve View IR element IDs, including nodes without
CSS classes. IDs are literal attribute data; HTML entity syntax in an ID is not
decoded a second time. Expression-based source IDs retain the IR's render-time
snapshot behavior.
Built-in HTML rendering encodes carriage returns in attribute data as `&#13;`,
preserving CR and CRLF values when a browser parses the output.
Labels retain `for` (or the JSX `htmlFor` alias), so their targets can match
those IDs. Label target expressions use the same snapshot behavior; duplicate
target attributes fail. This does not extend support to arbitrary form attributes.
Source `aria-label` is also preserved as a render-time snapshot in web output,
on the control rather than its styling wrapper. Explicit values override generated
accessible names; duplicate attributes fail.
Web form fields also retain explicit `name`, `required`, and text-compatible
input `type` values such as `email` and `url`. These are render-time snapshots.
Unsupported structural `input` types now fail instead of silently becoming text
fields; checkbox and file inputs use their dedicated control nodes, and the
custom slider control has its own lowering path. See the [View IR contract](view-ir-contract.md) for
the exact type and boolean-attribute rules.
View IR buttons are ordinary action buttons. An omitted `type` or explicit
`type="button"` is accepted; submit, reset, and other explicit types fail instead
of silently changing form behavior.
Standard checkbox inputs use HTML presence semantics for initial `checked`:
quoted `"false"` is enabled, while typed false/null and an absent attribute are
unchecked. Custom checkbox/toggle/switch boolean coercion is unchanged.
Unbound generated checkboxes retain this initial state as their native form-reset
default and preserve user edits across unrelated parent updates.
The View IR route rejects `disabled` and `readonly`/`readOnly` attributes, including
false-valued forms and metadata-only picker options, because it cannot preserve
those states. This restriction does not apply to the separate authored-module path.
View IR also rejects raw HTML (`{@html}` and `v-html`), including in flattened
labels and picker options, because its text nodes cannot represent markup.
The shared parsers still retain raw HTML for the HTML renderer and the separate
authored Svelte compiler. Vue `v-text` continues producing literal text.

## What the framework frontends actually compile

The shared JSX/TSX template frontend accepts fragments rather than JavaScript
modules. Leading module declarations fail explicitly instead of becoming visible
text. The CLI has a separate authored-module path for Moonshine DOM and SSR,
described below; other framework emit targets retain the template contract.

The shared Svelte and Vue template parsers compile **the template only**.
`<script>` and `<style>` blocks are extracted verbatim and never executed by
that path. Its extracted scripts do not provide these component semantics:

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
  reference variables. Structural directives (`*ngIf`, `*ngFor`, `@if`, `@for`,
  `@switch`, `@defer`) lower the way `v-if` / `v-for` do, and `{{ … }}` reuses
  the Vue text scanner.

Native `moonshine-dom` and `moonshine-ssr` builds have a separate authored
Svelte module path. A `.svelte` entry or imported child compiles its instance
script inside a component function, hoists imports, and retains module-script
exports. The generated `.svelte.ts` modules use the existing native JSX compiler;
they do not pass through View IR or evaluate scripts during the build.

Instance `export const`, functions, classes, and named local aliases expose
properties on the native component reference. Ordinary constants and functions are
captured values; mutable locals, classes, exported props, and named state aliases
use accessors. Named derived aliases expose live readonly getters. Direct state
exports capture their initial values; direct derived exports fail explicitly.
Exported props support local instance overrides that reset when the
parent value changes, including props declared with `const` destructuring.
Legacy `export let`/`var`, instance
reexports, and imported instance exports fail explicitly. Type-only declarations
remain local and do not become instance properties.

Component `bind:this={reference}` supports declared variables and identifier-rooted
member expressions, including computed keys. Authored Svelte DOM references use
the native `publication: "post"` option: they publish after the child's own
mount/effect callbacks, before ancestor post/mount callbacks, and clear to `null`
after enclosing teardown. This requires the scheduler installed for every
authored Svelte DOM component. Empty-export components still expose an empty instance. Ordinary member targets
are evaluated by the binding callbacks without subscribing to target changes,
matching the pinned Svelte behavior. SSR validates the target but never reads or
publishes it. The native instance is an opaque object; its prototype and lifecycle
callback ordering do not fully match Svelte. DOM element references, getter/setter
forms of `bind:this`, and each-block reference capture remain unsupported.

The authored subset supports `$state` values, including ordinary nested objects
and arrays, arbitrary initializers, no-argument state and typed initializers.
`$state.raw` keeps its initializer unproxied and reacts only to replacement,
using strict equality (`===`) on the client. Function-valued raw state is retained
as a value. Server state is already raw for both forms.
State declarations currently require direct component-instance bindings;
state declarations in nested helpers and class fields remain unsupported.
Reads, replacement assignments, compound assignments and postfix updates retain
lexical scope. Ordinary function calls preserve their unbound `this`; object
method calls preserve their receiver. Generic instantiation wrappers, such as
`(callback<number>)()`, retain this behavior for rune, prop, snippet-parameter and
scalar template-const callbacks, including optional calls and tagged templates.
`$state.snapshot(value)` produces a
snapshot through the native state helper: plain data is copied, while
uncloneable values are retained. Snapshot calls accept trailing commas and
transparent parentheses/TypeScript callee wrappers. The SSR direct-initializer
identity rule below also applies to those forms; other expression contexts clone.
The source compiler also supports
`$derived(expression)`, `$derived.by(fn)`, and live `$props()` bindings. Props
may use an object alias or flat destructuring with renamed fields, defaults,
and a rest object. A component may declare `$props()` only once; shadowed ordinary
functions named `$props` are unaffected. In DOM output, destructured bindings remain live in closures
and child props; local assignments override them until an upstream dependency invalidates the
prop. Optional typed props with defaults retain their non-undefined value type.
DOM object aliases and rest objects forward live string/symbol keys and readonly
value descriptors. They exclude any destructured named props and Svelte internal
`$$slots`, `$$events`, and `$$legacy` fields. Authored component spreads preserve
source order with explicit props before, between, or after layers. DOM layers
remain live when objects or keys change; computed spread expressions use the
same owned caching as ordinary component props. SSR snapshots the ordered layer
descriptors for each render. Component bodies become callable children snippets
and override a spread's `children` value. DOM `$props` aliases/rest exclude the
private native component-reference symbol while retaining user symbols.
Intrinsic spreads and generic JSX spreads remain unsupported.
Named authored Svelte SSR prop expressions evaluate left-to-right in the parent
before child setup, including unused props and values overridden by a later
spread. Spread object getters remain lazy; callback props and snippet bodies are
not invoked merely by constructing props. Binding getter/setter metadata remains
intact. DOM output and generic JSX retain their existing live getter semantics.
Ordinary authored component props may use `ref`, `innerHTML`, `outerHTML`,
`dangerouslySetInnerHTML` and `__proto__`. These names remain component data:
they do not register a native reference, inject markup or change a prototype.
Native SSR deliberately preserves an own `__proto__` data prop, unlike the
reference Svelte 5.57.1 SSR compiler's object-literal treatment of that name.
Intrinsic and generic JSX restrictions remain in place; component instance
references still use `bind:this` and raw markup still requires explicit `{@html}`.
Component `bind:prop={target}` (or identifier shorthand `bind:prop`) passes a live
getter and setter that override every spread regardless of attribute order.
This includes named props such as `open`, `checked`, `ref`, HTML-like names and
explicit hyphenated names such as `bind:aria-label={label}`. `__proto__` remains
an ordinary own prop, without changing prototypes. `bind:this` retains its
separate exported-instance behavior. A bound `children` prop cannot be combined
with an authored body until default-slot support is implemented.
Bare attribute words `true` and `false` are strings; use `{true}` and `{false}`
for booleans. Binding values require braced expressions or omitted-value shorthand.
Targets require an identifier root and
may include member access. Direct assignment to constants/imports, optional chains
and direct snippet parameters are rejected. Flat `$props` defaults may use `$bindable()` or
`$bindable(fallback)`, including renamed and typed bindings. DOM writes forward
to a bound parent; unbound values retain local overrides. Object/array defaults
are reactive and isolated per instance. A bound undefined value with an explicit
fallback throws; a no-argument `$bindable()` permits undefined.
Typed defaults retain their declared prop type, including mutable empty arrays,
while explicitly undefined defaults retain undefined. Ordinary array defaults
remain nonreactive. DOM text expressions track separately, so another reactive
update may leave a mutated ordinary array display stale where Svelte refreshes
the grouped text expression.
SSR snapshots bindable props and publishes their final value after successful
child rendering only when the parent remains undefined. Parent template retries
stabilize surrounding text without repeating parent script setup. Named component
bindings also accept `bind:open={get, set}`: the two expressions capture in order
once per template occurrence and are called without a receiver. Dynamic component
replacement retains those callbacks; if/key remounting captures them again.
The pair must contain exactly two expressions without surrounding parentheses.
This does not enable getter/setter forms for intrinsic bindings or `bind:this`.
In SSR, a pair setter can receive a child's fallback without forcing a template
retry: preceding text can retain its earlier value while following text sees the
write. Sibling component setup still follows native SSR construction order,
which can differ from Svelte's interleaved setup and serialization. Native errors
use host diagnostics rather than Svelte's exact error codes.
In particular, if child setup mutates a shared object's field, preceding text and
an enclosing element's attributes can observe that later mutation. Svelte retains
their earlier values. Scalar replacement and undefined-parent fallback settlement
have separate retry behavior; they do not establish shared-object ordering parity.
Context helpers called from deferred SSR text also run after component
initialization and currently throw; capture their values in script setup instead.
Local and member component tags select reactively by identity using the native
`mapArray` control helper. Keeping the selected component retains its local state
while props update; replacement disposes the previous instance, and a falsy
selection renders nothing. Bare imported components stay direct calls.
It supports ordinary HTML, imported component tags, event attributes such as
`onclick`, quoted attribute interpolation, `{#if}`/`{:else if}`/`{:else}`, and
`{#key expression}` blocks. Key blocks retain their subtree for equal keys and
dispose/recreate it when the key changes; equality uses object identity, treats
repeated `NaN` as equal, and accepts null/undefined keys. They use the native
keyed `For` control without changing the shared template-only parser. SSR renders
the body once and skips evaluating the key expression, including its side effects.
Static checkbox inputs support `bind:checked={target}` and `bind:checked`
shorthand. Targets may be writable local state/props or object members whose
base is an identifier. Calls, constructors, literals, and `this` cannot be the
base of a binding target; computed keys may contain arbitrary expressions. The DOM
binding updates the model before a plain `onchange` handler, synchronizes model and input,
and adopts the initial checked state when the model is nullish. Connected form
resets synchronize the model unless canceled; disposal removes binding listeners.
`defaultChecked` defines reset state; combining `checked` with the binding is an
error. SSR emits the bound checked value without client initialization/writeback.
`bind:value` supports text/search/tel/url/email/password/number/range/date/
datetime-local/month/week/time/color inputs (including
inputs with no type) and textareas with no authored body. It supports shorthand
and writable state/props/member targets, defaults, input events, and connected
form reset/cleanup. Empty numeric input writes `null`; equal numeric values retain
the user's formatting. SSR preserves nullish state without adopting defaults,
omits nullish input value attributes, and escapes textarea values as text.
Numeric reset uses the raw default, including `NaN` for a nonnumeric default.
Static select `bind:value` supports single/multiple selection, raw object/function
and numeric option values, imported keyed options, change events and connected
form reset/cleanup. Bound `multiple` must be bare or a literal string; expression
forms reject before normalization. Undefined multiple initialization follows the
independent runtime's array contract rather than Svelte's scalar initialization
quirk. Generated SSR selects require the Moonshine 0.4.10 SSR selection
policy/value-slot contract. Its GitHub runtime release and verified tarballs are
available; npm publication is separate, and these local compiler changes do not
publish the Crepuscularity compiler. Raw option identity transport for
hydration is unsupported.
Dynamic input types, textarea body defaults, function bindings and other
directives remain unsupported.
Mixed quoted attributes use the shared Svelte character-reference decoder,
including named entities and legacy semicolonless forms where valid in an
attribute. Unknown references stay literal, and decoding runs once. Literal-only
attributes retain JSX entity decoding.
Bare intrinsic attributes use empty strings, while component bare props and
expression values retain their booleans. Checked controls and the native
`defaultChecked`/`defaultValue` properties retain true; bare `value` also retains
true on textarea, select, and option. Static checked strings express presence.
Static `checked` and `value` attributes on inputs also establish form reset
defaults unless an explicit `defaultChecked` or `defaultValue` is authored.
Expressions and quoted interpolations set live values without adding defaults;
explicit defaults retain their precedence in either attribute order.
Intrinsic event attributes require JavaScript expressions or shorthand,
including a single quoted interpolation;
bare and string handlers fail during parsing.
Lexical references are resolved
with Oxc, so nested shadowing and object shorthand retain JavaScript semantics.
DOM derived values use `createDerived` from `@tschk/moonshine-svelte/runes`, with
lazy evaluation, cached reads, dynamic dependencies, and owned cleanup. Mutable
`let` derived bindings support optimistic local overrides; invalidation clears
the override even if the newly computed value equals the previous value.
`const` bindings remain readonly. DOM default prop expressions evaluate during setup
when initially missing; later reads use their cached reactive derived value.
This fallback behavior follows the inspected Svelte 5.57.1 contract.

DOM state uses `createState`, `createRawState`, and `stateSnapshot` from the native rune helpers.
Ordinary plain objects and arrays get deep reactive proxies with isolated writes;
nonplain objects remain opaque. Accessor properties and unusual own-property
descriptors fail at runtime. External raw-object mutations and some descriptor,
prototype and identity edge cases do not fully match Svelte.

SSR state and derived cells use `createServerState` and `createServerDerived`
from `@tschk/moonshine-svelte/ssr`; the host adapter must export these helpers.
Component derived cells explicitly request caching, while companion factories
capture their rendering context when each cell is created.
SSR state retains raw values and identity without reactive proxies. Direct
`$state.snapshot(...)` variable initializers retain the argument's identity,
matching pinned Svelte 5.57.1; snapshots in other expression contexts still clone.
Destructured props and their defaults are evaluated once, in declaration order; object aliases
and rest props are mutable copies of enumerable own string/symbol keys, excluding `$$slots`
and `$$events`. SSR derived values compute lazily and cache successful reads,
without dependency tracking. Local non-nullish overrides take precedence; assigning
null or undefined restores the cached computed value, matching Svelte 5.57.1.

Bare `$effect(callback)` registrations run in a batched microtask after DOM
updates, track synchronous reads, and run cleanup before replacement or disposal.
Nested effects and helper-function registrations retain native ownership. Calls
must be standalone statements within functions; module initialization is rejected.
SSR removes entire registrations, including callback-factory argument evaluation.
Parentheses and transparent TypeScript assertions/non-null wrappers around bare,
pre-effect, and tracking callees are normalized without changing lexical shadowing.
Comments inside replaced callees remain grouped to preserve return/throw semantics.
`$effect.pre(callback)` uses the same registration restrictions. Every authored
Svelte DOM component enters `@tschk/moonshine-svelte/pre` scheduling, including
parents without a pre-effect: subsequent writes batch into a microtask, parent
pre-effects read the old DOM, and structural updates dispose removed children
before their pending pre-effects run. Initial pre-effects run during setup.
SSR removes the entire registration and does not import the scheduler.
The native scheduler currently reruns pre-effects invalidated by `bind:this`
publication or clearing before pending ancestor post/mount callbacks; pinned Svelte 5.57.1
runs those reruns after the post/mount phase. Cleanup state in that extra cycle
can also differ. Ordering across synchronous non-Svelte host ancestors is outside
the authored Svelte subtree's scheduling contract. `tick` remains a single
microtask rather than a complete Svelte flush guarantee.
Manual `$effect.root` scopes created outside a scheduled component do not install
a scheduler themselves; registering a pre-effect there currently throws. Ordinary
post-effects in those roots retain their existing behavior. A root created inside
a scheduled component currently inherits that component's scheduler: its
pre-effects stop updating after the component is disposed, even while the manual
root remains live. Independent pre-effect root lifetime is not implemented.
Ordinary DOM event handlers have no active owner, so late effect registration
fails. `$effect.tracking()` queries synchronous dependency collection: false in
module/setup/event code and `untrack`, true during render/derived/effect execution.
It uses the native `runeTracking` helper on the client and is constant false in
SSR. Queries require zero arguments and reject optional/generic calls. Exact
teardown-context checks and other `$effect` variants remain unimplemented.
`$effect.root(callback)` creates an untracked, manually disposed scope and returns
its disposer. It may appear in module initialization or ordinary expressions,
including event handlers. Its nested effects stop with that scope; separately
created nested roots survive until their own disposers are called. The scope
survives component disposal. DOM output uses native `createRuneRoot`; SSR replaces
the entire call with a no-op disposer, without evaluating a callback factory.
Exactly one non-spread argument is required. Optional call spellings and
TypeScript type arguments are normalized to the same builtin operation.
Runtime compatibility is incomplete: ordinary and raw-state cleanup reads match the
pinned lifecycle fixture, but execution order across independent roots differs.
Raw state uses the native `createRawState` helper to retain teardown history.
The source lowering and native scheduler do not fully reproduce Svelte
scheduling or teardown internals.

Named `onMount`, `onDestroy`, `getContext`, `setContext`, `hasContext`,
`createContext`, and `getAllContexts` named imports/reexports from `svelte` use
`@tschk/moonshine-svelte/svelte`;
`untrack` uses the native reactive host. Declaration-level `import type` and
inline type modifiers stay type-only when these imports are split between hosts.
The same named-helper bridge applies to reachable ordinary JS/TS modules. Named
reexports preserve aliases and type modifiers; ordinary modules may reexport a
helper as default, while Svelte components reserve their generated default export.
Unsupported helpers, default/namespace/side-effect imports, wildcard/namespace
reexports, dynamic imports, and import-type queries fail explicitly instead of
mixing runtimes.
Context access is supported during
component initialization, with ancestor inheritance and per-component overrides.
Keys may be arbitrary objects or symbols; an entry containing undefined still
counts as present. `createContext<T>()` returns the typed `[get, set, has]`
triplet and its getter throws when no provider exists. `getAllContexts` returns
the live per-component map, including inherited entries; writes stay local to that
component and its subsequently initialized descendants.
In authored Svelte SSR, intrinsic event attributes are omitted before their
expressions can evaluate; component event/callback props retain ordinary prop
semantics. The generic JSX compiler retains ordinary JSX expression evaluation.
DOM event expressions preserve Svelte's late dispatch reads and nullish no-op
behavior. Factories requiring memoization use an owned lazy derived value; it
recomputes after dependency changes only when the event is dispatched again.
Normal functions receive the event element as `this`; inline arrows retain
lexical capture. Inline listener-option objects remain a native extension.
Component prop computations use per-occurrence lazy derived values when they
contain stateful calls or complex reactive expressions. Repeated child reads
share the result until its dependencies change. Simple reactive references stay
live; nonreactive expressions, including inline functions, are captured once
when the component is created. SSR retains ordinary prop evaluation.
Writable child prop cells can still refresh a parent computation on dependency
invalidation before its next explicit read; exact scheduling parity remains open.
Browser mount callbacks may return cleanup; SSR skips mount callbacks and runs
destroy cleanup per request. Consumers need the native Moonshine core, DOM,
and Svelte helper packages. Mount the generated component through native
`h(Component, props)` within `render` or `renderToString`, so it receives a
component owner and lifecycle context. No Svelte runtime/compiler is imported by
generated modules. Local Svelte imports, reexports, dynamic imports, and type queries are
rewritten to the resolved generated paths; module manifest version 1 is retained.
Named `mount` and `unmount` imports and reexports from `svelte` also bridge in
ordinary reachable launcher modules. Browser builds use the optional
`@tschk/moonshine-svelte/mount` entry: `mount(Component, { target, props })`
returns the component's synchronous typed exports, and `unmount(instance)`
disposes its owned output. Targets are current-realm elements or shadow roots;
optional anchors must remain children of the target through setup. Root context
maps are copied per invocation. SSR uses `/server-mount`, whose operations throw
synchronously when called and are safe to import without DOM globals. Hydration,
outro transitions and legacy mount event/error options remain unsupported.

Generated public factory types preserve annotated props, required/defaulted
binding keys and exported method/value types. Ordinary exported constants remain
writable instance fields; getter-only derived exports remain readonly. A typed
forwarder preserves ESM default-function hoisting and invokes setup once; metadata
itself adds no SSR value reads. This requires the matching native Moonshine
runtime entries, which older published adapters may not include. Normal project
type checks support setup-local interfaces and recursive props. Declaration output
also supports closed setup-local interfaces, including generic, mutually recursive
and merged declarations, by lifting their resolved type symbols without runtime
changes. Non-generic interfaces can also capture direct setup-local `typeof` value
or property queries, including self-recursion. Each query retains its original
declaration location so merged interfaces preserve flow-sensitive types. Generic
captures remain unsupported, but captured interfaces may extend or reference closed
local interfaces, including generic base and member types. Queries in
method/function/index signatures, dependencies on other captured local
interfaces/types and implicit `arguments` remain lexical;
declaration emission can still report TS4060 for those cases or captured values
whose inferred types themselves contain private local names.

The manifest's `sourceEntry` records the authored input, while `sourceModule`
records its actual emitted path relative to the output directory, including any
collision suffix. Consumers should use `sourceModule` to locate the compiled entry.

Ordinary authored text interpolation converts values before native JSX rendering.
Booleans remain visible, nullish values become empty text, arrays stringify with
commas, and function values are displayed rather than invoked as accessors. DOM
uses string-hint coercion (Symbols throw); SSR uses `String` coercion (Symbols are
text). Adjacent text expressions share a chunk, preserving conversion order;
single-expression DOM text retains Svelte's raw-value cache. Native effect grouping
and cross-chunk coercion scheduling are not full Svelte matches. Mixed quoted
attributes use string-hint coercion in DOM and default-hint coercion in SSR,
preserving left-to-right evaluation and nullish-to-empty conversion. A sole
quoted interpolation retains its value type.
Both hosts reject Symbols in mixed attributes; DOM update errors currently surface
synchronously through the native scheduler rather than Svelte's asynchronous delivery.

Authored literal text decodes character references with Svelte 5.57.1's rules,
including legacy semicolonless names, numeric validation, and textarea-specific
boundaries. Static DOM text uses browser decoding, including two-codepoint named
references; SSR and dynamic text preserve Svelte's scalar-table behavior.
Expression results are never entity-decoded. Quoted attributes retain the separate
subset described above. Fragment cleanup trims decoded ASCII whitespace at edges,
collapses spacing next to elements and blocks, and preserves expression-adjacent
spacing and non-ASCII whitespace. Comments retain separate text chunks for cleanup.
`pre` and `textarea` preserve descendant spacing; `pre` also follows the host's
initial-newline rules, retaining an SSR comment before a leading block/component
so HTML parsing preserves its text. Table/select formatting whitespace is removed.
This applies to the admitted HTML subset; native JSX still rejects SVG and other
unsupported tags.

DOM text expressions do not yet use Svelte's call-bearing expression memos or its
shared fragment render effects. Repeated reads returning the same object can
coerce it again, and read/coercion or exception ordering across text and attributes
can differ. Per-reader caching alone does not preserve that shared ordering.

SSR expression text uses owned plain children, including `textarea` bodies and
`option` labels, so the host escapes it once and can match a select's value against
its option text. A textarea cannot combine `value` with body content, including
whitespace. Tag-looking textarea content, comments, scripts and styles remain
literal text; expressions still interpolate, while blocks and directives fail
explicitly. One initial literal LF or CRLF is consumed.
DOM expression bodies update the live value while keeping the reset default
independent. Sole-expression bodies compare raw values before conversion; mixed
bodies compare their composed text. Repeated object identities skip conversion,
while repeated `NaN` values are written again. Literal-only bodies establish
the default unless an explicit `defaultValue` supplies the DOM reset value. Updating
that reset value preserves the current live value for a static body, including
after a form reset. DOM default expressions use DOMString coercion: nullish values
retain their spellings and Symbols throw. SSR erases textarea `defaultValue`
attributes without evaluating them and renders only the body. Other casing, such
as `defaultvalue`, remains an ordinary SSR attribute.
Dynamic bodies distinguish retained literal line breaks
from expression-produced breaks; SSR matches the browser-parsed value, including
CR/CRLF and decoded newline entities. Explicit DOM `value` expressions compare raw
values before coercion and preserve user edits when those values are unchanged.
Initial nullish values leave an explicit reset default intact. Changed values use the DOM setter,
including its Symbol errors. SSR preserves leading newlines from an authored
literal LF/CRLF prefix in `value`, while expression-first values follow HTML's
initial-newline removal rule.

Native authored Svelte expression boundaries use the existing Oxc JavaScript parser,
including braces inside comments or regex literals, division, nested templates and
JavaScript strings inside quoted attribute interpolations. Trailing line comments
remain separate from generated punctuation. Shared template/DSL scanners retain
their existing behavior.

Native authored `{#await source}` blocks support pending, then and catch branches,
inline then/catch shorthand, typed or destructured branch parameters, and nesting.
DOM branches retain their owners while their state is unchanged, ignore stale
promise completions and stop publishing after disposal. SSR renders the pending
branch for promises and the then branch for ordinary values; it never waits for
resolution. Branch bindings are readonly live facades: member writes and supported
member bindings remain available, while direct reassignment and direct bindings
fail explicitly. Erased `typeof` queries retain branch types, and callback calls
and tags preserve their original receivers through TypeScript wrappers. Generated
modules require the matching native `/await` and `/runes` runtime entries. Async
SSR, hydration and transitions remain unsupported.

Explicit `{@html expression}` inserts unsanitized markup. In DOM output it updates
owned content reactively, retains nodes while the raw value stays equal, clears
nullish values, and disposes the content with its owner. Sole-child HTML elements
use their own parsing context (including table sections); root and mixed content
use an owned raw node list. SSR emits raw markup while ordinary interpolation
continues escaping text. Inline scripts are inert in the DOM, but raw event
attributes remain executable. Raw HTML within known SVG contexts and hydration
remain unsupported. Raw HTML inside textarea fails explicitly, matching
Svelte. Ordinary JSX continues rejecting raw HTML attributes.

Root-level `{#snippet name(parameters)}…{/snippet}` declarations and required or
optional `{@render name(args)}` calls are supported, including snippets explicitly
passed as component props. Direct snippet declarations inside a component body
become named props and stay local to that component occurrence, including its
opening attributes and sibling snippets. They take precedence over spread props;
an explicit prop of the same name is an error. A `children` snippet cannot be
combined with ordinary body content. Whitespace and comments alone do not create
an implicit children snippet when named snippets are present.
Each occurrence retains its parent owner, so nested components inherit context
and are disposed before that parent.
Explicit type-only `Snippet` imports and reexports from `svelte`, including inline
`type` specifiers and aliases, are rewritten to the native `/svelte` helper entry.
Tuple parameter annotations remain intact. Runtime `Snippet` imports, unknown
types, and Svelte import-type queries remain unsupported; generated render cells
are a private compiler contract rather than the public source callable type.
DOM rendering retains the subtree while argument values
change and replaces it when the snippet function changes; optional nullish calls
clear it. Simple parameters remain live reads, while argument calls use owned lazy
derived values. SSR uses ordinary value arguments and preserves call receivers.
Render calls construct snippet children before later sibling components; optional
nullish calls skip their arguments. Omitted arguments read as undefined. Parameters may be identifiers with TypeScript
annotations; declarations nested directly in intrinsic elements, control blocks, or other snippets,
destructuring, defaults, rest parameters, spreads,
generic calls and optional member
chains fail explicitly. Snippet parameters cannot be reassigned or bound directly;
member mutations and member bindings remain supported. This generated snippet ABI
is private to native authored modules.

For example, `Counter.svelte` can contain:

```svelte
<script>
  let count = $state(0);
  let doubled = $derived(count * 2);
</script>
<button onclick={() => count++}>{count}/{doubled}</button>
```

Build with `crepus web build --emit moonshine-dom --entry Counter.svelte`;
import the default component from `dist/CrepusEmit.moonshine-dom.ts`.
For server rendering, select `--emit moonshine-ssr` and use its corresponding
facade with the native SSR `h` and `renderToString` functions.

Native authored `svelte:element` supports ordinary HTML tag selectors, falsy
removal, identity retention for an unchanged tag, events and `bind:this`. Tag
replacement creates the new branch before disposing the old one. Intrinsic
`class:name`, `class:name={expression}` and quoted single-expression flags use
Moonshine's opt-in `/class` helper; base and flag inputs have independent memos.
The scope hash precedes flags, and an authored class base retains its attribute
position. Static and interpolated attribute literals share the core entity
decoder with native rendering; JavaScript string expressions remain literal.
Flat scoped-CSS selectors conservatively account for optional dynamic siblings.
Dynamic `xmlns` values select HTML, SVG, MathML, custom or empty namespaces at
creation. Namespace-only changes retain the element's actual namespace and update
its ordinary xmlns attribute. Creation failures precede attribute input reads;
namespace/attribute conversion and errors follow browser WebIDL behavior. A null
tag still evaluates creation-namespace input on the client; SSR treats xmlns only
as an attribute and skips its reads for null tags. This requires the compiler-facing
`hSourceNS` runtime helper from the Moonshine 0.4.9 runtime sources/tarballs.
Local compiler sources admit static/inherited MathML, MathML text integration
points and static annotation-xml encoding. Dynamic MathML tags retain their lexical
namespace; annotation-xml children are preserved even where Svelte 5.57.1 drops
them. Dynamic annotation-xml encoding, intrinsic spreads, `:not`, and combinators
across unknown branch/component relations remain explicit errors. MathML initial
render and DOM/SSR typing are verified; reactive update admission is still pending. Local compiler source
integration does not establish support in a published compiler release.

Native authored Svelte modules also compile intrinsic `use:action` /
`use:action={parameter}` and `{@attach expression}` directives. Actions accept an
identifier or static member path; parameter presence is preserved independently
of its value. Component directives, duplicate actions, action modifiers and
computed action paths fail explicitly. Named type-only imports of `Action`,
`ActionReturn` and `Attachment` map to the native directive adapter. SSR validates
rune placement in directive operands, then erases their runtime reads. DOM
output uses the Moonshine template ownership and directive APIs; compatibility
with a published runtime version has not been verified. This does not add
transitions, animations, hydration or shared-AST directive execution.

The authored module path also supports public and `#private` instance rune-field
initializers (`$state`, `$state.raw`, `$derived`, `$derived.by`), component-owned
store subscriptions, keyed/unkeyed each blocks, and a bounded flat scoped-CSS
selector subset. Private fields preserve native brands and initialization order;
DOM state assignments return the resolved proxy, while SSR state/raw fields remain
ordinary private data. Constructor state/raw reads use untracked live values.
Private calls and tags require an inline callable type without a bound `this`, or
an inferred arrow/function initializer; constructor state/raw calls, optional
private receivers, outer lexical private references from nested classes, and
compound/logical assignments through receivers other than `this` fail explicitly.
DOM assignments to cast-wrapped private `$state` fields also reject an `await` or
`yield` in the right operand; nested async/generator function bodies are unaffected.
These features do not imply full Svelte compatibility: TypeScript protected/private
access modifiers, static and computed rune fields, nested local store lifetimes,
unsupported each binding patterns, CSS nesting/at-rules and unsupported selectors
fail explicitly. Published runtime compatibility for private-field helpers has not
been verified.
Component-child snippet declarations retain the subset described above.

`$state.eager`, `$effect.pending`, nested/computed destructured props, legacy
reactive statements, hydration, and bindings/directives outside the
described subsets remain unsupported and fail explicitly where encountered.
Rest-prop types currently use `any`; inferred
rest-property type checking and the client compiler's eager identifier-default
optimizations are not fully reproduced. Conditional component initialization
uses native ownership scheduling, so lifecycle order is not a full Svelte match.
Failed DOM mounts clean up the partially created native owner, including its
registered destroy callbacks; the reference runtime does not always do so.

The native View IR template path still rejects Vue script blocks and scripted
Svelte files reached by DSL includes or slots. Native web-module builds also reject
style blocks in reachable Vue/Svelte template includes rather than dropping CSS. Those includes use template
semantics rather than the authored module graph. The shared template parser and
other emit targets retain their extraction-only contract.
Template-only Vue entry files without style blocks keep this View IR route for
native web module targets. A Vue entry with an actual script or style block uses
the separate bounded authored module compiler; script-looking comments and quoted
attributes do not select it. Template-only Vue children imported by an authored
module compile through that graph for DOM and SSR, without injecting a script
block. Plain unscoped CSS, including an explicitly empty `lang`, is emitted as
inventoried stylesheet assets. Scoped CSS,
modules, preprocessors, URLs (including string URLs in `image-set`), imports,
escapes, and CSS `v-bind` remain explicit errors. Empty and propsless JavaScript or TypeScript `<script setup>` blocks are supported
with an empty props parameter; ordinary setup declarations and template interpolation
remain in the authored module. When `defineProps` is used, it must be a typed,
top-level declaration. Parentheses and transparent TypeScript assertions around its
callee or result retain that contract. Normal `<script>` blocks remain unsupported
on this authored route.
This authored Vue route still rejects template conditionals and loops; the shared
template frontend's `v-if`/`v-for` support does not extend to that module compiler.
A const `withDefaults(defineProps<Props>(), { ... })` declaration supports finite
static defaults with the same bounded prop types. Defaults are hoisted to module
scope; they may use imports and factory-local parameters/bindings, but cannot
capture setup values. Successful factory results, including `undefined`, are cached
per instance; `null` stays explicit and Boolean casting follows default resolution.
Function defaults are preserved when the finite prop type resolves to Vue's sole
`Function` constructor; mixed function/data types still use factories.
Resolved setup props are shallow readonly; caller props retain their original type.
Authored casts and `satisfies` constraints around defaults remain in the emitted
TypeScript for downstream checking.
Spreads, accessors, computed/duplicate/unknown keys and `__proto__` defaults fail
explicitly. Indirect prop value types remain unsupported. This route requires the
native Vue default-props helper ABI; published runtime compatibility is unverified.
Named or aliased `watch` imports use the existing native reactive helper, requiring
`flush: 'sync'` and optionally `immediate`. Other watcher scheduling/options and
Vue app-instance error attribution remain unsupported; factory failures follow
the native host's error path.

`parse_vue_sfc_with_generics` preserves string-valued script `generic` declarations
alongside the script blocks, without changing the existing `VueScriptBlock` fields.
`parse_vue_sfc_with_metadata` also returns style attribute names for backend
validation, preserving the original `VueStyleBlock` fields and parser signatures. Native Vue module compilation rejects generic components explicitly
until their parameter scope and prop inference are supported.
Event handler identifiers may be stable setup functions, const
functions, or default/named value imports. Imported handlers are read and unwrapped
when the event fires, retaining live module bindings; namespace and type-only
imports are excluded. Stable setup function and const-arrow handlers may be async;
awaits and `for await` loops inside those functions remain intact. Top-level async
setup, including awaited loops, still fails explicitly. Unsupported macros inside
nested functions also fail.
Unresolved compiler macros cannot escape into runtime aliases or collections.
Local and imported bindings with the same names retain their ordinary lexical meaning;
a declaration using `defineProps` still requires the unshadowed compiler macro.
Named and aliased `onMounted`, `onBeforeUnmount`, and `onUnmounted` imports from
`vue` use the native `@tschk/moonshine-vue/lifecycle` helper. Mixed imports with
reactive helpers are split between their native modules. Mount callbacks run
after native DOM insertion; teardown callbacks use the native before/after disposal
phases. SSR does not run these callbacks. Other lifecycle hooks, explicit target
instances, and full Vue scheduler parity remain unsupported.
Authored Vue literal text decodes HTML character references once, including numeric,
legacy named, and multi-scalar references. Runtime interpolation values are never
decoded. Attribute entities and text requiring whitespace condensation or decoded
ASCII boundary whitespace remain explicit unsupported errors.
Authored Vue template expressions unwrap default and named value imports, including
refs used in arithmetic and bound attributes. Expression-local parameters retain
their own bindings; namespace and type-only imports are excluded. Ordinary JS/TS
modules still need native reactive helpers rather than an automatic Vue bridge.
Authored Vue templates can render explicit PascalCase default or named value
imports as native components, including `.vue` and `.tsx` children. Static and
bound declared props use the existing native component getters. Top-level
unshadowed `defineExpose()` and `defineExpose(object)` statements lower in DOM
to `exposeComponent(exposeRefs(...))` (and components without `defineExpose`
expose an empty null-prototype object in DOM so `createApp().mount()` and
component refs always receive a bounded exposed instance). SSR erases runtime
exposure while preserving `ComponentView` return-type branding in both targets.
Static `ref="name"` and `:ref="'name'"` template attributes on intrinsic
elements and imported PascalCase components bind top-level `const` `ref` /
`shallowRef` setup declarations in DOM via `bindRef` and `componentRef` (with
synchronous `commit` cleanup on disposal) and are validated then omitted in SSR;
combining `ref` with `v-model` on the same intrinsic control, function/dynamic
`:ref` expressions, and `useTemplateRef` remain explicit errors. Named/scoped
slots, key bindings, undeclared fallthrough class/id/style attributes,
namespaces, and dynamic component selection remain errors.
Component event listeners use stable handler identifiers for declared typed
`defineEmits` events. Imported ref-valued handlers unwrap inside live getters,
including each callback in fused listener arrays; ordinary functions remain callable.
DOM `.once` listeners retain their separate once key; SSR fuses listeners in source
order. Other component event modifiers remain errors.
This uses the native props helper's coalesced updates; nested update ordering is
not full Vue renderer scheduling.
Local Vue setup type aliases and interfaces are preserved at module scope, including
finite props aliases, intersections, and interface inheritance. Hoisted types may
reference imports, other hoisted types, and their own type parameters, but cannot
capture setup-local values, classes, or enums, including names shared by type and
value declarations. Generic public props shapes remain
unsupported; indirect prop value types keep the existing casting-metadata restrictions.
Vue prop type aliases, reexports, and inheritance chains are limited to 64 active
named type references. Cycles and deeper chains fail before output is written.
Dynamic imports of `vue`, `vue/*`, and `@vue/*` are rejected rather than
bypassing the named native helper bridge. Ordinary literal dynamic imports retain
the authored module graph behavior.

Imba IR checkboxes with no nonempty binding use their checked snapshot as the
initial form default. User edits survive host rerenders, and form reset restores
that default. Bound checkboxes continue reading their live checked value from scope.
Unbound Imba sliders likewise use their normalized snapshot as the form default;
bound sliders continue reading their live value from scope.
Imba file pickers render a labeled native file input, preserving accept filters,
multiple selection, and the original change event passed to the pick handler.
Imba badges render a span with literal text, shared styling and identity attributes,
and an optional `data-tone` attribute, matching the other framework emitters.
Imba WebView nodes render an iframe, preserving the source URL, classes, ID,
and explicit accessible label. Both `iframe` templates and `embed` with
`adapter="webview"` use this mapping.
React/Moonshine JSX, Solid, Svelte, and Vue dropzones cancel browser dragover
and drop defaults. Drop handlers still receive the original event, including
its file transfer data; cancellation also applies when no callback is supplied.
Imba dropzones use the same cancellation and raw-event behavior, preserving
child content, classes, IDs, and explicit accessible labels.
Moonshine DOM and SSR also support Dropzone content and metadata. DOM cancels
dragover/drop defaults and forwards the original event; SSR emits markup without
event attributes or executing callbacks.
Moonshine DOM and SSR WebViews render as iframes with their source URL, classes,
ID, and explicit accessible label, matching the other web emitters.
Their Spacer nodes render an empty `aria-hidden="true"` div with shared classes
and identity metadata, matching the other web targets.
Moonshine DOM and SSR Tabs preserve bound selection, labels, and panel content.
DOM supports arrow/Home/End navigation and local selection for unbound groups;
change callbacks receive the selected tab value. SSR emits the initial selection
without running refs or callbacks. Each rendered group gets unique tab/panel IDs
and reciprocal ARIA links, including separate instances and nested groups.

Moonshine DOM SlotRotate nodes rotate through literal phrases and release their
timers when the render owner is disposed. Empty and single-phrase nodes need no
timer; reduced-motion users retain the first phrase. Animated nodes require a
Moonshine render owner; intervals are bounded to
1–2147483647 ms. SSR emits the first phrase without creating timers.
Changing the reduced-motion preference while mounted stops or resumes rotation;
disposal removes both the timer and preference listener.
Generated DOM rotation imports `createRotatingText` from
`@tschk/moonshine-dom/rotating-text`; the installed Moonshine package must expose
that optional entry point. Components without SlotRotate and SSR output do not
import it.
Svelte SlotRotate uses an action, Vue uses directive mount/unmount hooks, and
Solid registers a ref with `onMount`/`onCleanup`. They share timer bounds and live
motion preference handling. Conditional removal and component disposal clean up
each timer and listener; SSR emits the first phrase without starting timers.
React/Moonshine JSX uses a module-level helper component with an effect and ref;
parent renders retain each mounted rotation, and effect cleanup releases its timer
and listener. SSR keeps the first phrase. Imba uses a span subclass with
mount/unmount hooks and the same shared timer helper; ordinary components omit
the rotation tag and runtime. Its server-compiled component serializes the first
phrase without mounting or starting timers.

Moonshine DOM and SSR IR emitters also support file pickers, including the standard
file-input alias. DOM forwards the original selection-change event; SSR emits only
the initial native input markup, without file values or event listeners.
Unbound Moonshine DOM/SSR IR checkboxes likewise use their checked snapshot as
the form default; bound checkboxes retain live checked reads. Toggle behavior is unchanged.

Svelte and Vue IR emitters preserve literal NUL characters in client-rendered
text and attributes through escaped expressions. Vue uses a runtime read to avoid
static HTML stringification. SSR strings retain NUL, but HTML parsing drops text
NUL and replaces attribute NUL with U+FFFD; this is an HTML transport limitation.

All IR emit targets validate their reachable template sources before writing.
Generated IR, components, and Imba bindings cannot replace an entry or referenced
source through the same canonical path, including symlinked destinations.
Authored module graphs and framework IR emission reject generated files that
resolve to the same output destination through existing symlinks, before writing
any output files. This includes Imba's companion bindings file.
Existing output destinations must be regular files. Directories, sockets, and
named pipes fail during preflight, before replacing the IR, component, companion,
or authored-module files.
Unresolved symlinks in output paths or their ancestors fail before publication;
ordinary missing directories and resolvable symlink directories remain supported.
Unreferenced generated templates can still be rebuilt into the site directory.
Filesystem write failures are not transactional; hard-link aliases are not detected.

Expressions in the markup are evaluated by the crepuscularity template
evaluator against the crepuscularity template context, not by a JavaScript
engine. Loop item variables in all frontends follow the evaluator's variable-name
rules. Loop-header separators (`as`, `in`, `of`) are recognized outside quoted
strings and bracketed expressions. `<style scoped>` is recorded but scoping is not implemented.

View IR lowering checks condition and loop-collection syntax before emitting
source, using the shared DSL parser without evaluating runtime data or arithmetic.
Missing context does not make a valid condition fail compilation. Function calls
and other expressions outside the DSL are rejected; dotted collection binding
paths, including numeric indices, retain their existing support. The direct
Moonshine DOM/SSR emitters also check these expressions when given supplied IR.

Unsupported markup constructs are **hard parse errors**, not silent drops. A
template that uses them fails to compile rather than rendering something
subtly wrong.

### Svelte

Supported: HTML elements and nesting, comments, `{expr}` interpolation,
`{#if}` / `{:else if}` / `{:else}`, `{#each list as item}`, `{@html expr}`,
static `class`, `class:foo={cond}`, `on:click={h}` with modifiers, Svelte 5
`onclick={h}` for the known DOM event names, `bind:value|checked|group|files`,
and `{value}` attribute shorthand (including surrounding whitespace, `{ value }`).
Other `bind:` directives, including `bind:this`, media and measurement bindings,
are hard parse errors in this shared template-only frontend. The authored module
compiler's separate binding support is described above.
Quoted class lists keep interpolation expressions together across whitespace: `class="w-{1 + 2} flex"` resolves to `w-3 flex`.

Shorthand names follow the evaluator: ASCII letters or `_`, followed by ASCII
letters, digits, or `_`; literal keywords are not variable names. Shorthand cannot
have an assigned value:
use `title={other}` instead of `{title}={other}`.

Crepuscularity's `<include src="parts/card.crepus" title={title} />` extension
also works in Svelte input. Use exactly one nonempty literal `src` or `path`;
other attributes become include props, including `{title}` shorthand. Duplicate
attributes and dynamic source paths are errors. Non-self-closing includes pass
their child Svelte markup as the caller's slot, including `{#if}` / `{#each}`
blocks. Paths resolve relative to the including file and use the shared parser
for the callee's extension. Include scope and slot rules are the same as the
indent/JSX frontends; this does not execute Svelte imports or source scripts.

The `{#each}` `as` separator is recognized outside quoted strings and bracketed
expressions.

Keyed `{#each items as item (key)}` accepts balanced nested parentheses and quoted
parentheses. The key must be nonempty with no trailing text; keying is not
represented in the shared AST and the key expression is not executed.

Svelte event modifiers `preventDefault` and `stopPropagation` lower to shared
`prevent` and `stop` semantics; `stopImmediatePropagation` lowers to `immediate`.
`trusted`, `self`, `once`, and `capture` retain their order.

Svelte extracts at most one top-level instance script, one module script
(`module` or `context="module"`), and one style block. Their bodies remain
verbatim. Duplicate, nested, or unclosed blocks are parse errors. Tags inside
comments, attribute values, or interpolation strings are not extracted as
blocks, and script/style tag-name prefixes are ordinary element names.
The complete template must be consumed: stray closing tags and orphan block
delimiters are errors. Empty interpolation bodies are rejected; an expression
that evaluates to an empty string remains valid.

Rejected with an error:

- Event modifiers `passive` and `nonpassive`, unknown names, empty modifiers,
  and foreign aliases such as `prevent`, `stop`, or `immediate`. Use the Svelte
  names `preventDefault`, `stopPropagation`, and `stopImmediatePropagation`.
- Bare event forwarding (`on:click` without a handler).
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
- Any capitalised tag. This frontend compiles markup and performs no module
  resolution, so native Svelte component imports remain unsupported. Use the
  explicit Crepuscularity include extension for file composition.
- `<svelte:window>` and the other `<svelte:*>` special elements.

Silently dropped: the key expression in `{#each … as item (key)}`.

Static class strings in JSX, Vue, Angular and Astro retain literal braces instead of reading context variables. Decoded string entries and object keys in host class lists retain the same literal behavior. Svelte quoted-attribute interpolation, Angular `{{…}}`, explicit class expressions and indentation DSL interpolation remain dynamic.

Class lists in Vue `:class`, Angular `[ngClass]`, and Astro `class:list` flatten nested arrays, including conditional objects. Quoted literals and object keys decode common escapes before splitting class names. String concatenations such as `'prefix-' + suffix` retain expression semantics in scalar and array class lists. Repeated object keys use their final condition at the original key position; distinct whole keys and separate array objects remain independent. Computed object keys such as `{ [name]: enabled }` are rejected; object keys must be static. Each key must be one complete quoted literal, a numeric literal, or a bare token containing only letters, digits, `_`, `$`, and `-` (including hyphenated utility names). Other punctuation and whitespace require quoting; concatenations and adjacent quoted literals are errors. Recursive class-list parsing is limited to depth 64.

### Vue

Vue SFC block attributes use exact names and support quoted `>` characters.
Script and style bodies remain verbatim; template extraction skips tag-like
strings inside attributes and moustache expressions. Closing tags must match
exactly, with optional whitespace before `>`. Inline self-closing blocks retain
empty bodies and metadata. Duplicate template blocks and external block `src`
attributes are parse errors; external SFC block loading is not implemented.

An element may have only one ID across `id`, `:id`, and `v-bind:id`.

Property bindings (`:name` / `v-bind:name`) require a property name and nonempty expression; use `:name="''"` for an empty string.

Plain `v-model` requires a nonempty binding expression.

Each element may have only one `v-for` directive; nested elements may declare their own loops.

`v-text` and `v-html` require nonempty expressions and cannot be combined or duplicated on one element.

`v-if` and `v-else-if` require nonempty expressions; bare directives and empty values are parse errors. An element may have only one branch directive (`v-if`, `v-else-if`, or `v-else`).
`v-else` closes its sibling chain, even when its body contains another `v-if`.
Whitespace and comments may separate valid sibling branches.

Supported: SFC splitting, `{{ expr }}` mustaches, `v-if` / `v-else-if` /
`v-else`, `v-for` (`item in items`, `(item) in items`, `of` form), `v-show`
(lowered to a conditional `hidden` class), `v-html`, `v-text`, plain `v-model`,
`v-bind:x` / `:x` including `:class` with string, object, and array syntax,
`v-on:evt` / `@evt` with the modifiers `prevent`, `stop`, `self`, `once`,
`capture`, `<template>` as a structural wrapper, void elements, comments.

Rejected with an error:

- Object spread `v-bind="obj"` — bind properties individually.
- JSX-style `class={expr}` — use `:class="expr"` instead.
- Event directives without a nonempty handler, including modifier-only `@click.stop`.
- `:style` — use utility classes.
- `v-bind` modifiers (`:x.prop`), dynamic binding arguments (`:[key]`),
  dynamic event names (`@[evt]`).
- `v-model` modifiers (`v-model.number`, `.trim`, `.lazy`).
- Any event modifier outside the five above — including **all key modifiers**,
  so `@keyup.enter` is an error.
- Every other directive: `v-once`, `v-memo`, `v-pre`, `v-cloak`, `v-slot`, the
  `#name` slot shorthand, and any custom directive.
- More than one top-level `<template>`, unclosed `<template>`, missing
  `</script>`, or top-level content outside `<template>`, `<script>`, and
  `<style>` blocks, including trailing markup. Comments, Unicode whitespace,
  and a leading UTF-8 BOM around the supported blocks are allowed.

`v-for` index/key bindings and destructuring are rejected: the shared loop AST
binds one item variable. Item names must follow the evaluator's variable rules.
Unlike Svelte, a capitalised tag
is **not** an error — it becomes an element with that tag name, and since no
module resolution happens, custom components render as unknown elements.

### Astro

The Rust `split_astro_source` API exposes borrowed frontmatter text and its
original UTF-8 byte range, plus the template slice and its original start offset.
It shares the template parser's line-based fence matching, distinguishes an empty
fence from no fence, and preserves script contents including CRLF. Splitting does
not execute JavaScript, validate markup, or establish authored module support.

Leading frontmatter requires both `---` fences; its contents are not executed.
Empty interpolation bodies are parse errors. Comment-only expressions can
contain braces, quotes, and Unicode. Block comments require `*/`, and line
comments end at their newline. Trailing code after a comment is rejected rather
than silently discarded. Comments between text runs leave surrounding text
intact.

Compiles markup only. JavaScript/TypeScript in `---` frontmatter is skipped;
imports, `Astro.props`, server execution, and component resolution do not run.
Supported: HTML elements, comments, `{expr}` interpolation, bare `{value}` attribute shorthand,
`class:list`, `set:html`, `set:text`, conditional expressions (`&&`, ternaries),
and `.map(item => markup)` lowered to the shared `ForBlock`.
Quoted `set:html` and `set:text` values are literal strings with HTML attribute
entities decoded; empty strings and whitespace are preserved. Braced values remain
expressions, and string literals inside expressions are not HTML-entity decoded.

Rejected: component tags, slots, script/style raw-text elements (including empty,
self-closing and external-script forms), spread attributes, `transition:*`,
`define:vars`, and map callbacks with index bindings, destructuring, or block
bodies. `client:*` directives are recorded as bindings; this frontend does not
implement Astro hydration.

### Angular

Whole-value interpolation works in `id="{{ name }}"` and
`class="{{ theme }}"` as it does in other attributes. Empty expressions and
mixed literal/interpolated values are rejected. An element may have only one
ID across `id`, `[id]`, and `[attr.id]`.

JSX-style `class={expr}` is rejected; use `[class]="expr"` or `[ngClass]="expr"`.
Literal and empty `class` attributes remain supported.

An element may have only one content replacement binding: `[innerHTML]`, `[innerText]`, or `[textContent]`.

Property bindings require a name and nonempty expression. `[attr.name]` also requires a nonempty attribute name; `"''"` is a valid empty-string expression.

Attribute bindings retain separate provenance. A null value omits the attribute;
other values use the shared evaluator's string formatting. In particular,
`[attr.checked]="false"` emits `checked="false"` (present), whereas
`[checked]="false"` omits the Boolean attribute. Attribute class strings stay
literal, and a textarea's attribute `value` does not replace its body/default
value. Attribute `bind` and `interval` never configure Crepus control bindings.
Attribute spellings `defaultValue`, `default-value`, `defaultChecked` and
`default-checked` likewise do not initialize native controls; ordinary property
bindings and native default aliases keep their existing behavior.
Competing writers for one attribute, including attribute/property pairs or an
attribute class binding with a nonempty class list, fail explicitly.

Native View IR projects supported attributes rather than preserving arbitrary
HTML. Attribute URL bindings remain snapshots; null unsupported disabled/readonly
attributes disappear, while present unsupported control state still errors.
GPUI macro lowering explicitly rejects HTML attribute bindings. `BindingOrigin`
now includes `Attribute`; downstream exhaustive AST matches must handle it.


Compiles the component template only; the component class is never read.
Supported: mustaches, property and event bindings, `[(ngModel)]`, `*ngIf`,
`*ngFor`, `@if` / `@else if` / `@else`, `@for`, class bindings, and
`<ng-container>` as a structural wrapper. Tracking expressions are ignored
because the shared IR has no DOM-diffing keys.
`@if (expr; as name)` aliases are rejected explicitly, including whitespace
variants. Quoted `as` or semicolons inside conditions remain ordinary string data.

Rejected: `<ng-template>`, `<ng-content>`, template reference variables,
`*ngSwitch`, `*ngIf` with an `else` reference, style bindings, event key
modifiers, global event targets (`window:`, `document:`, `body:`),
two-way bindings other than `ngModel`, loop index bindings,
`@switch`, `@defer`, and `@empty`. Dependency injection, pipes, and
`@Input` / `@Output` semantics are outside this frontend's scope.

## Limits

All five tag-based frontends cap nesting depth at 256 and error beyond it.
Angular `@else if` branches count toward that limit as nested conditions.
JSX checks actual parser nesting: angle brackets inside quoted attributes do not
count as tags or bypass the limit.
The indentation frontend also rejects excessive nesting with a parse error.
Valid deep markup uses stack growth during recursive parsing; the WASM JSON
binding serializes child nodes iteratively to avoid exhausting the host call stack.
Internal `RawParseError` diagnostics contain a message and an optional byte offset.

## Authored Moonshine modules

The aggregate authored-module host contract documented here uses the verified
Moonshine **0.4.49** baseline. Use matching **0.4.49** packages for
`@tschk/moonshine`, `@tschk/moonshine-dom`, and the applicable native
`@tschk/moonshine-svelte`, `@tschk/moonshine-vue`, or `@tschk/moonshine-solid`
adapters. Feature-specific versions below record individual ABI introductions;
they do not establish that an older release supports the complete current
compiler output. This baseline does not remove the unsupported cases or
feature-specific verification limits documented here.

This requirement applies to authored `moonshine-dom` / `moonshine-ssr` modules.
The independent View IR emitters for React, Svelte, Vue, Solid, and Imba use their
corresponding host frameworks and do not acquire a Moonshine dependency.

`crepus web build --emit moonshine-dom --entry Counter.tsx` (or
`--emit moonshine-ssr`) recognizes JSX/TSX modules using parsed import/export declarations. Plain JSX
fragments retain the existing View IR route. Module compilation parses source
without executing it, retains TypeScript annotations, imports, exports, and lexical
closures, and lowers supported intrinsic JSX to Moonshine's own `h` function.
It introduces no React or Solid runtime.

```tsx
import { createSignal } from '@tschk/moonshine-dom/reactive';
export default function Counter(props: { initial: number }) {
  const [count, setCount] = createSignal(props.initial);
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
```

Expression children and ordinary intrinsic expression attributes become reactive
accessors; event expressions stay handlers. Uppercase local/imported components
and member tags such as `<UI.Child />` receive getter properties, so reading
`props.label` tracks the caller's expression without changing that value into a
function. Component callback props remain handler values. A single body child is
an ordinary scalar `props.children`; multiple children form an array. An explicit
`children` prop is allowed only without body children. Direct JSX children and
JSX-valued component props are created lazily once under the caller's owner, so
reading them during an update preserves child state and cleanup ownership. Other
expression props and children remain live. Descriptor getter arrows retain the authored `this`, `arguments`, and `super` context. Signal creation stays inside the
authored function, so instances and SSR requests retain separate state. The host
calls the exported component inside its DOM `render` or SSR `renderToString` root.
This output does not mount itself or claim existing server DOM for hydration.

JSX fragments group empty, single, multiple, and nested children without a DOM or
HTML wrapper. Dynamic fragment expressions remain accessors; static sibling nodes
and component instances stay stable across updates. A hygienic local fragment
helper is emitted only when needed, without another runtime dependency.

Ternary JSX child and component-prop expressions with JSX branches use the native
host's owned `Show` control flow. Changing a condition between truthy values
preserves the selected child instance; changing branches disposes the old branch
and creates the new one. Nested ternaries follow the same rule, while scalar
branches remain reactive. Branches initialize lazily under the caller's owner.
This lowering applies to JSX expression positions, not arbitrary JavaScript
function returns or keyed list reconciliation.

Authored Svelte SSR conditionals instead attach their branch memos to the
component owner. Children are destroyed before their parent, and binding
settlement discards the previous attempt without publishing its destroy hooks.
Truthiness is cached separately so a different truthy value retains the branch.
Authored Svelte DOM conditional memos use the structural render scheduler so
parent removal disposes child branches before those children reread missing
parent data. DOM branch replacement retains the native host's cleanup-before-setup order.
Conditional children can initialize after following unconditional siblings;
cross-sibling setup and disposal order still differs from Svelte.

JSX on either side of `&&`, `||`, and `??` uses the same owned branches. The left
operand is evaluated once per reactive update, and the selected left value is
preserved, including `0`, empty strings, `false`, and `NaN`. `??` tests for null or
undefined rather than truthiness. Right branches remain lazy; nested logical and
ternary branches on either side share the ownership rules above. Each operand's
accessor initializes once under its owner, so reading a nested left choice does
not reconstruct its child. JSX evaluated on the left but not displayed still
receives cleanup when its branch or owner is disposed.

For SSR, supported named imports and reexports from
`@tschk/moonshine-dom/reactive` use the server host. Its `createEffect` and
`onMount` exports are no-ops; signal, memo, owner, and cleanup operations retain
their server behavior. Namespace, wildcard, dynamic, default, and unsupported
reactive import forms fail explicitly rather than scheduling browser lifecycle
work during server rendering. DOM imports remain unchanged.

For files without an explicit JSX source selector, the native module compiler
also bridges named imports and reexports from
`solid-js` for `createSignal`, `createMemo`, `createEffect`, `createComputed`,
`batch`, `untrack`, `onCleanup`, `onMount`, `on`, and the `Accessor` type. DOM output
uses `@tschk/moonshine-dom/reactive`; SSR uses `@tschk/moonshine-dom/ssr`, where
effects and mount callbacks do not run. This applies to reachable helper modules
as well as JSX files, so signals and rendering share one dependency tracker.
Aliases remain intact, and dependency metadata records the authored imports.

This bridge uses the native host signatures: signals require an initial value,
signal/memo options support `equals`, and effects accept a callback and optional
initial value. Solid debugging option types and richer `Setter`/`Owner` types
are not part of this contract. Default, namespace, side-effect, wildcard, and
dynamic Solid imports fail explicitly, as do `solid-js/*` subpaths and unlisted
exports, including stores, resources, control components, and owner/root APIs.
This is a primitive bridge, not execution by the Solid runtime.

An authored JS/TS module can opt into the local Solid adapter with a complete
leading `/** @jsxImportSource solid-js */` comment. Place it before any statement,
including `"use strict"` or `"use client"`. The selector is per file: tsconfig
settings and importing files do not select a runtime for their dependencies.
Duplicate, late, conflicting, or quoted selector mentions fail explicitly.
Other comments, directive prologues, and hashbangs are preserved.

Selected DOM modules use `@tschk/moonshine-solid/h` and the adapter's reactive
exports; SSR uses `@tschk/moonshine-solid/ssr` and the native server primitives.
The selected primitive set adds `createRoot` to the unmarked bridge, but excludes
`createComputed` and owner access APIs. Generated DOM conditionals use the
adapter's accessor-shaped `createSolidShow` helper. This does not admit public
Solid control components, stores, resources, hydration, or compiled Solid runtime
instructions. SSR retains native synchronous cleanup and suppressed effects.

Component tags and default component exports require local stable synchronous
functions. Imported, aliased, member, dynamic, async, generator, or reassigned
component identities fail explicitly. Runtime imports and reexports naming a
`.svelte` or `.vue` file also fail; type-only edges and companion utilities remain
allowed. Extensionless or indirect foreign functions called as ordinary JS are
not identified by this source-only check; callers must use an explicit native
frame bridge for those boundaries. Imported utility functions are not components.

This cohort requires an explicit Solid adapter bootstrap by the caller. It does
not select the ordinary project bootstrap automatically. Acceptance covers the
current local adapter; compatibility with published runtime versions is unverified.

`on` accepts a single accessor or a dense readonly tuple of accessors. It tracks
only those dependencies; callback reads do not add dependencies. Callbacks receive
the current input, previous successful input, and previous computation result.
`{ defer: true }` skips the first invocation and preserves its supplied result.
The native host keeps these semantics for SSR computations and memos; SSR effects
remain suppressed. Solid's server-only `on` behavior is not reproduced. The native
type signature has two generic parameters and retains readonly tuple inputs.

Named `mapArray` and `indexArray` imports, reexports, and type queries use
`@tschk/moonshine-dom/control` for both targets. They can share a declaration with
reactive primitives; the compiler splits that declaration while preserving
aliases, type modifiers, and authored dependency metadata. Mixed declarations
with import attributes or phases fail explicitly rather than losing metadata.

These helpers use the native two-argument contract: a dense readonly array
accessor and a mapper. `mapArray` retains item identities and supplies a live index
accessor. `indexArray` retains positions and supplies a live item accessor and
numeric index. Mapping initializes eagerly under the native owner, including for
SSR. Duplicate identities retain native occurrence order, which can differ from
Solid's optimized retention choices. Use `[]` for empty input; null/false inputs
and third-argument fallback options are outside this contract. JSX `For`/`Index`
components remain unsupported by the Solid import bridge.
Current native DOM validation retains keyed nodes and state during reversal but
loses focus on a moved input.

The initial compiler supports an explicit HTML intrinsic subset. Unsupported tags,
spreads, refs, raw HTML properties, component type arguments, source
JSX factory pragmas, and suspending intrinsic JSX expressions fail explicitly.
Component-bearing modules cannot shadow the `globalThis` binding used to construct
property descriptors; ordinary reads from `globalThis` remain supported. This is native
Moonshine source compilation, not execution of React, Vue, Svelte, or Imba scripts.
Stylesheets and asset imports are not compiled by this module path.

The CLI traverses reachable local `.js`, `.jsx`, `.ts`, and `.tsx` imports,
reexports, and literal dynamic imports, including type dependencies. CommonJS
`require` and TypeScript import-equals declarations are unsupported. React and
React DOM runtime imports, reexports, and dynamic imports (including subpaths)
fail explicitly: native JSX does not execute React's renderer or hooks. Use a
React island with `adapter="react"` for authored React components. Declaration-level
`import type`/`export type` and import-type queries remain supported. Inline-only
type specifiers still create runtime module edges under verbatim TypeScript
semantics and are rejected for these packages. Other bare package imports stay
external. Extensionless local imports first consider ordinary JS/TS modules, then
fall back to Svelte/Vue components. The selected tier must resolve unambiguously
inside `--site`; explicit component extensions bypass this fallback. Computed
imports, unsupported assets, source symlinks, non-regular source files, and output
paths that could overwrite source are errors. Use the project root as `--site`
when imports reach sibling source directories.

Module builds write the exported entry facade and its graph beneath
`modules/moonshine-dom/` or `modules/moonshine-ssr/`, preserving source-relative
paths and extensions. Both targets can share an output directory. They do not
produce View IR; unrelated older output artifacts are retained.

`crepus-module-manifest.json` describes the latest module invocation:

```json
{
  "version": 1,
  "target": "moonshine-dom",
  "entry": "CrepusEmit.moonshine-dom.ts",
  "sourceEntry": "Counter.tsx",
  "sourceModule": "modules/moonshine-dom/Counter.tsx",
  "files": [
    "CrepusEmit.moonshine-dom.ts",
    "modules/moonshine-dom/Counter.tsx"
  ]
}
```

`files` is the sorted set of emitted facade and module paths relative to the output
directory; the manifest itself is not in that list. Consumers must preserve the
listed paths together. The facade forwards actual named exports and a default
export only when the source has a runtime default export. Module graph validation
finishes before writing any artifacts. The manifest is a module-output contract,
not a stylesheet, asset, or hydration contract.

### Local native Svelte file-input binding

The locally verified authored compiler accepts `bind:files` on intrinsic inputs
with a literal `type="file"`, using identifier or member targets. DOM output calls
Moonshine's independent owned `bindFiles`; keyed row targets stay live through
reorder and same-key replacement. Nullish models preserve the browser selection;
an empty `FileList` clears it. Connected-form resets and listener cleanup use the
native binding contract. Native disposal removes a retained detached input's
listener, while the reference can still update through that input.

SSR emits no file value and never reads the bound getter or calls a change-handler
factory. Typed output retains unreachable assignment/change-handler checks.
Wrong model types, readonly targets and invalid change handlers fail strict
checking; JavaScript bodies remain JavaScript. Dynamic input types/elements,
intrinsic spreads and function bindings reject. Hydration is unsupported.

This compiler change is local and unpublished. It requires the Moonshine
`bindFiles` runtime added in commit `f214d60`; that export is absent from the
0.4.12 runtime release. Verification: 32 matching browser phases, eight Bun/Node
SSR comparisons, both strict type lanes, 16 typing negatives, 14 source
rejections and the combined 856-module source gate. See the Moonshine local
coverage ledger for the preserved evidence and explicit disposal differences.

### Local authored Solid client resources

The local Crepus compiler admits `createResource` and the `Resource` type from
`solid-js` in DOM source with a leading `/** @jsxImportSource solid-js */`
directive. Imports, aliases, reexports and TypeScript import queries use
Moonshine's independent `/solid` implementation. Each helper module needs its
own directive. The runtime is available in SDK 0.4.13; this compiler increment
is local and unpublished and still requires explicit native bootstrap.

Thirteen pinned Solid reference browser phases match loading, stale completion,
refresh, mutation, errors, recovery and disposal, while retaining input identity,
focus and selection. Both strict type checkers pass the source graph and reject
three invalid resource uses each. Six native integration tests and the existing
856-module source gate pass.

The minified acceptance application, including its driver, measures 22,259 bytes
(8,484 gzip) for Moonshine versus 19,826 (7,636 gzip) for Solid. These are whole
fixture sizes, not isolated runtime measurements; no speed improvement is claimed.
SSR rejects resource imports, including type-only forms. Async SSR, hydration,
Suspense and the full Solid resource option surface remain unsupported.

### Local imported Solid components

The local Crepus CLI now verifies relative default and named component imports,
local aliases and explicit named/default reexport chains before emitting the
module graph. Every component and barrel on that chain needs its own leading
`/** @jsxImportSource solid-js */` directive. The ultimate export must be a stable
synchronous function. Existing site boundaries, symlink checks and preflight
output protection remain in force. Standalone compilation stays fail-closed;
resolver-aware native compiler entry points provide the graph proof.

Thirteen pinned-reference browser phases match imported child setup, mount,
reactive props, connected cleanup, resource transitions, focus, selection and node
identity. Four fresh SSR requests across Bun and Node produce matching parsed
attributes/text; Moonshine additionally escapes `<` in quoted attributes and
`"` in text, so raw HTML differs. Both strict type checkers pass the DOM and SSR graphs.

The complete browser acceptance fixture is 23,012 minified bytes / 8,680 gzip,
versus Solid's 21,202 / 8,101. This includes the driver and makes no speed claim.
The compiler increment remains local and unpublished, using SDK 0.4.13's existing
independent renderer. Manual bootstrap is required. External or foreign
components, namespace/member tags, dynamic identities, component lookup through
`export *`, hydration and async SSR remain unsupported.
