# GPUI Integration

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md)

Crepuscularity integrates with [GPUI](https://gpui.rs) to build fast, native desktop applications. GPUI is a Rust UI framework designed for high-performance applications like text editors and IDEs.

## Quick Start

```bash
# Create a new GPUI app
crepus new my-gpui-app
cd my-gpui-app

# Build and run
SDKROOT=$(xcrun --show-sdk-path) cargo run
```

## macOS Xcode Environment

GPUI's macOS build compiles Metal shaders during Cargo builds. Cargo does not invent Xcode settings; it passes through the shell environment to GPUI's build script.

From this repo, use:

```bash
eval "$(scripts/metal-env.sh)"
cargo build -p crepuscularity-gpui
scripts/metal-env.sh -- cargo check -p crepuscularity-gpui
```

For an app outside this checkout, export the same values explicitly:

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
export DEVELOPER_DIR="$(xcode-select -p)"
export TOOLCHAINS="Metal"
export PATH="$(xcodebuild -showComponent MetalToolchain -json | plutil -extract toolchainSearchPath raw -o - -)/Metal.xctoolchain/usr/bin:$PATH"
```

`SDKROOT` makes SDK headers such as `dispatch.h` visible. `DEVELOPER_DIR` pins the active Xcode for `xcrun`. `TOOLCHAINS=Metal` is the short `xcrun` selector for the installed Metal Toolchain when GPUI runs `xcrun -sdk macosx metal` and `xcrun -sdk macosx metallib`. The `PATH` entry is for direct `metal` / `metallib` diagnostics; GPUI still goes through `xcrun`.

Check the local state without downloading anything:

```bash
scripts/metal-env.sh --check
```

If `xcrun_metal=failed` appears even though `path_metal=ok`, Xcode can see the downloaded compiler on disk but has not registered it for `xcrun`. Run `xcodebuild -downloadComponent MetalToolchain` or install the component from Xcode Settings > Components, then rerun the check before treating a Cargo/GPUI failure as a project issue.

## Project Structure

```
my-gpui-app/
├── Cargo.toml              # Dependencies and features
├── src/
│   ├── main.rs            # Application entry point
│   └── views/             # .crepus template files
│       └── ui.crepus
└── assets/                # Static assets (optional)
```

## Application Setup

```rust
use crepuscularity_gpui::prelude::*;
use gpui::{App, Application, WindowOptions};

struct MyApp;

impl MyApp {
    fn new(cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for MyApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            div w-full h-full bg-zinc-900 text-white flex items-center justify-center
                div text-4xl font-bold
                    "Hello, GPUI!"
        "#}
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(MyApp::new)
        });
    });
}
```

## Template Integration

Use the `view!` macro to embed `.crepus` templates directly in Rust code:

```rust
impl Render for MyComponent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            div flex flex-col gap-4 p-6
                div text-xl font-bold
                    "My Component"
                button px-4 py-2 bg-blue-500 text-white rounded @click=handle_click
                    "Click me"
        "#}
    }
}
```

Inline `view!` templates do not need a build script because Cargo already tracks the Rust source file. If a GPUI app also keeps reusable `.crepus` files on disk and loads them from Rust, use the same one-line build helper for Cargo rebuild tracking and early syntax validation:

```rust
fn main() {
    crepuscularity_gpui::build::compile_crepus("src/views").unwrap();
}
```

## Event Handling

Events are handled through GPUI's listener system:

```rust
impl MyComponent {
    fn handle_click(&mut self, _event: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        println!("Button clicked!");
        // Update state, trigger effects, etc.
    }
}

impl Render for MyComponent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            button @click=|this, event, window, cx| this.handle_click(event, window, cx)
                "Click me"
        "#}
    }
}
```

## State Management

GPUI uses reactive state management. Components can have fields that trigger re-renders:

```rust
#[derive(Clone)]
struct Counter {
    count: i32,
}

impl Counter {
    fn new(cx: &mut Context<Self>) -> Self {
        Self { count: 0 }
    }

    fn increment(&mut self, cx: &mut Context<Self>) {
        self.count += 1;
        cx.notify(); // Trigger re-render
    }
}

impl Render for Counter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            div flex items-center gap-4
                button px-4 py-2 bg-red-500 text-white rounded @click=increment
                    "-"
                div text-2xl font-bold
                    {format!("{}", self.count)}
                button px-4 py-2 bg-green-500 text-white rounded @click=increment
                    "+"
        "#}
    }
}
```

## Styling

Crepuscularity uses Tailwind-inspired utility classes that map to GPUI styling APIs:

```text
div w-full h-full bg-zinc-900 text-white p-6 rounded-lg shadow-lg
    div text-2xl font-bold mb-4
        "Title"
    div text-base leading-relaxed
        "Description text"
