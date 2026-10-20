# View IR contract

**Also:** [Polyglot plugins](polyglot.md) · [Plugin surface](plugin-surface.md) · [Native shells](native.md)

This document is the stable cross-language boundary for Crepuscularity. Rust (`crepuscularity-native`) is the authoritative compiler; hosts consume **View IR JSON** only.

## Version

- Field: `ViewIr.version` (camelCase in JSON).
- Must equal [`IR_VERSION`](https://docs.rs/crepuscularity-native/latest/crepuscularity_native/constant.IR_VERSION.html) from `crepuscularity-native` (currently **7**).
- Breaking IR shape changes bump `IR_VERSION` and require plugin type updates.

## Document shape

Root object:

```json
{
  "version": 7, /* always IR_VERSION; see above */
  "root": [ /* ViewNode[] */ ]
}
```

Node kinds include `stack`, `text`, `button`, `input`, `picker`, `image`, `link`, and others defined in [`crates/crepuscularity-native/src/ir.rs`](../crates/crepuscularity-native/src/ir.rs). Style hints use camelCase keys aligned with Swift/Kotlin decoders in `examples/native-shells/` and CLI scaffolds.

Image nodes may carry optional `srcBind`, and links optional `hrefBind`, alongside
their existing `src`/`href` snapshots. The binding is an authored DSL expression,
resolved against the current host scope and lexical loop item. Framework emitters
use their existing safe expression translation and text coercion; missing/null
values become blank. Literal URLs and legacy JSON without these fields keep their
snapshot behavior. Expression-only URL changes participate in IR mutation diffing.
Rust callers constructing `ViewNode::Image`/`Link` explicitly must supply the new
optional `src_bind`/`href_bind` field (`None` preserves static construction).
Declarations, includes and named-component defaults retain captured URL snapshots
until the IR can represent those lexical environments. This does not make `alt`,
`placeholder`, `target`, `rel`, or WebView URLs reactive.
Generated SwiftUI/Compose links use their existing model evaluator; images remain
text placeholders. Legacy static demo shells do not gain a binding runtime.

Text nodes contain literal text, not raw HTML. Lowering rejects raw HTML such as
Svelte `{@html}` and Vue `v-html`, including in flattened control labels and
metadata, rather than escaping or dropping the markup. Vue `v-text` remains text.
Indentation text expressions (`{expression}`) also supply flattened label text;
button labels retain the expression in `labelParts` for reactive web emission.
Nested expressions remain the primary label when initially empty or missing;
later static siblings do not replace them. Purely static empty wrappers still
allow the following text to supply the primary label.
An explicit label overrides child expressions only when its evaluated value is
nonempty after trimming. Empty labels retain the child expression selected for display.

Embedded browser islands require the web site build. View IR only accepts
`embed` with `adapter="webview"`, which produces a WebView node from its source
URL. Other adapters, and all embeds inside flattened labels or metadata, fail
explicitly rather than disappearing.

Optional `style.id` is emitted as the element's literal HTML ID by all web
component targets, independently of `style.classes`. Source ID expressions are
evaluated during IR rendering; this field does not retain a reactive binding.
For controls rendered inside a label wrapper (checkboxes, DOM/SSR switches,
and supported file pickers), the ID belongs to the focusable input. Layout
classes remain on the wrapper. External labels therefore target the control.

Optional `style.htmlFor` preserves a label's `for` target (source `htmlFor` is
also accepted). It is emitted only with `style.htmlTag: "label"`: React uses
`htmlFor`, and the other web targets use `for`. Like IDs, target expressions are
evaluated during IR rendering. Duplicate `for`/`htmlFor` attributes are errors.
Native hosts ignore this metadata; older IR can omit it.

Optional `style.htmlDisplayContents: true` marks the compiler-generated layout
wrapper inside a scroll node. Web emitters use inline `display: contents` so the
original element's flex/grid layout, spacing and alignment reach its children
without an extra CSS utility. Native hosts ignore the flag and retain the inner
stack's axis, spacing and alignment. Older IR can omit it.

A scroll node's `axis` describes viewport scrolling independently of its inner
stack's layout axis. A single explicit `overflow-x-auto`/`overflow-x-scroll` utility
selects horizontal scrolling; the corresponding `overflow-y-*` utilities select
vertical scrolling. Without a single explicit direction, `overflow-auto`,
`overflow-scroll`, or paired x/y utilities retain the content-layout-axis fallback.
View IR represents one scroll axis, not simultaneous two-axis scrolling.

