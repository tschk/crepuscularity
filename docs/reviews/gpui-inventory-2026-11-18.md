# GPUI source inventory, 2026-11-18

Compiled GPUI templates, runtime previews, and the experimental Kit adapter have
different contracts. This inventory records the current source boundaries, not
new desktop or GPU verification.

| Responsibility | Source | Contract |
| --- | --- | --- |
| CE facade | `crates/crepuscularity-gpui/src/lib.rs` | Reexports GPUI, the runtime, `view!`, `view_file!`, and build helpers. `application()` delegates to the platform crate. |
| Compile-time templates | `crates/crepuscularity_macros/src/lib.rs` | `view!` emits Rust builders. `view_file!` tracks template dependencies and expands includes as text. Explicit include props, selected component defaults, and authored include slots are compilation errors. |
| Runtime traversal | `crates/crepuscularity-runtime/src/renderer.rs` | Walks the shared AST, evaluates expressions and control flow, and resolves includes. An element with a template event renders a diagnostic instead of an inert control. |
| Runtime utilities | `crates/crepuscularity-runtime/src/styler.rs` | Applies the runtime utility subset and computes hover, focus, and active style refinements. Its API is distinct from compile-time builder generation. |
| Reload preparation | `crates/crepuscularity-runtime/src/hot_reload.rs` | Watches the entry and resolved source directories. File reads, include preparation, and parsing use the background executor. The initial snapshot is empty; reloads retain the last completed snapshot. |
| Public animation helpers | `crates/crepuscularity-gpui/src/animation.rs` | Provides animation helpers separately from the runtime renderer's template animation path. |
| Experimental Kit facade | `crates/crepuscularity-gpui-kit/src/lib.rs` | Reexports GPUI Kit and the shared macros, not the CE runtime. Kit types are not interchangeable with CE types. See [GPUI Kit](../gpui-kit.md). |

The runtime renderer has an explicit `Node::Embed` diagnostic. Neither that
diagnostic nor shared-AST parsing establishes browser-island execution or event
parity with the compile-time macros.

The existing [target capability review](target-capabilities-2026-11-05.md)
records the retained event and spacing limitations and the separate #853/#857
proposals. This inventory does not replace that review or claim those proposals
have landed. The [GPUI guide](../gpui.md) remains the target reference.

## Verification boundaries

The source paths above identify where a contract is implemented. Parser tests
can verify frontend admission and AST shape, but cannot establish rendered text,
Metal output, input behavior, hot-reload interaction, or cross-platform parity.
Those require the corresponding runtime, macro-consumer, and platform checks.
No desktop process or GPU test was run for this documentation update.

The broader historical target audit from PR #552 includes results for unpublished
runtime changes. Its claims must be reconciled with the selected implementation
and rerun before they become current validation evidence. In particular, removing
the runtime event diagnostic from documentation would contradict current source.
