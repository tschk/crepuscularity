# CE retained-input candidate: contract, gaps and validation

This is source work awaiting a build slot. No Cargo compilation, test run,
dependency installation, native UI or device operation has been performed.
Do not merge, publish, or describe it as full GPUI-CE declarative coverage yet.

## Input contract

Runtime `input` and `input type="text"` now construct a retained `TextInput`
entity via CE `Window::use_keyed_state`. Its lifetime follows consecutive
rendered frames; removing/reinserting the field creates fresh state. Explicit
template namespaces and stable loop keys preserve the identity across renders
and reorders. Host `Entity<TextInput>` values can also be inserted as raw native
expressions in compiled templates. Shared macros still use `::gpui`; they do not
import the CE runtime or mix CE and Kit/gpui-pre types.

```text
input #display-name value={name} placeholder="Name" aria-label="Display name" @input=edit_name @change=save_name @submit=submit
```

`@input` receives the current draft, UTF-16 selection range, reversed-selection
flag and composing flag. A registered host handler must write its own state if
it wants two-way synchronization. `@change` fires when the value differs from its
last committed value on blur or Enter. `@submit` fires on Enter outside an
active composition. Native hosts can subscribe to `TextInputEvent` on their
entity for draft edits. No extracted script or callback text is evaluated.

Internal cursor/anchor/marked offsets are UTF-8 boundaries. The platform bridge
uses UTF-16. Nonempty ranges intersecting a surrogate pair expand to cover its
whole scalar; an insertion inside a pair snaps backward without deleting it.
Deletion and horizontal logical movement use Unicode grapheme boundaries.
Reversed selections preserve anchor and focus independently.

Composition updates replace the marked range, retain the original transaction,
and convert the new selection relative to the inserted marked string.
`unmark_text` commits; Escape cancels and restores original text/selection. A
committed composition is one undo step. Readonly/disabled transitions commit the
existing composition and block subsequent text mutation. Readonly permits
selection/copy. Disabled fields omit focus and behavioral listeners and write a
disabled accessibility flag.

Repeated external values do not reset a local draft. An external echo of the
current draft acknowledges it without ending composition. A different external
value during composition is deferred, with the latest update winning when the
composition ends; it clears obsolete undo history. New authoritative values
outside composition update the text and clamp the selection safely. Undo is
bounded to 100 text transactions. A single-line field replaces newlines with
spaces.

Collapsed deletion inside an extended grapheme consumes the complete cluster,
including when a platform selection lands between a base and combining mark.
Explicit nonempty scalar selections retain their requested range. Placeholders
also normalize line breaks before single-line shaping. Readonly fields remain
tab stops; disabled fields are removed from the active tab-stop tree.

The input paints selection, caret and marked-text underline, retains shaped-line
geometry, horizontally scrolls the caret into view, and refreshes stale geometry
for candidate bounds queries between paints. macOS CE converts these window
bounds into screen coordinates. The accessibility wrapper writes actual
disabled/readonly flags, value/label/placeholder, a synthetic text run and
directional selection. SetTextSelection validates its text-run node IDs;
SetValue is installed only when editable.

Text alignment offsets are shared by painting, hit testing and candidate bounds;
the native TextStyle supplies letter spacing and decorations. Cursor-only queries
refresh horizontal scroll even when the shaped text is unchanged. A permission
change that ends composition emits a terminal `composing=false` snapshot. Runtime
render-triggered notifications are deferred until the host view is no longer
borrowed, retaining the event/config snapshot for that transition.
Placeholder paint uses a separate shaped line and origin: its width never changes
the empty text's caret/candidate position. A synthetic fixture checks both center
and right alignment with changing hints and deletion back to an empty field.

## Explicit gaps

The [machine-readable inventory](gpui-ce-api-inventory.json) now classifies each
of 256 explicit methods across ten pinned traits. Generated style helpers and
cfg alternatives are listed separately. Utility references indicate a subset,
not general argument support. No absence of a wrapper is counted as parity.