Optional `style.ariaLabel` preserves an explicit source `aria-label`. Web targets
put it on the interactive control, including inputs inside styled wrappers;
Tabs use it on their tablist. It overrides generated Slider/Progress/Meter names
and the default tablist name. An explicit empty value remains empty (Svelte may
warn about that value); omission retains existing fallback names. Values are
render-time snapshots like IDs, and duplicate source attributes are errors.
Native code generation ignores this web metadata.

Web form fields retain optional `style.formName`, `style.inputType`, and
`style.required` metadata. Explicit names, including `""`, override existing
binding-derived names. Names and required state belong to nested inputs rather
than checkbox/file-picker styling wrappers. Source `required={false}` and
`required={null}` omit the attribute; other supplied values preserve its presence,
including `required="false"`. Sliders accept names but reject `required`.
Source `disabled` and `readonly`/`readOnly` attributes are rejected until View IR
and its hosts can preserve their control state. This includes false-valued
attributes and flattened picker/tab metadata; they are never silently discarded.
The separate authored web-module compilers do not use this View IR restriction.
Consumed scalar attributes and raw `bind` paths reject duplicate bindings rather
than choosing the first value. This also applies to picker-option and tab metadata.

Source buttons accept an omitted `type` or explicit `type="button"`. Other
explicit types, including submit and reset, fail at View IR lowering because the
button node cannot preserve form submission/reset behavior. Duplicate `type`
attributes also fail. This does not restrict the separate authored-module path.

Text-compatible input types are `text`, `search`, `tel`, `url`, `email`,
`password`, `number`, `date`, `datetime-local`, `month`, `time`, `week`, `color`,
and `hidden`. Source types are trimmed and case-insensitive; empty/null types
default to text. Secure inputs always use password masking. Textareas ignore
type hints, and secure multiline inputs remain errors. Supplied IR hints are
allowlisted, with unsupported hints falling back to text. Structural source
`input` types such as `radio`, `range`, and `submit` fail explicitly;
use the existing slider control where supported. Standard `input type="file"`
lowers to FilePicker, with `change` as its selection callback and HTML presence
semantics for `multiple`. Source `value` and `bind`, including empty values, and
`click`, `input`, or `pick` handlers are unsupported on this alias and fail explicitly.
Custom file-picker aliases retain their existing `pick`/`click` callback and
multiple-value/class conventions.

These attributes are render-time snapshots and do not add native validation or
keyboard behavior. Duplicate source names, types, and required attributes are
errors. Custom text-field aliases retain their existing fallback behavior for
unsupported types. Host forms can validate and serialize generated controls;
source form/submission lowering remains incomplete.

Optional `style.formValue` preserves source `value` snapshots on text inputs,
textareas, and checkboxes. Unbound text controls initialize from it, retain edits
through unrelated host rerenders, and use it as their native form-reset default.
A nonblank `bind` takes precedence, including empty or missing host values.
Checkboxes use it as their submitted value, independently of checked state; the
value belongs to the input rather than its styling wrapper. Unbound generated web
checkboxes retain their initial checked state as the native form-reset default.
Bound checkbox reset behavior still differs between hosts; model synchronization
on form reset remains a separate concern.

For standard `input type="checkbox"`, a supplied `checked` attribute enables the
initial state unless its evaluated value is typed false or null. Literal values
such as `checked="false"` and `checked="checked"` preserve HTML presence semantics;
an absent attribute starts unchecked. Custom `checkbox`, `toggle`, and `switch`
controls retain their existing boolean coercion.

Values are escaped literal data, including braces, quotes, newlines, and HTML
entity syntax. Explicit empty values are retained; duplicate source values fail.
Textarea and multiline aliases also snapshot direct text and expression children
as their initial value. Nonempty child content combined with an explicit `value`
is an error, as are nontext children. Existing frontend whitespace normalization
still applies. Native code generation ignores this web metadata; initial values
on other control kinds are not covered by this field.

## Semantic HTML tags

Optional `style.htmlTag` retains paragraph, heading, label/caption, and common
inline text tags (`strong`, `em`, `code`, and related text-formatting elements).
All generated web targets use the hint for text nodes and outer containers,
including text with nested markup. The hint does not alter native code generation.
Old IR without it keeps the existing span/div defaults, and the IR version stays 7.
Web emitters allowlist these text tags; invalid or executable tags supplied in IR
fall back to the normal span/div element. This field preserves tag identity only;
it does not add support for arbitrary source HTML attributes or form semantics.
Markup frontends retain horizontal spaces at nested text boundaries and between
inline tags. Formatting-only newline indentation remains trimmed.

## Button label parts

`button.label` remains the evaluated snapshot for older consumers. Optional
`labelParts` preserve child-text labels containing expressions:

```json
"labelParts": [
  { "kind": "literal", "value": "Open " },
  { "kind": "expr", "value": "row.label" }
]
```

