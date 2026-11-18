# Components

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [CLI](cli.md) · [Extensions](webext.md)

Crepuscularity supports reusable components via `include` statements.

## Single-File Components

One `.crepus` file = one component. Use `$: default` to declare optional props:

```text
# components/card.crepus
$: default subtitle = ""

div rounded-lg border border-zinc-700 p-4
  div font-bold
    {title}
  if {subtitle}
    div text-sm text-zinc-400
      {subtitle}
  slot
    div text-zinc-500 italic
      "No content provided"
```

Or in JSX syntax:

```jsx
// components/card.crepus
let-default subtitle = ""

<div class="rounded-lg border border-zinc-700 p-4">
  <div class="font-bold">{title}</div>
  <if condition={subtitle}>
    <div class="text-sm text-zinc-400">{subtitle}</div>
  </if>
  <slot>
    <div class="text-zinc-500 italic">No content provided</div>
  </slot>
</div>
```

Include from another template:

```text
include components/card.crepus title="Dashboard" subtitle="Overview"
  div p-4
    "Card body content"
```

Or using fragment syntax for multi-component files:

```text
include ui.crepus#Card title="Settings"
  div
    "Settings content"
```

In JSX syntax:

```jsx
<include src="components/card.crepus" title="Dashboard" subtitle="Overview">
  <div class="p-4">Card body content</div>
</include>

<include src="ui.crepus#Card" title="Settings">
  <div>Settings content</div>
</include>
```

## Multi-Component Files

Group related components in a single file with TOML frontmatter. Each component
name must be unique; duplicate definitions are parse errors.
Separate `---` from the component name with spaces or a tab. Section markers
inside multiline quoted text remain literal text.
An opening `+++` frontmatter block must have a closing `+++` delimiter on its
own line. A line containing exactly `+++` is reserved as the closing delimiter.
When provided, component `description` metadata must be a TOML string.

```text
+++
[Card]
description = "A bordered card"

[Card.defaults]
title = "Untitled"

[Button]
description = "A clickable button"

[Button.defaults]
label = "Click"
variant = "primary"
+++

--- Card
div rounded-lg border border-zinc-700 p-4
  div font-bold
    {title}
  slot

--- Button
$: default variant = "primary"
button px-4 py-2 rounded cursor-pointer
  {label}
```

Include with `#Name` fragment:

```text
include ui.crepus#Card title="Settings"
  div
    "Settings content"

include ui.crepus#Button label="Save" variant="primary"
```

## Slots

The `slot` keyword renders content passed by the parent:

```text
# Layout component
div flex flex-col min-h-screen
  header bg-zinc-900 p-4
    "Header"
  main flex-1
    slot
  footer bg-zinc-900 p-4
    "Footer"
```

Usage:

```text
include layout.crepus
  div p-8
    "Main content goes here"
```

### Fallback Content

Slot can have fallback children if nothing is passed:

```text
slot
  div text-gray-500
    "Default content"
```

## Component Context

Each component gets its own context with:

- Props passed via `prop=value` syntax
- Defaults from `$: default` or TOML `[Component.defaults]`
- The slot content (accessible via `slot` keyword)

TOML `[Component.defaults]` values support strings, signed 64-bit integers,
finite floats, and booleans. Arrays, tables, dates, and non-finite floats are
parse errors; the diagnostic names the component and property.
Declare defaults with `[Component.defaults]` or an inline `defaults = { ... }`
table; other types for the `defaults` field are parse errors.

Props always override defaults:

```text
# In component:
$: default color = "blue"

# In parent:
include component.crepus color="red"  # color is "red"
include component.crepus              # color is "blue"
```

## UI Library Components

Crepuscularity ships a small set of reusable UI components in [`examples/ui-library/`](examples/ui-library/). They are intentionally generic so they can be used across projects; see the `examples/ui-library/examples/` demos for sample usage.

### Available components

| Component | File | Props | Use case |
| --- | --- | --- | --- |
| `Grid` | `ui-library.crepus` | `columns`, `gap`, `aspectRatio` | Photo galleries, dashboards |
| `Panel` | `ui-library.crepus` | `title`, `collapsible`, `defaultExpanded` | Grouped settings, collapsible sections |
| `Wizard` | `ui-library.crepus` | `steps`, `currentStep`, `showProgress`, `allowSkip` | Pairing flows, onboarding |
| `List` | `ui-library.crepus` | `items` (`label`, `detail`), `selectable` | File lists, logs, settings |
| `Card` | `ui-library.crepus` | `title`, `subtitle`, slot | Dashboards, detail panels |
| `Badge` | `ui-library.crepus` | `label`, `color` | Status indicators |
| `Button` | `ui-library.crepus` | `label`, `variant` | Actions, forms |
| `Alert` | `ui-library.crepus` | `message`, `kind` | Feedback banners |
| `Tabs` | `ui-library.crepus` | `tabs`, `active` | Navigation, settings |
| `EmptyState` | `ui-library.crepus` | `message`, `action` | Empty lists, pages |
| `Avatar` | `ui-library.crepus` | `src`, `label`, `size` | User avatars, icons |
| `TransferProgress` | `transfer-progress.crepus` | `label`, `fileName`, `value`, `max`, `speed`, `eta` | Upload/download progress |

### Example usage

```text
include ui-library/ui-library.crepus#Grid columns=4 gap=4
  for photo in {photos}
    div aspect-square rounded-lg overflow-hidden
      img src={photo.thumb} object-cover w-full h-full
```

Components are documented by their TOML frontmatter and can be rendered with `crepus render` or any supported target.

## Runtime vs Compile-Time

The `include` statement and multi-component files are **runtime-only** features. The compile-time `view!` macro does not support includes.

For compile-time composition, use Rust functions or the GPUI component model directly.
