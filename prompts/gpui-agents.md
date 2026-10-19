# crepus new / crepus init gpui — GPUI desktop apps

Native desktop applications using Zed's GPUI framework with the `view!` macro.

## Quick start

```bash
crepus new my-app
cd my-app
crepus dev                              # hot-reload dev loop
crepus build --release                  # cargo build --release
```

## Project structure

```
my-app/
  Cargo.toml           # dep: gpui, crepuscularity-gpui
  src/main.rs          # view! macro with inline .crepus templates
  .gitignore
```

## Cargo.toml

```toml
[dependencies]
gpui = { version = "0.2", default-features = false, features = ["font-kit"] }
crepuscularity-gpui = { version = "0.4.1" }
```

## Template syntax (inline in Rust)

GPUI templates use the `view!` macro with `.crepus` string templates inline:

```rust
use crepuscularity_gpui::prelude::*;

struct MyView { count: i32 }

impl Render for MyView {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.count;
        view! {r#"
            div w-full h-full bg-zinc-950 text-white flex flex-col items-center justify-center gap-6
                div text-8xl font-bold leading-none
                    "{count}"
                button bg-white text-black font-semibold px-6 py-2 rounded-lg @click=increment
                    "increment"
        "#}
    }
}
```

## Features

- `view!` macro — compile `.crepus` templates into GPUI elements
- Full GPUI API re-exported through `crepuscularity_gpui`
- Event handlers: `@click=method_name`
- Hot-reload with `crepus dev`
- Conditional classes, bindings, animations
- `include` for component reuse

## Feature flags

```toml
crepuscularity-gpui = { features = ["font-kit"] }        # default
crepuscularity-gpui = { features = ["full-gpui"] }       # all GPUI features
crepuscularity-gpui = { features = ["symbols"] }          # icon symbols
crepuscularity-gpui = { features = ["inspector"] }        # GPUI inspector
crepuscularity-gpui = { features = ["wayland"] }          # Linux Wayland
crepuscularity-gpui = { features = ["x11"] }              # Linux X11
```

## Key crates

- `crepuscularity-gpui` — view! macro, GPUI re-exports
- `crepuscularity-core` — parser, AST
- `crepuscularity_macros` — view! proc macro
- `crepuscularity-runtime` — hot-reload, shared runtime types