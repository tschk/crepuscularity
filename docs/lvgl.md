# LVGL

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [Embedded / framebuffer](embedded.md)

`crepuscularity-lvgl` turns `.crepus` templates into LVGL Pro XML. Use it when the panel UI is owned by an LVGL project or STM32/LVGL workflow, and you want Crepuscularity to generate `component` or `screen` XML from the same template language used by GPUI, web, TUI, native IR, and embedded framebuffer targets.

## Install

```toml
[dependencies]
crepuscularity = "0.4"
crepuscularity-lvgl = "0.1.0"
```

Workspace examples use the umbrella crate:

```rust
let artifacts = crepuscularity::target::build_manifest_file_target(
    "crepus.toml",
    Some("dashboard"),
)?;
```

## Manifest target

Add an `lvgl` target to `crepus.toml`:

```toml
[[targets]]
type = "lvgl"
id = "dashboard"
path = "."
template = "ui.crepus"
out = "dist/dashboard.xml"
name = "Dashboard"
root = "component"

[targets.vars]
status = "ready"
cpu = 68
```

Build it with the shared target pipeline:

```bash
crepus build lvgl
crepus build --target dashboard
```

Manifest builds select the parser frontend from the template filename, including
Vue, Svelte, Astro, Angular, and JSX paths.

`root = "component"` emits a `<component name="...">` wrapper. `root = "screen"` emits a `<screen name="...">` wrapper for full-screen panel assets.

