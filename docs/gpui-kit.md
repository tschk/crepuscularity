# GPUI Kit adapter (experimental)

`crepuscularity-gpui-kit` combines Crepuscularity's compiled templates with
Longbridge's GPUI Kit. Select this crate for a Kit application. Existing
`crepuscularity-gpui` applications continue to use GPUI CE.

## Dependency and type boundary

The adapter pins `gpui-kit = "=0.7.0"`, inspected at upstream commit
[`0c830f4d257e69fdd17200650533ab4ca9a40cc0`](https://github.com/longbridge/gpui-kit/tree/0c830f4d257e69fdd17200650533ab4ca9a40cc0).
That release pins its GPUI snapshot family to `=0.3.7`, including `gpui-pre`
and `gpui-pre-platform`. The repository's CE requirements remain `gpui-ce 0.2.2`
and `gpui_ce_platform 0.1.0`. Resolve and test upgrades as a complete Kit graph.

The adapter depends on the shared parser and proc macros, but never on
`crepuscularity-gpui` or `crepuscularity-runtime`. Kit's `Element`, `Entity`,
`Context`, `Window`, and related types are not interchangeable with CE's types.
Do not use the CE workspace `gpui` dependency in a Kit consumer. Cargo feature
unification does not convert one framework's types into the other's.

Adding the adapter to the workspace makes workspace-wide commands include it;
building a CE package does not opt that package into Kit. The workspace lockfile
still resolves both dependency graphs. Coexistence in that lockfile must be
checked before publishing this integration.

## Use from a local application

The adapter is not published. Point to its source checkout and name the Cargo
dependency `gpui`, because the existing shared template macros emit `::gpui`:

```toml
[dependencies]
gpui = { package = "crepuscularity-gpui-kit", path = "../crepuscularity/crates/crepuscularity-gpui-kit" }
```

Alternatively, keep the package's default dependency name and declare this once
at the consuming crate's root:

```rust
extern crate crepuscularity_gpui_kit as gpui;
```

Use `gpui::prelude::*` for the template macros, GPUI traits, and common types.
The facade exposes Kit's API at its root and the original facade as `gpui::kit`.
Construct Kit controls in Rust, then insert the elements through raw expressions:

```rust,ignore
use gpui::component::button::Button;
use gpui::prelude::*;

let save = Button::new("save").label("Save").on_click(cx.listener(
    |this, _, _, cx| {
        this.saved = true;
        cx.notify();
    },
));

view!(r#"
div flex flex-col gap-2 p-4
  "Document"
  {save}
"#)
```

Call `gpui::init(cx)` before creating application content. Use
`gpui::open_window(options, cx, build)` to wrap that content in Kit's Base Root,
which hosts overlays and other window behavior. Do not add a second Root. Attach
`gpui::assets::Assets` with `application().with_assets(...)` when using bundled
assets. Window closure and quit actions remain application responsibilities.

The runnable counter example uses `view_file!`, a native Button, native Input
state with a retained subscription, and conditional template content:

```sh
cargo run --locked -p crepuscularity-gpui-kit --example kit-counter
```

## Feature selection

| Adapter feature | Effect |
| --- | --- |
| Default | Enables Kit's `component` and `assets` layers |
| `--no-default-features` | Keeps GPUI, Base, and compiled templates |
| `component`, `assets` | Forward the corresponding Kit features |
| `test-support` | Enables Kit's GPUI/UI test harness |
| `inspector`, `profiler` | Forward Kit's optional instrumentation |
| `decimal` | Enables styled components and their decimal support |
| `tree-sitter`, `tree-sitter-languages` | Enable styled editor syntax support |

Other Kit features can be selected through an application dependency on the
same exact `gpui-kit` release. This adapter does not duplicate Kit's complete
feature catalog. Kit 0.7.0 selects platform features including font-kit, X11,
Wayland, and runtime shaders; this adapter does not offer independent platform
feature switches. Turning off `assets` while keeping `component` does not remove
the component crate's own asset dependency.

## Coverage and limits

The following is the initial acceptance scope, not a claim of full GPUI Kit
coverage. Tests must pass against the resolved dependency graph before the
integration is considered validated.

| Area | Integration and acceptance evidence |
| --- | --- |
| Compiled layout, text, conditional classes, `if`, `for`, `match` | `tests/templates.rs` compiles representative templates and checks Kit type identity |
| Native element expressions | A Kit element is inserted through `{child}` and retains its upstream type |
| File templates | The runnable example and UI test share `examples/support/counter.crepus` |
| Native Button callbacks | UI test clicks Increment and Reset and checks rendered state |
| Native Input | UI test types Unicode text, checks focus/value, and checks the subscription's template update |
| Window hosting | UI test uses production `init`/`open_window` and asserts Base Root ownership |
| Kit components beyond this fixture | Available through Rust APIs and expression composition; not individually validated here |

Lowercase `button` is the shared macro's primitive GPUI element, not Kit's Button.
Lowercase `input` and `img` are not native Kit controls. Use native Kit builders
and their event/state APIs for these components. This PR adds no automatic Kit
tag catalog, DSL input binding, runtime renderer, hot reload, CLI scaffold, or
cross-backend conversion. Existing macro limitations still apply; passing the
focused tests does not establish all-class, all-event, accessibility, overlay,
or platform parity. Mobile and WebAssembly are outside this acceptance scope.

Existing CE applications may share template source within the tested subset,
but their state, subscriptions, event types, initialization, and native controls
need porting. The adapter deliberately does not re-export CE's runtime or its
window/animation convenience helpers. Revalidate Kit when shared macros change.

## Validation before publication

Resolve dependencies with Cargo in the assigned validation slot and inspect the
lockfile diff. Never hand-edit it. Check that CE's existing versions remain
unchanged, Kit uses the intended `gpui-pre` snapshot family, and a Kit-only
dependency tree does not include `gpui-ce` or `crepuscularity-runtime`.

Run the focused matrix after resolution:

```sh
cargo test --locked -p crepuscularity-gpui-kit --no-default-features
cargo test --locked -p crepuscularity-gpui-kit
cargo test --locked -p crepuscularity-gpui-kit --features test-support --test ui -- --test-threads=1
cargo check --locked -p crepuscularity-gpui-kit --example kit-counter
cargo clippy --locked -p crepuscularity-gpui-kit --all-targets --features test-support -- -D warnings
cargo test --locked -p crepuscularity_macros
cargo test --locked -p crepuscularity-gpui
```

The UI test uses Kit's headless test harness. It does not launch the example,
request device permissions, or establish acceptance on other operating systems.
Complete the relevant repository checks and review the immutable resulting diff
before opening the draft PR. The feature-gated UI test must be selected explicitly; the default
workspace test command does not run it.

## Licensing and dependencies

The new adapter uses the repository's ISC license. Kit, Base, Component, and the
assets crate declare Apache-2.0, which is already allowed by `deny.toml`.
Distributions must retain applicable dependency license/notice material. The
resolved transitive graph still needs the repository's license and advisory
checks; this is not a complete dependency license audit.

The adapter does not enable Kit's separate JavaScript shell or introduce its
QuickJS binding patch. Syntax grammars and instrumentation remain opt-in.
Use a Rust toolchain that supports the resolved dependencies, including Kit's
Rust 2024 edition; no minimum supported Rust version is claimed before testing.
