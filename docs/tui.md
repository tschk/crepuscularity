# TUI Mode

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [Lite shell](lite.md)

The TUI backend renders `.crepus` templates into [Ratatui](https://ratatui.rs) frames. It is Ratatui-backed, but the UI surface can live in `.crepus` files so terminal apps can share the same indentation/JSX-like template language as the rest of Crepuscularity.

## Install

```toml
[dependencies]
crepuscularity-tui = "0.5.0"
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

## Current Scope

The crate is a rendering backend and template binding layer. It supports template evaluation, includes, slots, control flow, style mapping, file-backed rendering, and compile-time handle generation. Terminal event loops, input handling, app state, and hot reload remain app-owned for now.