Mixed text inside includes, named component entries, or after local declarations
retains its evaluated snapshot until the IR preserves lexical scope provenance.

Plain `text.content` remains the evaluated snapshot. A single-expression
`text.bind` retains its existing expression; mixed text uses ordinary DSL string
concatenation, for example `"" + "Hello " + (count)`. The leading empty string
forces display conversion rather than numeric addition. Compiler-generated
bindings use quoted literal operands and parenthesized expression operands; the
core `text_binding` helpers encode and decode this form without changing the IR
schema. Web emitters apply display conversion to each expression separately.
Native code generation and Flutter reuse their constrained per-part text paths;
unsupported operands retain the whole snapshot, as with button label parts.

Web, native bridge, and Flutter text/label display converts booleans to
`true`/`false`, lists to `[N items]`, and object values to empty text. Null and
missing expressions display empty text; Flutter retains its existing snapshot
fallback for null/missing bound text. Literal text is preserved. Numeric
conversion retains each host's number formatting. Collection transport and
condition comparisons keep their separate value representations.

View IR pickers represent single selection. Enabled `multiple` on a standard
`select` or custom `picker` is a lowering error rather than silently changing the
selection mode. Standard `select` follows HTML presence rules: even
`multiple="false"` enables the mode; typed false/null or omission leaves it off.
Custom pickers retain their existing boolean coercion, including quoted false.
Option groups and other unsupported child elements fail rather than dropping
options. Enabled option `selected` defaults also fail; supply initial selection
through the picker's bound value instead. Typed false/null `selected` values and
omission remain valid. Options retain HTML presence rules, so quoted
`selected="false"` still enables the unsupported default.

Solid, Vue, and Imba pickers serialize the first matching option as selected for
valid bound values, preserving the initial selection in server-rendered HTML.
Unmatched values clear selection on the client, while parsed server HTML retains
the browser default first option.

Imba client pickers apply their bound value after options are rendered. Missing or null
values use the empty string; unmatched values select no option, and duplicate
values select the first match. An unchanged normalized bound value preserves user
selection on unrelated rerenders. Unbound pickers retain browser defaults and
user selection. Change callbacks receive the event; host code owns model updates.

React/Moonshine, Svelte, Vue, Solid, and Imba tabs read their `bind` in the current
lexical scope. The first tab whose value matches is selected; only its panel is
visible. Missing or unmatched bound values select no panel, while the first tab remains
keyboard reachable. Inactive panels stay mounted with `hidden` set.
Arrow Left/Right (wrapping), Home, and End focus and activate tabs through the
existing value callback; Enter and Space use native button activation. Callbacks
remain responsible for updating host selection. During client rendering, refs/actions or Imba render hooks
assign stable, instance-unique tab/panel IDs and their ARIA relationships; these
IDs are not serialized by server rendering.

Without a `bind` (including a whitespace-only bind), tabs select the first panel
initially and manage selection locally on clicks and keyboard activation. Local
selection survives unrelated host rerenders and stays separate for nested groups
and component instances. Optional change callbacks still receive the selected
value once per activation. Remounting a group resets its local selection.

Generated React/Moonshine, Svelte, Vue, Solid, and Imba sliders read bound
numeric values from the current lexical scope and clamp them to normalized
bounds. Missing, nonnumeric, or nonfinite values use the normalized snapshot.
Optional labels become accessible names. Bounds and steps use the same validation
as native code generation; absent or invalid steps use continuous HTML ranges
(`step="any"`). Change callbacks receive the host event and retain responsibility
for updating the model. React sliders without a bind retain uncontrolled input
behavior through `defaultValue`. Unbound Svelte sliders also preserve their
normalized initial value as the native form-reset default. Svelte installs range
bounds and step before initializing values, including ranges outside 0–100.

Numeric and date/time inputs preserve authored `min`, `max`, and `step` as
optional string metadata (`formMin`, `formMax`, `formStep`) in `ViewStyle`.
All web emitters pass these to the browser, including temporal strings and
`step="any"`; the browser interprets constraint values for the input type.
Lowering rejects these constraints on text, password, and multiline inputs.
These are web validation hints; native code generation and Flutter do not enforce
them. Older IR omits the fields and retains unconstrained behavior.

Text inputs and textareas have independent optional `onInput` and `onChange`
actions. In generated web components, `input` fires during editing and `change`
fires when the browser commits the edit (normally on blur). Both forward the
native event, including `event.target.value`, to the named host callback.
Callbacks update the host model; fields without a binding remain editable.
React uses a cleanup-aware native change ref and synchronizes bound values on
host renders, avoiding React's per-edit synthetic `onChange` behavior.
Server-rendered HTML contains initial values, without live event listeners.
Imba escapes initial textarea text and preserves leading newlines in SSR output.
Bound Imba textareas retain an empty form-reset default on client mounts; parsed
SSR HTML uses its serialized text as the default. Host code remains responsible
for synchronizing bound models on form reset.

