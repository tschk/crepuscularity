# LVGL

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [Embedded / framebuffer](embedded.md)

`crepuscularity-lvgl` turns `.crepus` templates into LVGL Pro XML. Use it when the panel UI is owned by an LVGL project or STM32/LVGL workflow, and you want Crepuscularity to generate `component` or `screen` XML from the same template language used by GPUI, web, TUI, native IR, and embedded framebuffer targets.

## Install

```toml
[dependencies]
crepuscularity = "0.5.0"
crepuscularity-lvgl = "0.2.0"
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

`root = "component"` emits a `<component name="...">` wrapper. `root = "screen"` emits a `<screen name="...">` wrapper for full-screen panel assets.

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

The renderer maps common tags and style classes to LVGL XML attributes. It preserves `#id`, evaluates `if` / `for` / `match`, expands includes and slots, and converts text nodes into labels.

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
