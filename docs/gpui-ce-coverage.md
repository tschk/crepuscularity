# GPUI-CE coverage contract

Status: **incremental implementation, not full template parity**. This document
separates access to native APIs from template shorthand and verified behavior.
The source patch and regression tests below have not yet been compiled or run. The original foundation patch is preserved outside the checkout with SHA-256 `d298e8012ce5e123e03f3ab0efd81035c24f20afc5e3f3d138e8f32ed4c5080f`; this candidate extends it without rewriting that artifact.

## Baseline and upgrade decision

The baseline is Crepuscularity main
[`3aa603b`](https://github.com/tschk/crepuscularity/commit/3aa603b99f4beb1f5936b4da2ff05ec03f714fef):
adapter 0.5.10, runtime 0.4.20, macros 0.5.12, core 0.5.0. Its lock selects
`gpui-ce 0.2.2` and `gpui_ce_platform 0.1.0`. No dependency version bump is made.

Verified on 2026-10-03 against the official registry index:

| Package | Latest non-yanked version | Locked archive checksum |
| --- | --- | --- |
| [gpui-ce](https://index.crates.io/gp/ui/gpui-ce) | 0.2.2 | `b0af79c6659e0fea67773cfbd751fc5c8e51be00139827f365d7d1237a468a4d` |
| [gpui_ce_platform](https://index.crates.io/gp/ui/gpui_ce_platform) | 0.1.0 | `42fc3ad60af3e375b224e701a3e5db31f4def8e3a1099899882e937a9e9c12e9` |

CE 0.3.2 and 0.3.3 are yanked. The published archives record VCS base
`99109e3a77ddadbcb46cba8567e779a651600608` with `dirty: true`; that is **not** an
exact clean commit equivalent. The archive checksums identify the audited source.

CE Git main was
[`c6b17e6`](https://github.com/gpui-ce/gpui-ce/commit/c6b17e616a35271183ab49f0da1890ee81953a99).
It adds Parley inline/bidirectional text, Motion/springs and accessibility APIs.
Moving to it requires a separate migration: core `font-kit` and platform
`runtime_shaders` features disappeared, `BoxShadow.color` changed from `Hsla` to
`Background`, and the toolchain requirement is Rust 1.95. It is not a safe
version-only change. Existing `Animation::new`, `repeat`, and `with_easing` still
exist there; their removal is not a migration blocker.

Upstream Zed GPUI is a different dependency lineage. Its published version was
also 0.2.2; latest inspected GPUI-path commit was
[`f37989f`](https://github.com/zed-industries/zed/commit/f37989fbdf840b308892af66516c2b9c3ef3853a)
(headless support), within repository head
[`5eed239`](https://github.com/zed-industries/zed/commit/5eed2397a863423c50bebee8f001dad33c3e15f2).
Equal version strings do not make these crates interchangeable.

GPUI Kit and gpui-component are widget libraries, not GPUI core. The separate
Kit lane targets Kit 0.7.0 / `gpui-pre =0.3.7`; its entities and traits cannot be
mixed with CE entities as if they were the same Rust types.

## What access is provided

`crepuscularity_gpui::gpui` exposes the original CE crate namespace, including
names shadowed by Crepuscularity compatibility helpers such as `Anchor` and
`WindowButton`. `crepuscularity_gpui::gpui_platform` exposes the platform crate.
This avoids a lossy hand-maintained public-item wrapper. Feature and target
restrictions still apply.

Compiled templates can insert a Rust element expression or apply a typed native
builder using `bind:native={|element| ...}`. Generated paths stay `::gpui`, so
the caller's dependency alias determines the type universe.

Runtime templates can register `RuntimeBindings::element` factories. Each
factory receives its scoped ID, AST, evaluated context, rendered children, and
the real `Window` and `App`. Capture retained entities in the host closure for
input, canvas, typed actions, typed drag payloads, lists or other native widgets.
The factory owns applying any template styling and semantics; it does not get
automatic behavioral bindings. Script/frontmatter strings are never executed.

These escape hatches provide native API access. They do **not** establish declarative parity. Runtime `input type="text"` now has a dedicated retained implementation; compiled input lowering and other input types remain unsupported. The [source inventory](gpui-ce-api-inventory.json) lists explicit
methods and feature declarations; it is not a coverage percentage.

## Coverage matrix

"Implemented" here describes this source patch, pending compilation and tests.
"Native" means application code must supply the behavior through the escape
hatches above. No row establishes platform acceptance merely by compiling.

| API family | Compiled template | Runtime template | Remaining acceptance/gaps |
| --- | --- | --- | --- |
| Entities, globals, subscriptions, async tasks, custom `Element`/`Render` | Native Rust | Native factory with retained entities | Host owns lifecycle; no DSL serialization |
| Div/text hierarchy and conditionals | Existing mappings | Existing mappings | Six frontends need shared behavioral fixtures |
| Loop identity | Keyed root (`key={item.id}`), index fallback | Keyed root, duplicate-key diagnostic, index fallback | Reorder tests and nested-loop render acceptance |
| Template instances/includes/slots | Identified native parent required for repeated macro instances | Explicit namespace threaded through all recursion | Legacy call-site fallback cannot distinguish repeats at one call site |
| Layout/color/spacing/typography/borders/shadows | Utility subset; self alignment/items-stretch added | Utility subset; items-stretch added | Unknown static utilities and CSS-only approximations remain; generated helpers need full mapping inventory |
| Hover/focus/active styling | One accumulated refinement per state, including conditions | One accumulated refinement per state | Focus needs focusable/host focus handling; native handles advanced state selectors |
| Mouse/key/scroll/pinch/hover | Named direct mappings; Ctrl and Command distinct; unknown modifiers error | Registered callbacks borrow original event and receive Window/App | Real dispatch, bubbling and propagation tests pending |
| Capture, gestures, actions, tooltips, focus handles | Native builder hook | Native factory | No generic string-to-type conversion |
| Disabled controls | Omit declared callbacks and focusable flag | Generic callbacks omitted; input additionally writes native disabled/readonly flags | Generic controls still lack disabled a11y semantics; input node/tree acceptance pending |
| Raster images and SVG | Actual identified image/SVG child in semantic wrapper | Same; asset/URL and filesystem sources distinguished | Intrinsic/constrained layout, loading failures and animated frames need UI tests |
| StyledImage transforms/fallbacks/cache, canvas/surface | Native | Native factory | No dedicated DSL properties yet |
| Text input/selection/composition | Retained `Entity<TextInput>` via native expression; built-in macro input still errors | Built-in retained single-line text input; 11 EntityInputHandler methods, model and synthetic fixtures written | All tests pending; textarea/password/number/autofill/validation/visual-bidi editing still unsupported; real IME acceptance open |
| Roles, names, descriptions and values | Typed AccessKit roles and metadata bindings | Named role subset and basic metadata/checked bindings | Extra runtime metadata fields/actions/synthetic children use native factory |
| Accessibility actions/synthetic children | Native builder/custom element | Built-in input provides text run, directional selection, SetValue and SetTextSelection; generic nodes use native factory | Accessibility tree and screen-reader verification pending |
| External file drop | Native builder hook | `@drop` borrows `ExternalPaths` | OS delivery not verified |
| Internal typed drag/drop, external drag source | Native builder hook with exact payload type | Native factory with exact payload type | No generic serialized drag substitute |
| Animation/Transition | Native; AST animate attributes now error explicitly | Existing supported animation subset with scoped identity | Built-in scale approximation removed; unknown properties report error; easing/duration validation remains incomplete |
| Lists/uniform lists/deferred/anchored/container queries | Native | Native factory | Dedicated template shorthands absent |
| Window/App/platform/assets/HTTP/clipboard/keymap/testing | Native namespace | Host code/factory | Platform capabilities are not exercised by template tests |

## Runtime integration

Indentation syntax uses `key={...}` or `bind:key={...}`; a leading `:key` is not
an indentation binding. Vue's binding syntax is normalized by its own frontend.

```rust,ignore
use crepuscularity_gpui::gpui::{IntoElement, ParentElement};
use crepuscularity_gpui::crepuscularity_runtime::{
    RuntimeBindings, render_nodes_with_bindings,
};

let bindings = RuntimeBindings::new()
    .on("save", move |event, window, cx| {
        // Match RuntimeEventKind, update host-owned entities, stop propagation,
        // or notify the appropriate view. event.context includes loop/include data.
    })
    .element("Editor", move |request, window, cx| {
        // editor is an Entity of a retained native editor captured by the host.
        Ok(editor.clone().into_any_element())
    });
let element = render_nodes_with_bindings("document:42", &nodes, &context, &bindings);
```

```text
div
    for item in {items}
        button key={item.id} @click=save disabled={item.disabled}
            "{item.title}"
    img src={logo_asset} alt={logo_description}
    Editor
```

Compiled loop roots that are custom components are placed in an identified div; this adds a layout container. Built-in roots receive the scoped ID directly.

Use a different stable namespace per simultaneously mounted template instance.
An index fallback is only safe when rows do not reorder. Keep keys unique and
stable. Legacy `render_nodes`/`render_node` derive a namespace from caller
location; callers looping at the same source location must migrate to the
explicit API. `HotReloadState.bindings` and the owning state entity's identity
provide the registry/namespace for hot-reloaded views.

`src` strings are embedded asset paths or URLs, as defined by CE `ImageSource`.
Use `path={filesystem_path}` for filesystem raster images. The semantic wrapper
exists because the pinned `Img` implementation does not expose its own a11y
node. Size/min/max utilities are transferred to the actual image child, with a default 1rem SVG size. The semantic wrapper follows its child. Percentage sizing, padding, fitting, state-dependent dimensions and native rendering callbacks still need rendering acceptance or a native factory.

## Feature and platform boundary

The adapter now forwards core `font-kit`, `input-latency-histogram`, `inspector`,
`leak-detection`, `profiler`, `bench`, `windows-manifest`, plus platform `font-kit`,
`wayland`, `x11`, `screen-capture`, `runtime_shaders`, `test-support`, and `wgpu`.
`full-gpui` retains its historical meaning: upstream defaults, not every optional
feature. Implicit dependency features are available through the original CE
dependency when required; this is not an all-features build claim.

Platform crate 0.1.0 selects macOS and Windows by target; Linux/FreeBSD need
the appropriate Wayland/X11 setup. The platform also has a wasm backend, which
does not make the filesystem/hot-reload/CLI stack a verified browser target.
`wgpu` in this pinned release forwards to Windows only. `runtime-shaders` is a
macOS backend option. The pinned platform's test-support headless path is macOS
only; newer Zed headless behavior must not be attributed to this CE release.

## Verification and release gate

Completed for the foundation: archive checksum/source review, independent source review, standalone
Rust formatting/parser check, `git diff --check`, build-free inventory generation.
Not completed: Cargo type checking, unit tests, native runtime dispatch, image
layout/animation, input/IME, accessibility trees, cross-platform feature builds.
No heavy jobs, dependency downloads or live device/UI actions were run for this
milestone. Do not publish or label it "full coverage" on this evidence.
The input extension's independent source review found missing TextRun metadata,
tab-stop opt-in and newline normalization. The corrected candidate also addresses
grapheme-interior deletion, cursor-only geometry, alignment and composition-end
notifications; its narrow follow-up review is recorded in the external receipt.

When a build slot is assigned, first run the macro unit tests, then focused runtime/adapter tests including
`crates/crepuscularity-gpui/tests/template_coverage.rs`. The fixture covers images,
native builders, typed IDs, custom loop roots, captured callbacks and namespace access. Use one Cargo job and the approved
target directory; do not regenerate the shared lock concurrently with Kit work.
After that, build deterministic headless dispatch/state/a11y regressions before
requesting real-platform acceptance. The retained input implementation and editing/composition tests are now part of this candidate; all remain unexecuted. See [validation and remaining gaps](gpui-ce-validation.md) for exact commands and prerequisites.

Regenerate the inventory from already-cached sources:

```sh
python3 scripts/gpui-ce-inventory.py \
  --core /path/to/gpui-ce-0.2.2 \
  --platform /path/to/gpui_ce_platform-0.1.0 \
  --output docs/gpui-ce-api-inventory.json
```