XML export rejects characters excluded by the [XML 1.0 character range](https://www.w3.org/TR/xml/#charsets), including NUL and forbidden control characters. This applies to rendered text, attributes and the export name. Valid Unicode remains intact; tabs, newlines and carriage returns use character references to preserve attribute whitespace.

Serialized attribute keys must also satisfy the [XML Name grammar](https://www.w3.org/TR/xml/#NT-Name). Invalid keys return an export error; valid custom keys retain the existing hyphen-to-underscore mapping. The component/screen export name is an attribute value and remains free text.

Bindings use the shared evaluator after the parser removes one pair of expression
braces. Extra braces, such as `value={{name}}`, remain part of the expression and
return an evaluation error rather than being silently removed.

## Hidden state

The source `hidden` attribute uses boolean presence semantics: evaluated false or
null emits `hidden="false"`; bare, empty, string `"false"` and other present values
emit `hidden="true"`. Use `hidden={false}` to reveal an object. Explicit attributes
retain their existing priority over class hints, and the last repeated attribute
wins. Normalizing typed source values avoids losing their meaning in XML string
conversion; the [LVGL 9.3 boolean converter](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/lv_xml_utils.c)
recognizes the explicit false spelling.

The `disabled` attribute uses the same presence rules: false, null or a missing
variable emits `disabled="false"`; bare, empty, string `"false"` and other present
values emit `disabled="true"`. Names are canonicalized so repeated case variants
retain the last binding. The [LVGL object parser](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/parsers/lv_xml_obj_parser.c) applies this value to the disabled state.

## Custom checked state

Direct `checked` on `checkbox` and `switch` follows native custom-control Boolean
coercion. Null or a missing variable is false. After text coercion and trimming,
empty/bare values and `true`, `1`, `yes`, `on` are true; all other spellings are
false. Matching is case-sensitive. This differs from HTML input presence rules.
Output uses explicit `checked="true"` or `checked="false"`; the last `checked` or
canonical `bind` attribute wins. Canonical `bind` still requires a Boolean snapshot.

## Template

```text
div #dashboard w-full h-full flex flex-col gap-3 bg-[#101820] p-4
  h1 text-white text-lg
    "LVGL Pro {status}"
  div flex flex-row gap-2
    span text-zinc-100
      "CPU"
    progress #cpu value={cpu}
  button #refresh bg-blue-500 text-white rounded @click="refresh"
    "Refresh"
```

The renderer maps common tags and style classes to LVGL XML attributes. It preserves `#id`, evaluates `if` / `for` / `match`, expands includes and slots, and converts text nodes into labels. Slot wrappers retain their IDs, styles,
bindings, and handlers. Supplied slot content uses the caller's variables and
include directory; wrapper attributes use the component context.
Supplied slot content borrows its caller during synchronous rendering, avoiding
a full parent-scope copy per repeated include. Local declarations can still require
copying their scope; explicitly owned `TemplateContext.slot` values remain supported.

Each public render keeps a private include cache. The first active lookup resolves
and reads a source; ordinary templates and named component files are parsed once
per mode and their ASTs are shared across occurrences. Captured slots with separate
virtual bundles retain separate caches. Props, defaults, caller slots and depth
checks still apply to every occurrence. Later lookups in the same render use that
source snapshot; the next public render starts fresh and sees edits or deleted
files. This reduces repeated generation work; XML generation remains synchronous.

Text inputs (`input` and `textarea`) emit LVGL `text` for both `value` and the
shared `bind` property used by Svelte form bindings. This matches the
[LVGL textarea XML API](https://lvgl.io/docs/pro/built_in_widgets/lv_textarea).
Aliases follow the existing last-binding-wins rule; a single extracted body-text
child retains its existing precedence. Sliders and bars retain their `value`
attribute. These are evaluated XML snapshots, not a live two-way binding runtime.

Canonical `bind` reads the current context: bare or quoted bindings name a path,
while expression results are used once without a second lookup. The shared AST
represents string-literal bindings as paths even when authored inside braces.
Supported snapshots are:

| Source tag | XML property | Snapshot type |
| --- | --- | --- |
| `input`, `textarea` | `text` | Text coercion, as with `value` |
| `checkbox`, `switch` | `checked` | Boolean |
| `select`, `dropdown` | `selected` | Nonnegative 32-bit integer index |
| `slider` | `value` | Signed 32-bit integer |

Wrong types, integer overflow and canonical `bind` on other tags produce errors.
The integer range follows the publicly available LVGL 9.3 XML converter's
[signed 32-bit contract](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/lv_xml_utils.c).
These snapshots do not create LVGL subjects or emit `bind_value`; the host owns
write-back and live updates. HTML input `type` variants are not converted into
other widget types; use the explicit control tags above.

For `slider`, `progress`, and `meter`, source `min`/`max` aliases emit
`min_value`/`max_value`, following the current [slider](https://lvgl.io/docs/pro/built_in_widgets/lv_slider)
and [bar](https://lvgl.io/docs/pro/built_in_widgets/lv_bar) XML APIs. Bounds must
resolve to representable signed 32-bit integers; finite integral numeric values
and quoted integer/zero-fraction decimal strings are normalized without rounding
fractional strings. Reversed bounds retain their source order.

Explicit steps use the same integer policy: only unit steps are accepted and
omitted. Other steps produce errors because this generator does not implement
quantization. Explicit target attributes such as `min_value`/`max_value` remain
pass-through values with existing last-binding precedence; other widget tags
retain their authored attributes. Older XML dialects can use different range
names, so use their explicit target attributes when necessary.

## Rust API

Use `crepuscularity_lvgl` directly when you already have template text and context:

```rust
use crepuscularity_core::context::TemplateContext;
use crepuscularity_lvgl::{
    render_template_to_lvgl_xml_with_options, LvglOptions, LvglRoot,
};

let xml = render_template_to_lvgl_xml_with_options(
    include_str!("ui.crepus"),
    &TemplateContext::new(),
    &LvglOptions {
        name: "Dashboard".into(),
        root: LvglRoot::Screen,
    },
)?;
```

For multi-component files, call `render_component_file_to_lvgl_xml(content, "ComponentName", &ctx)`.
Use `render_component_file_to_lvgl_xml_with_options` to select a screen root or
custom wrapper name. Manifest `name` and `root` options also apply when selecting
a named component; without a name override, the component name is retained.

XML generation borrows unchanged sibling scopes and reuses one scratch context
per loop, restoring row overrides between iterations. Declarations still clone
their local scope when needed. This does not change the generated XML contract.

## Target variables

LVGL rendering injects these target flags:

| Variable | Value |
| --- | --- |
| `crepus_target` | `"lvgl"` |
| `is_lvgl` | `true` |
| `is_embedded` | `true` |
| `is_tui` / `is_web` / `is_gui` | `false` |

Use them for target-specific template branches:

```text
if {is_lvgl}
  button #ack bg-blue-500 text-white rounded
    "Acknowledge"
```

Active `embed` islands are unsupported and produce a render error, including
islands in includes or supplied slots. Guard web-only islands with `if {is_web}`
and provide an LVGL fallback in the `else` branch.

## Examples

[`examples/advanced/lvgl-pro-mode/`](../examples/advanced/lvgl-pro-mode/) renders a dashboard component XML artifact from a host program.

[`examples/advanced/lvgl-stm32/`](../examples/advanced/lvgl-stm32/) uses `build.rs` to generate STM32-oriented LVGL XML at compile time.

## Relationship to embedded framebuffer

LVGL and embedded framebuffer mode are separate output paths:

| Target | Output | Use when |
| --- | --- | --- |
| `lvgl` | LVGL Pro XML | An LVGL project owns runtime rendering and panel integration |
| `embedded` | RGB565 framebuffer / PPM snapshot | Rust firmware or a host simulator pushes pixels directly |

Both targets can share `.crepus` source, variables, includes, and compile-time validation.