SwiftUI/Compose code generation preserves its existing edit-setter semantics:
one model binding write, then any declared change and input actions. Native
`onChange` does not represent a separate browser-style commit. Older IR omits
`onInput`; mutation comparison includes the action when present. Other native
hosts need their own event handling before they can support this new field.

Generated React/Moonshine, Svelte, Vue, Solid, and Imba progress and meter
elements retain optional labels as accessible names. Progress values clamp to a
finite positive total; invalid totals render empty over 0–1. Meters preserve
valid bounds and clamp finite values, using the minimum for nonfinite values.
Invalid bounds render empty over 0–1. Equal-bound meters use a 0–1 range and
render full only when the finite value reaches the bound, matching native code
generation. Range attributes are installed before values to avoid premature DOM
sanitization.

Generated Svelte static text escapes template braces; literal labels never become
Svelte expressions. Svelte and Vue emit whitespace-sensitive static text and
labels as string expressions so compilation preserves leading/trailing spaces,
repeated spaces, tabs, and newlines in DOM text. Intentional bound text and button
label expressions remain reactive. CSS white-space rules still control visual
layout.

Generated web event callbacks resolve only explicitly owned entries in `handlers`.
Inherited names are ignored; explicitly registered names retain their original
record as the callback receiver.

Generated Vue components give optional `scope` and `handlers` props independent
empty-object defaults. Omitting or removing these props is safe for text,
conditions, loops, and event dispatch.

Named TSX components retain their requested named/default exports, props type,
and function name even when the name overlaps a runtime binding such as
`Array`, `globalThis`, `Fragment`, `For`, or `Show`.

Generated web property reads use optional chaining, including text, conditions,
and loop collections, so missing intermediate data does not throw during rendering.
Indexed collection paths such as `rows.0.children` use quoted bracket access;
keys retain their exact spelling, including leading zeroes.

The DSL null spellings `null`, `none`, `None`, and `Null` are literals,
including when host data contains fields with those names.

Literal parts are never interpolated again. Expression parts resolve in the
current host/loop scope. Generated framework components and SwiftUI/Compose
generators use these parts. Native SwiftUI/Compose labels resolve paths and numeric, quoted string, boolean, and null
literals; labels containing other expressions retain the whole snapshot.
Native SwiftUI/Compose loops retain ancestor aliases inside nested bodies. The
nearest loop wins when an alias is shadowed; bound controls write the corresponding
indexed model path. Bare fields still resolve against the innermost item, then
root state. Missing members of an explicit alias resolve to null without falling
through to a same-named root object. Supported compound conditions carry the
visible aliases and innermost item through the reserved `@crepus:lexical` scope
transport. Regenerated views using this transport require the current Rust actions
runtime; the public C ABI signatures are unchanged. Loop paths remain positional.

Authored binding paths ignore surrounding whitespace after unquoting. SwiftUI and
Compose generators also trim path edges in manually supplied IR. Whitespace-only
control bindings behave as unbound controls. Internal path characters and keys in raw
result payloads remain unchanged.

Flutter's IR renderer supports literal/path operands and retains the snapshot
for unsupported compound expressions. Native, Flutter, and generated web paths support string
`len`/`length`, `upper`, `lower`, `trim`, `is_empty`/`empty`, plus list lengths,
empty checks, and numeric indices. Map keys take precedence over computed
properties, and computed results can be chained. String lengths count Unicode
scalar values. Web emitters generate a property helper only when needed; object
fields with these names keep their ordinary values. Generated web numeric properties
`abs`, `floor`, `ceil`, and `round` use JavaScript numbers; `round` resolves ties
away from zero. Rounding rejects nonfinite inputs and results outside the core
signed 64-bit range. JavaScript does not retain the core integer/float distinction
or exact integers beyond its safe-integer range. Web casing follows the host
JavaScript engine's Unicode version. Flutter casing uses generated Unicode 17 mappings and the core's
contextual Greek final-sigma rule, including expansions such as `ß` to `SS`.
Regenerate the pinned tables when the core's Rust Unicode version changes:
from `plugins/crepuscularity-flutter`, compile `tool/generate_unicode_case.rs`
with `rustc -O`, then redirect its output to `lib/src/unicode_case_data.dart`. Label
parts count toward Flutter's node limit, without increasing structural depth.