```

### Layout Classes

- `flex`, `flex-row`, `flex-col` - Flexbox layout
- `grid`, `grid-cols-3`, `grid-rows-2` - Grid layout
- `block`, `inline`, `inline-block` - Display modes
- `absolute`, `relative`, `fixed` - Positioning

### Spacing Classes

- `p-4`, `px-2`, `py-3` - Padding
- `m-4`, `mx-2`, `my-3` - Margin
- `gap-4`, `gap-x-2`, `gap-y-3` - Gaps

### Colors

- `bg-zinc-900`, `text-white`, `border-gray-300` - Background, text, and border colors
- GPUI color palette includes standard colors: `red`, `green`, `blue`, `yellow`, `black`, `white`, `transparent`

### Typography

- `text-xs`, `text-sm`, `text-base`, `text-lg`, `text-xl`, `text-2xl` - Font sizes
- `font-thin`, `font-normal`, `font-bold`, `font-black` - Font weights
- `leading-none`, `leading-tight`, `leading-normal`, `leading-relaxed` - Line heights

### Effects

- `rounded`, `rounded-lg`, `rounded-full` - Border radius
- `shadow`, `shadow-lg` - Box shadows
- `opacity-50`, `opacity-75` - Opacity

## Advanced Features

### Text Transforms and Letter Spacing

GPUI supports advanced text styling when using the vendored GPUI build:

```toml
[dependencies]
crepuscularity-gpui = { version = "0.4.1", features = ["gpui-text-extras"] }
```

```text
div uppercase tracking-wide
    "TRANSFORMED TEXT"

div lowercase tracking-tight
    "another example"

div capitalize
    "hello world"
```

### Custom Components

Create reusable components by implementing the `Render` trait:

```rust
struct Card {
    title: String,
    content: String,
}

impl Card {
    fn new(title: String, content: String, cx: &mut Context<Self>) -> Self {
        Self { title, content }
    }
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            div bg-white rounded-lg shadow p-6
                div text-xl font-bold mb-2
                    {self.title.clone()}
                div text-gray-600
                    {self.content.clone()}
        "#}
    }
}
```

### Asset Loading

GPUI supports loading images and other assets:

```rust
struct ImageView {
    image: gpui::SharedString,
}

impl Render for ImageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"
            img src={self.image.clone()} w-32 h-32 object-cover rounded
        "#}
    }
}
```

## Hot Reload

`crepus dev` is a rebuild-and-relaunch loop: it watches `src/` and `Cargo.toml`, then
runs `cargo build` and restarts the app. Templates compiled
with `view!`/`view_file!` only change through that rebuild. For in-process template hot
reload, render through `HotReloadView` (below) or use `crepus preview <file.crepus>`.

### Click handlers in hot-reloaded templates

Runtime-rendered templates name `@click` handlers; the host registers callbacks under
those names. Handlers are looked up on every render, so they keep working after the
template or its components reload:

```rust
use crepuscularity_runtime::{EventHandlers, HotReloadState, HotReloadView};

let handlers = EventHandlers::new()
    .on_click("increment", |_args, _window, _cx| { /* update app state */ })
    .on_click("add", |args, _window, _cx| { /* args[0] is the evaluated first argument */ });
let state = cx.new(|cx| HotReloadState::new(path, context, cx).with_event_handlers(handlers));
let view = HotReloadView::new(state);
```

- `@click=increment` calls `increment` with no arguments.
- `@click={add(5, item.id)}` evaluates each argument in the element's scope (loop items, component props).
- Components take handlers as string props: `include button.crepus action="increment"` with
  `button @click=action` inside resolves to `increment`, or `action="add(5)"` to `add` with `5`.
- Without a registry (plain `render_nodes`, `crepus preview`) clicks are intentionally
  inert, so previews render templates whose handlers live in the host app. With a registry
  installed, an unregistered name renders an in-window error instead of being dropped.
- Only `@click` runs in the runtime path. Other events (`@keydown`, `@keyup`, `@hover`,
  `@mousedown`, …) are still ignored there and need the compile-time `view!` macro.

## Performance

GPUI is designed for high-performance applications:

- **GPU-accelerated rendering** - All drawing happens on the GPU
- **Minimal allocations** - Efficient memory usage
- **Async operations** - Non-blocking UI updates
- **Compiled templates** - `view!` macro compiles to optimized GPUI calls

## Examples

See the `examples/advanced/text-features/` directory for a complete GPUI application demonstrating advanced text styling features.
