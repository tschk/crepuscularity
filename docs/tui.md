# TUI Mode

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [Lite shell](lite.md)

The TUI backend renders `.crepus` templates into [Ratatui](https://ratatui.rs) frames. It is Ratatui-backed, but the UI surface can live in `.crepus` files so terminal apps can share the same indentation/JSX-like template language as the rest of Crepuscularity.

## Install

```toml
[dependencies]
crepuscularity-tui = "0.4.4"
ratatui = "0.29"
```

Use TUI when the terminal is the real app surface. Use [Crepuscularity Lite](lite.md) when the app is a GPUI desktop shell that embeds guest JavaScript/TypeScript and Rust-native plugins.

## Template Files

```crepus
div w-full h-full flex-col bg-zinc-950 text-white
  div h-[1] flex-row border-b #title
    "{title}"
  div flex-1 p-1 #messages
    "{messages}"
  div h-[1] flex-row border-t
    span w-[2]
      ">"
    input flex-1 #input
      "{input}"
```

Text, interpolated values, and raw output retain embedded line breaks. Interior
blank rows are preserved, and CRLF is treated as a single line break. Each row
keeps the inherited text style.

For text-only elements, `h-fit` measures logical rows before wrapping, while
`w-fit` measures the widest terminal display line. Fit sizing follows the
parent layout axis: height in columns and width in rows. Borders and padding
are included. Containers measure nested content on that same requested axis:
children and gaps add along a row or column, while the cross-axis extent is
the largest child. Flexible children within the fitted axis use their natural
sizes. Block titles also contribute to the measured extent.

Vertical overflow scrolling measures wrapped rows at the available width for
automatically sized text and flexible children, including nested containers.
`scroll-offset` selects the first visible row and `scroll-bottom=true` selects
the final viewport. Explicit child heights remain fixed. Borders and padding reduce the text width used for wrapping;
columns inside rows use their allocated widths. Scroll rendering allocates cell
buffers for the visible viewport rather than the entire content height.
Logical offsets and aggregate sibling heights/gaps can exceed 65,535 rows;
visible children paint in local terminal coordinates. An individual child subtree
still has a maximum measured height of 65,535 rows.

When a host buffer clips a render area horizontally, text wrapping, alignment,
titles and borders retain the original area's geometry. The renderer crops cells
from a buffer at the logical width and visible height; full-width rendering skips
this extra allocation.

Active web `embed` islands render an inline red diagnostic naming the asset.
Like include failures, this is visible frame content rather than a `draw` error
return. Guard web-only islands with `if is_web` and provide terminal content in
the other branch. Inactive branches and unused slot fallbacks remain unevaluated.

`class="hidden"` and active conditional/target-prefixed `hidden` classes remove
an entire subtree from painting, layout space and automatic focus discovery.
The `hidden` attribute follows HTML boolean presence: bare/empty and string
`"false"` hide; evaluated boolean false or null keep content visible. Context
updates restore rendering and focus eligibility. Hidden children, including
slot fallbacks and islands, are not evaluated.
Malformed active `hidden={expr}` bindings return the original expression error
before any cells are painted. A hidden ancestor or an already-resolved hidden
class skips that evaluation.
Active title and consumed scroll bindings also return expression errors before
painting. Numeric scroll parsing still falls back to zero; unconsumed scroll
bindings on non-scroll elements, empty boxes or horizontal pure text stay lazy.

`invisible` suppresses a subtree's text, borders and backgrounds while preserving
its normal size, fit measurements and layout gaps. Its descendants still evaluate
for layout. Invisible ancestry is excluded from automatic focus discovery;
conditional and target-prefixed visibility classes follow the current context.

## Runtime API

Use `template(path)` when the app owns the Ratatui draw loop.

```rust
let mut ui = crepuscularity_tui::template("ui/ui.crepus")?;
ui.set("title", "My App");
ui.set("messages", "hello");
ui.set("input", "input contents");

terminal.draw(|frame| {
    ui.draw_full(frame).expect("template renders");
})?;
```

Use `draw(&mut terminal, path, |ui| ...)` when Crepus should own a single draw pass.

```rust
crepuscularity_tui::draw(&mut terminal, "ui/ui.crepus", |ui| {
    ui.set("title", "My App");
    ui.set("input", "input contents");
})?;
```

`draw_if_changed` and `draw_full_if_changed` reuse cached cells until variables,
source, layout, virtual files, include base directory, or slot content changes.
Slot changes include the caller's context. `changed_keys` reports variable
names only. Use `HotTemplate` for filesystem edits;
conditional drawing does not watch files itself. Rotating `slot-rotate` phrases
also expire the cache at their next interval boundary. The host must continue
calling the drawing method; conditional drawing does not start a timer loop.
Rotating phrases must be plain literal text. Nested elements and interpolated
expressions return render errors without partially painting the frame.

Active text interpolation, raw-text/raw-HTML expressions, declarations, missing
component defaults, branch conditions, loop iterators, match selectors and include
properties, class interpolation and conditional classes propagate evaluator errors
before painting any cells. A supplied default
skips its expression. Missing variables retain their normal null/empty-text behavior;
inactive branches and hidden subtrees remain lazy. The renderer can be reused after
an error. Include properties evaluate in caller scope, while authored slots retain
the caller context. Missing or invalid include files keep their inline diagnostics.
Focus discovery remains best effort. Valid non-list loop values produce no rows;
loop rendering stops after a body error. Class expressions resolve before an
element's own hidden suppression and target-prefix filtering; a hidden parent skips
its descendants entirely. Conditional class values interpolate only when selected.
Other visual bindings retain their existing fallback behavior.

Each `Template` also caches its successfully parsed root AST for drawing and
focus discovery. Context and included resources remain live; changing the source
clears the AST cache. Cloning a template shares its current AST without tying
future source edits to the clone.

Supplied include slots borrow their caller scope during synchronous rendering and
focus discovery, avoiding a full context copy for every loop iteration. Public
owned `TemplateContext.slot` values remain supported. The manual allocation probe
`cargo test -p crepuscularity-tui --test slot_scaling -- --ignored --nocapture --test-threads=1`
checks forwarded-slot output/focus identity and reports cumulative requested
allocation bytes; it does not measure peak memory or frame latency.

## Macro Codegen API

`template_refs!` is the compile-time, jQuery-like API. It reads the `.crepus` file during compilation, finds every `#id`, and generates a typed handle for each valid Rust field name.

```rust
let mut ui = crepuscularity_tui::template_refs!("ui/ui.crepus")?;

ui.title.text("My App");
ui.input.content = "input contents".to_string();
ui.find("#input").expect("input handle").val("edited input");

terminal.draw(|frame| {
    ui.draw_full(frame).expect("template renders");
})?;
```

For a template element like `input #input "{input}"`, the macro generates `ui.input`. Before each `draw` or `draw_full`, handle content is synced into the template context under the generated Rust field name, so `{input}` renders the current value. IDs that contain hyphens are converted to underscores for field access, so `#message-list` becomes `ui.message_list`; selector lookup still accepts `ui.find("#message-list")`.

Rust cannot generate fields from a runtime string path. Use `template_refs!("ui/ui.crepus")` when you want generated handles, and use `template(path)` when the path is only known at runtime.

For Cargo rebuild tracking and early syntax validation, add a build dependency on the same crate and put this in `build.rs`:

```rust
fn main() {
    crepuscularity_tui::build::compile_crepus("ui").unwrap();
}
```

## Layout

Templates map onto Ratatui's `Rect` layout model.

| `.crepus` class | Terminal behavior |
|---|---|
| `flex-col` | children stacked vertically |
| `flex` / `flex-row` | children stacked horizontally |
| `flex-1` | fill remaining space |
| `w-[20]` / `h-[3]` | exact terminal columns or rows |
| `w-1/2` | percentage split |
| `w-full` / `h-full` | full available area |
| `gap-1` | one cell between children |

## Styling

The backend supports Tailwind-style terminal classes for foreground/background color, bold, italic, underline, dim text, borders, padding, and exact sizing. Examples include `text-white`, `bg-zinc-950`, `text-green-400`, `font-bold`, `border`, `border-t`, `border-b`, `px-2`, and `h-[1]`.

## Terminal events

`Template::handle_event` forwards terminal events to the focused element's
callback and bubbles unhandled events through registered parents. Bracketed
pastes arrive as `Event::Paste(String)`, preserving Unicode, line breaks, tabs,
and other payload characters without synthesizing key presses or moving focus.
Exhaustive matches on `Event` must include the paste variant.

Calling `Template::focus_mut()` switches to a caller-managed focus list, including
an intentionally empty list. Events and source reloads preserve that list until
`Template::refresh_focus()` resumes automatic discovery.

The public `collect_focusable_ids` helpers collect syntactic branches without
evaluating conditions or dynamic classes. Static classes use exact `focusable`
tokens and honor TUI/current-platform prefixes; `not-focusable` is not a marker.

The host event loop owns terminal modes: enable crossterm's
`EnableBracketedPaste` while accepting pastes and restore it with
`DisableBracketedPaste` when leaving that mode. The backend forwards the text;
the app decides how to insert or interpret it.

## CLI preview reloads

`crepus tui preview app.crepus` reloads template files and sibling `context.toml`
data on saves; `r` forces a reload. File variables refresh before drawing, including
removed keys and deleted context files. Failed template reloads keep the previous
source and variables. Include paths, virtual files and slot context are preserved.

This behavior belongs to the CLI preview. `HotTemplate` preserves caller-managed
context when reloading source. Terminal preview reads and renders remain synchronous.

## Current Scope

The crate is a rendering backend and template binding layer. It supports template evaluation, includes, slots, control flow, style mapping, file-backed rendering, and compile-time handle generation. `HotTemplate` provides optional file watching; hosts own terminal event loops, input handling, and app state.