Omitted or empty parts retain the snapshot. Old JSON still decodes, and static
labels omit the field. Rust callers constructing `ViewNode::Button` must now
initialize `label_parts` (usually `Vec::new()`). This additive JSON field does
not change `IR_VERSION`.

Flutter's pure-Dart source parser also preserves parts for inline quoted labels
and direct indented quoted children. Label attributes still produce snapshots.
Legacy shells and static plugin renderers continue to use `label`; supporting
reactive labels there requires adapter work. Nested child selection retains the
existing primary-text rules rather than flattening all button children.

Included component-local bindings still lack lexical scope in flattened IR.
Dynamic label parts can therefore resolve a component prop against root scope
instead. This remains an open correctness issue.

## JSON Schema

Generate schema for codegen (Swift, Kotlin, TypeScript, etc.):

```bash
cargo run -p crepuscularity-native --features schema --bin export-view-ir-schema
cargo run -p crepuscularity-native --features schema --bin export-view-ir-schema -- -o view-ir.schema.json
```

Plugins should pin a schema revision or `IR_VERSION` in their package metadata.

## Compile entrypoint (CLI)

Primary integration: subprocess **`crepus native ir`**. No Rust embedding required.

| Mode | Invocation |
| --- | --- |
| File | `crepus native ir <path.crepus> [--component Name] [--ctx FILE] [--var k=v] [--pretty]` |
| Stdin template | `crepus native ir --stdin --base-dir DIR` |
| JSON envelope | `crepus native ir --stdin-json` |

**Success:** compact or pretty JSON on **stdout** only.

**Failure:** `{"error":"..."}` on **stderr**, exit code non-zero.

### `--stdin-json` envelope

```json
{
  "entry": "main.crepus",
  "files": { "main.crepus": "motion.div\n  \"Hi\"" },
  "template": "div\n  \"Hi\"",
  "component": "Card",
  "context": { "name": "Ada", "items": [{ "label": "a" }] },
  "baseDir": "/path/to/project",
  "pretty": false
}
```

When both `files`+`entry` and `template` are present, **files+entry** wins. Context values are JSON scalars or arrays of objects (for `for` loops). Nested objects in context are rejected.

## Optional in-process ABI

[`crepuscularity-abi`](../crates/crepuscularity-abi/) exposes the same compile/session model via C (`include/crepuscularity_abi.h`): template or virtual files, context JSON, `crepus_session_render_ir_json`, event dispatch. This crate does **not** depend on Equilibrium; language packages wrap the C API with their own FFI tooling.

## Hot reload envelope

When a plugin supports live template updates, transport messages from `crepuscularity_native::hot_reload`:

- `HotReloadEnvelope { sequence, message }`
- `message.kind`: `noop`, `patch`, `fullReload`, `error`, plus dev-only variants for Aurorality

Patch payloads use `IrMutation` paths from `crepuscularity_native::mutations`. Plugins
may apply patches or fall back to full `ViewIr` replacement. `apply_mutations` applies
operations in order against a working copy and commits only on success; an invalid
operation leaves the original document unchanged. Nonempty patches temporarily clone the
document. Diffing an unchanged tree produces no operations. A text update within a stack
omits unchanged siblings from the patch, allowing consumers to retain their mounted
state.

Patches do not carry a document version. Use full-document replacement for version
changes; `diff_ir` returns a root replacement in that case, but applying it retains
the receiving document’s version. The source hot-reload planner lowers both trees
at the current version. Conditional and loop changes replace the whole node; patch
paths do not index into branch or loop-body arrays.

## Equilibrium