| Area | Unsupported or unverified |
| --- | --- |
| Compiled input syntax | `input`/`textarea` still diagnose missing built-in lowering. Inserting a retained entity is native composition, not declarative parity. |
| Runtime input types | `textarea`, password, number, date, file, checkbox/radio input types, max-length/validation/autofill, rich text and multiline layout are not implemented. Unsupported bindings/types report errors. Password content is never silently treated as ordinary text. |
| Advanced editing | Visual bidirectional movement, vertical/multiline movement, word deletion, platform context menus, double-click word selection, drag autoscroll outside the field, touch selection handles, and caret blinking remain open. Native shaping does not establish these semantics. |
| IME | Model and all EntityInputHandler methods are implemented, but Japanese/Korean/Chinese dead-key/composition flows and OS candidate-window positions are unverified. |
| Accessibility | Input flags/actions/text selection have source and node fixtures. Whole-tree focus, text-run geometry, VoiceOver/Orca/Narrator navigation and action delivery remain unverified. Generic disabled buttons still lack a native disabled property mapping. |
| Images | Pixel/intrinsic dimension transfer has a fixture. Percentage sizing, padding, state-dependent dimensions, async failures, animation frame retention and actual paint output remain unverified. |
| Styling | Utility coverage is incomplete. CSS-only utilities and unknown static classes still include legacy silent no-ops; nested selectors, group state and general StyleRefinement arguments require native code. This is a coverage gap, not claimed support. |
| Actions, drags, lists, canvas | Typed action/drag payloads, drag-source lifecycle, tooltips, list virtualization, custom drawing and host services still require native code. File-drop shorthand is only ExternalPaths. |
| Animation | Runtime supports the documented limited properties; scale is rejected. Compiled animate attributes diagnose missing lowering. Arbitrary easing/duration expressions, Transition and new CE Git Motion features have no built-in template mapping. |
| Targets | No macOS/Windows/Linux/FreeBSD/wasm build matrix has run. Kit/gpui-pre remains a separate backend. |

## Validation gate

The immutable foundation is the external patch with SHA-256
`d298e8012ce5e123e03f3ab0efd81035c24f20afc5e3f3d138e8f32ed4c5080f`.
The new candidate gets a separate patch/hash after source review. Canonical
`~/projects/crepuscularity` and its WIP are not validation targets.

Prerequisites before any Cargo command:

1. The parent explicitly allocates one build slot and approves a target directory.
2. The integration/Kit lock owner adds **only the required runtime dependency
   edge** for `unicode-segmentation`, then reviews/freezes the combined lock.
   The baseline already contains 1.13.3 with checksum
   `c6f5d3c3b1bf09027a88a6bc961fc00497d651009560b5463668dc81b0fa87a8`.
   This lane has not changed the root lock. Running `--locked` before this update
   is expected to fail; do not drop the flag or resolve concurrently with Kit.
3. Dependencies must already be cached. All commands use `--offline`; a missing
   cache is a blocker to report, not permission to download/install.
4. Use a Rust toolchain compatible with the pinned CE source. This host reports
   `rustc 1.98.1 (48a229cea 2026-09-01)`; unicode-segmentation declares MSRV 1.85.
   On macOS, native builds need the selected Xcode SDK/toolchain. CE's pinned
   macOS build script can fall back to runtime shader compilation when the Metal
   command-line component is unavailable; do not install or switch toolchains
   automatically. Linux/FreeBSD need existing target-native X11/Wayland/font and
   graphics development libraries; Windows needs an existing native toolchain/SDK.

Run serially from the isolated checkout, after substituting the approved target
directory. These commands have **not** been executed:

```sh
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=/tmp/crepuscularity-ce-approved-validation
cargo test --offline --locked -p crepuscularity_macros --lib gpui_coverage_tests -- --test-threads=1
cargo test --offline --locked -p crepuscularity-runtime --features test-support --lib -- --test-threads=1
cargo test --offline --locked -p crepuscularity-gpui --test template_coverage -- --test-threads=1
cargo test --offline --locked -p crepuscularity-runtime --features test-support --test gpui_dispatch -- --test-threads=1
cargo check --offline --locked -p crepuscularity-gpui --features inspector,leak-detection,input-latency-histogram,profiler
```

Expected gates: macro expansion preserves native aliases/opaque components;
model tests cover surrogate/grapheme/reversed selection, IME transaction undo,
cancel/commit, external updates and readonly guards; node fixtures cover native
disabled/readonly semantics; the downstream fixture type-checks custom IDs,
closures/images; synthetic TestPlatform tests dispatch callbacks, block disabled
callbacks, retain an input entity through template re-rendering, exercise
word-navigation/selection/undo shortcuts and verify readonly/disabled tab order.
Fixtures explicitly draw before dispatch. A focused native-bridge fixture checks
the terminal composition event on a runtime permission change.

TestPlatform uses CE's NoopTextSystem. Passing these tests cannot verify real
font shaping, pixel output or OS accessibility/IME. Native GPU/headless rendering
and real UI acceptance are separate future runs, requiring their own authorized
slot and environment. The pinned platform's `current_headless_renderer` is
macOS-only. Do not substitute newer Zed headless APIs or silently skip a failing
native gate. Native acceptance must include constrained/intrinsic images, IME
candidate positioning after horizontal scrolling, composition commit/cancel,
keyboard shortcuts with Ctrl versus Command, readonly/disabled focus changes,
and screen-reader SetValue/SetTextSelection against a captured tree.