[Equilibrium](https://github.com/tschk/equilibrium) is **not** part of this contract. Plugin maintainers may use it in separate repos to compile/load their own native glue.

View IR event fields currently store one bare handler name per event. Lowering
rejects modifiers and multiple handlers for the same event instead of silently
losing their semantics. This also applies to `parseTemplate` in the WASM package;
HTML rendering uses a separate event contract.

Custom file-picker `pick` and `click` events are aliases for the same `onPick` field.
Use one of them; declaring both is a lowering error.

`@longpress=handler` and JSX `onLongPress={handler}` populate `onLongPress` on
stack, scroll, button, toggle, image, and list-item nodes. Declaring both aliases or
attaching long press to a node without that field is a lowering error. No IR
schema version change is needed: the optional scroll field defaults to absent for older IR.
Direct IR diffs replace nodes when a long-press action is added, changed, or
removed. Source hot reload retains its conservative full-reload gate for event
handler changes.

SwiftUI code generation emits a high-priority `LongPressGesture` for these
fields and dispatches through `CrepusActions.perform`. Existing button click
actions remain in the generated source. Compose code generation emits a pointer
handler and an accessible semantics long-click action for the same fields.
The pointer handler leaves ordinary taps available to the underlying control
and consumes pointer events after recognizing a hold. Containers ignore a child's
consumed gesture; native buttons and toggles can recognize their own consumed
down events. Images retain the existing
text-placeholder rendering; long press does not add image loading.

Generated inputs, sliders, toggles, and checkboxes dispatch a declared `@change` action
without requiring `bind`. An unbound control dispatches the action name through
`CrepusActions.perform`; it retains its declared value and does not persist the
new value. Unbound inputs display empty text and do not read the root model.
Use `bind` for state updates and value-bearing change dispatch.

SwiftUI and Compose `CrepusActions.perform` submit actions to one serial worker
lane in submission order. Each request captures its dispatch callback and result
sink; the worker waits for main-thread sink delivery before starting the next
queued action, so that action observes the previous sink's committed state.
Callbacks must return without synchronously waiting for another queued action.
The default host
router presents native UI on the main thread before dispatching Rust work on the
worker. Custom dispatch callbacks run on the worker and must route their own UI
operations to the main thread.

Generated bound controls use the same lane through `performChange`. Submission
snapshots the edited JSON value and records a local draft before returning;
handler dispatch runs on the worker. Text fields, toggles, checkboxes, sliders,
pickers, and tabs read their draft while it is pending. Each binding has a
sequence number, so an older result cannot clear a newer edit. Main-thread sink
delivery precedes clearing the matching draft: a committed handler transform
becomes visible, while rejection restores the last committed value. Custom
sinks retain responsibility for applying results. Bind-only queued changes
prepare a patch without writing view state before sink delivery; the explicit
synchronous Rust change APIs retain their existing immediate bind-only behavior.

Draft keys use resolved model paths, including positional loop indexes. They
do not establish stable row identity across list reordering or removal. General
labels, conditions, and list expressions continue to read committed model state.
Values are validated and copied at submission; invalid JSON values do not leave
a pending draft.

This lane limits concurrent action execution, not queue length. Result sinks
still process JSON and update view state on the UI thread. Result commits and
diagnostic reads use a separate metadata lock, so they do not wait for a running
handler's action-state lock. Single-field commits update an object model without
copying unrelated state; multi-field commits still copy state for atomic rollback.
Queued actions are not cancelled when a view disappears. These changes do not
provide a frame-time guarantee.

The native Rust scaffold wraps named bound changes as
`{"action":"handler","payload":{"binding":value}}`. Binding names such as
`action`, `kind`, and `payload` remain data and do not alter action routing.
Successful named responses include the edited binding in their `value` object
unless the handler supplies that field itself; the result sink can then update
view state. Failed actions do not echo a binding update.

Successful plugin changes carry edited state in a separate `bindings` object.
The native result sink applies those fields after the existing `value` fields,
so a binding named `capability`, `method`, or `value` does not replace plugin
response metadata. Ordinary plugin action responses retain their existing shape.

Default native action routers accept dotted plugin names such as `device.info`
alongside explicit plugin JSON requests. Generated controls preserve the original
handler string for custom dispatch callbacks. Named actions such as `sync`
retain their existing route.

Lowering rejects bare handlers that the resulting node cannot represent, such
as `keydown` on a button or `click` on a stack. Supported bare events map to
`onClick`, `onChange`, `onPick`, `onDrop`, or `onLongPress`; this validation does
not change the IR schema.

Handlers in metadata-only children are also lowering errors. This includes
nested control labels, picker options, and tab/page wrappers. Interactive
content inside a tab/page still lowers normally; place selection handlers on
the parent picker or tabs node.
Includes are unsupported in metadata-only children; use them inside rendered
content instead of flattened labels or options.
Conditionals, loops, and matches also fail in flattened labels or metadata:
these fields cannot retain branching or repeated content. Ordinary layout
children and content inside an actual tab/page retain control-flow nodes.

`input` with an evaluated `type` of `checkbox` lowers to the existing checkbox
node, including `checked`, `bind`, and `onChange`. Evaluated password types keep
secure input rendering. Type matching is case-insensitive; custom `checkbox`
and text-input tags retain their existing behavior.

Native numeric model reads, including present pending drafts, return zero for
non-numeric or non-finite values. A missing draft alone reads the committed fallback.
The Rust bridge preserves finite Double values; the Compose Float accessor also
returns zero if conversion would overflow its Float range.

Native slider initial values are clamped to their declared bounds, including
when the default zero falls outside that range. Generated SwiftUI and Compose
slider getters also clamp bound model values for presentation; they do not
rewrite the underlying model. Ranges whose width overflows Float are errors.
SwiftUI emits the declared slider `step`. Compose uses native discrete steps
when the interval divides the range evenly into 2–1000 intervals. This preserves
native snapping and accessibility semantics. Irregular intervals, endpoint-only
grids, and larger grids currently remain continuous on Compose; enforcing their
step semantics is still incomplete.

The native result sink records responses with `ok: false` for error reporting
but applies neither their `value` fields nor their `bindings` updates. Legacy
responses without an `ok` field retain their existing model-update behavior.
If a result supplies `ok`, it must be a boolean. Malformed status values are
recorded as protocol errors and neither update channel is applied.

Native binding updates traverse dotted object and array paths just like model
reads. Missing object containers are created; existing array indices are
required. Paths support at most 126 segments. Updates preserve sibling fields,
and an invalid path rejects the entire result update without partially changing
model state.

Generated native controls inside a `ForEach` resolve item-relative bindings to
indexed model paths for writes (for example, `row.enabled` becomes
`rows.0.enabled`). Nested loops extend the parent item path; bindings to root
model fields retain their original paths.

Compose text inputs set `singleLine` from the IR `multiline` flag: normal and
password inputs use one line; textareas and the `multiline` class permit
multiple lines. This controls presentation and keyboard behavior, without
rewriting model text.

Secure multiline template inputs are lowering errors: the SwiftUI adapter has
no secure multiline editor. Directly supplied IR with both flags uses a
`SecureField` on SwiftUI, preserving masking while omitting multiline editing.

Native text-input aliases (`input`, `textfield`, `textinput`, and `textarea`)
evaluate `type` expressions before checking for password mode. Matching is
case-insensitive. Checkbox-type conversion remains specific to `input`.

Generated native links activate their `href` through SwiftUI `Link` or Compose
`LocalUriHandler`. Labels and container styles are preserved. Empty destinations
render non-clickable content; SwiftUI also preserves content when its URL parser
rejects a destination. Compose opener failures report an `openUri` error through
the existing result sink. Browser `target` and `rel` metadata remain in the IR;
native navigation is controlled by the platform URL handler. Evaluated link
`target`/`rel` and badge `tone` metadata are trimmed; empty values are omitted,
and malformed expressions are lowering errors.

Native button, file-picker, and dropzone labels use a nonempty evaluated `label`
attribute, trimmed of
outer whitespace; otherwise they fall back to child text. Malformed label
expressions are lowering errors.

Standard `select` options without an explicit value use descendant text, excluding
script subtrees and template contents, with HTML ASCII whitespace stripped and
collapsed. Nonbreaking spaces, case, and punctuation are retained. A nonempty
`label` attribute controls the display label independently; otherwise the normalized text is displayed. Custom picker
aliases retain their slug-derived default values. Set an explicit `value` on
standard options to retain a previously slug-derived identifier. An explicitly
supplied option `value` that evaluates to null, including an unresolved
expression, becomes `value=""` under Crepus scalar-attribute coercion. Omitting
`value` uses text instead.

Picker option labels and tab/page values, labels, and icons evaluate context
expressions during lowering. Explicit empty selection values are preserved.
Malformed metadata expressions are errors rather than silently missing labels
or selection identifiers. File-picker aliases and dropzones also propagate
label and `accept` expression errors; evaluated accept filters remain in the IR.
Custom file-picker `multiple` evaluates its expression: false or null disables multiple
selection, while bare or empty boolean attributes enable it. Malformed
expressions are errors.

Generated SwiftUI and Compose file-picker nodes currently render action
buttons: `onPick` dispatches the named action, but the node's `accept` and
`multiple` settings do not configure a system picker. Scaffold host routes
`import_files` and, with the image-picker capability, `pick_media` can open
platform pickers; their configuration and results follow the host route.
Android document/media picker routes accept one active request at a time;
overlapping named or plugin requests return `picker already active` without
replacing the original action. Completion or launch failure releases the
active request. iOS document, media, camera, and video pickers share a delegate registry and
reject overlapping requests through an asynchronous error result. The original
delegate is retained until completion or cancellation. Media results are emitted
after the picker dismissal animation completes. Camera and video capture results
also wait for dismissal completion, allowing a result handler to open the next picker.

Dropzones remain presentation fallbacks rather than native drop handlers.
Android and iOS document-picker results, and iOS media-picker results, report
`ok: false`, an `error`, and `failedCount` when selected files fail to import
into the cache. Metadata for
successfully imported files remains in `value.files`; the default state sink
does not apply failed results. Cancellation is an empty successful file list.
Individual failed-file identities are not currently reported. Failed copy operations
attempt to remove their newly allocated cache destination before rethrowing
the original error; successful imports keep their cached files.

Bare or empty `checked` attributes also enable the initial state of custom
checkboxes, toggles, and switches; explicit false or null values disable it. Malformed
checked expressions are lowering errors. Bound controls continue to read their
state from the model.

Compose entrypoints apply the supplied `modifier` to the component root.
Root conditionals, loops, and multiple root nodes use a `Column` container;
nested conditionals and loops retain their parent layout flow. Template style
modifiers and long-press handling are composed with the supplied modifier.

Divider axis describes the separated layout: `row` (`flex-row`) emits a vertical
separator, while `column` (the default) emits a horizontal rule. Generated
SwiftUI and Compose vertical dividers default to 16 display points of height;
explicit nonnegative style height overrides that extent. A fill-height sentinel
uses the parent's available height, while fit and invalid negative heights use
the 16-point default. Maximum-height fill (`max-h-full` / `max-h-screen`) also
fills an otherwise unspecified divider; an explicit nonnegative or fit height
keeps its extent. SwiftUI fixes divider orientation with a matching stack so
it remains independent of the surrounding layout.

Padding hints must be finite and nonnegative. Invalid negative padding classes
are ignored without overwriting earlier valid hints; original class tokens
remain in the IR. Native generators also omit invalid padding supplied directly
through IR, retaining valid companion edges and normal button defaults.

Native margin classes include standard negative forms such as `-m-4` and
`-mx-2`, plus signed arbitrary values. Compose uses a layout modifier for margin
layers containing negatives: the reported slot shrinks, and content may paint
outside it. Loose zero minima do not force content to grow; nonzero parent
minima still constrain the result. All, axis, and edge layers remain additive.
`margin_left` and `margin_right` keep the existing start/end convention, including
RTL placement for zero-width slots. Intrinsic sizing includes the margin, and
extreme values are bounded by Compose's representable constraints and pixel
coordinates. Positive-only layers retain their existing padding output.

Generated SwiftUI and Compose omit nonfinite numeric style hints from directly
constructed IR. A missing transform axis uses zero for translation and one for
scale; a valid companion axis is retained. Compose also omits line height when
conversion to display units overflows. Finite style values and sizing sentinels
retain their existing behavior.

The `truncate` hint limits native text to one line, taking precedence over an
explicit line clamp. Compose line clamps use ellipses unless `text-clip` is
requested; SwiftUI retains its native trailing truncation behavior.

Line-clamp counts must be positive. Invalid classes preserve a preceding valid
limit; SwiftUI and Compose omit nonpositive limits supplied through direct IR.

Canonical `sans`, `serif`, and `mono` font-family hints map to the native
platform font designs/families while preserving explicit font size and weight.
Unrecognized family names retain the platform default.

Generated text and badge styles preserve finite letter-spacing hints, including
negative, zero, and fractional values, using SwiftUI tracking and Compose sp.

Generated font-size literals preserve fractional precision. Compose converts
line-height multipliers using the explicit font size, or the existing 16-point
fallback when no finite font size is supplied.

Compose emits floating-point dimension literals for finite values outside
Kotlin's integer range, including padding, borders, offsets, and spacing.
This keeps generated source compilable without changing the supplied values.

SwiftUI handles width and height independently: filling one axis retains a
fixed or content-fitting dimension on the other. The `-2` fit sentinel applies
`fixedSize` only to the corresponding axis. Maximum-width and maximum-height
fill sentinels map to infinite upper bounds in SwiftUI, matching Compose's
fill modifiers.

Generated sliders also sanitize directly constructed IR: invalid or overflowing
bounds use the default 0–100 range with an initial value of zero. Nonfinite
initial values use the minimum; finite values are clamped to the bounds.
Nonfinite or nonpositive steps, and steps wider than the range, are omitted.
Equal bounds
are retained without a step, and bound model values are clamped to the
normalized range.

Generated native progress controls clamp finite values to their total. Direct
IR with a nonpositive or nonfinite total renders empty with a safe total of one;
nonfinite values render empty. Valid small values and totals are retained.

Numeric slider, progress, and meter attributes propagate expression evaluation
errors. Successfully evaluated nonnumeric or nonfinite values retain the
existing default fallback. Meter ranges must have ordered bounds
and a finite width; equal bounds are allowed. Generated SwiftUI and Compose
meters use native progress controls with a clamped fraction relative to both
bounds. Equal-bound ranges display full when the value reaches the bound,
otherwise empty. Labels and container styles are retained.

Explicit spacer sizes must evaluate to finite, nonnegative numbers. Fractional
sizes are preserved in native source; an omitted size retains the existing
default or gap-class behavior.
