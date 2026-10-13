# ZPUI completion goal

Finish all **25 View IR v7 node kinds, 71 ViewStyle fields, and five event contracts**
with working behavior and acceptance evidence. The target is complete integration,
including retained input, scoped conditions/collections, timed text, files, and native
browser hosting. Decoder acceptance and a host-factory witness do not establish a
working text editor or browser.

`coverage.json` records the complete inventory and current gaps. Run
`python3 tools/check-coverage.py` from this package to compare the inventory against
the authoritative Rust `ViewNode` and `ViewStyle` definitions. CI runs the check.
It rejects a `complete` status while any item, acceptance gate, or remaining task
is unfinished. It does not independently prove implementation correctness.

## Current checkpoint

All 25 node shapes decode with owned storage. The bridge directly implements 23
kinds. `input` and `webView` have explicit, fallible host factory contracts, with
request/placement tests; a reference retained editor and real browser acceptance
remain required. Missing factories or platform support return errors. Tab icons
are preserved by the decoder and explicitly rejected by the renderer pending
actual icon rendering.

Conditions select active branches. Collections require unique nonempty keys and
carry scoped values into text, controls and callbacks. Rotation uses entity-owned
timers. Long press recognizes a 500 ms hold, cancels on release or movement above
8 points, and copies its async context. File picking uses the native platform
dialog; drops use `ExternalPaths`. Both filter extensions. MIME filters require
an explicit host resolver. Dialog callbacks copy context and survive document,
window and App retirement without dereferencing dead host storage.

The style inventory has 59 implemented entries, three partial entries
(`position`, `cursor`, `objectPosition`) and nine pending entries. The nine are
`letterSpacing`, `textTransform`, `zIndex`, `translateX`, `translateY`, `scaleX`,
`scaleY`, `rotate`, and `userSelect`. Source classes are retained as metadata;
resolved numeric/enum hints determine the native styles. All eight two-stop
linear gradient directions map to ZPUI's native gradient fill.

## Work required to finish

1. Provide a retained editor reference with text/value synchronization, selection,
   caret movement, clipboard, IME/marked text, Unicode grapheme editing, secure
   display, multiline behavior, and teardown tests. Implement `crepusInput` with
   that editor; do not count the contract witness in `runtime_native.zig` as one.
2. Provide a browser reference that hosts an actual web engine. Verify source
   navigation, layout/clipping, focus, unmount, and window teardown. The headless
   native-view witness establishes placement plumbing only.
3. Complete tab icons, the nine pending styles, static/fixed positioning, wait
   cursor, and all image positions. The pinned backend lacks a general element
   transform, letter-spacing and selectable-text surface. Add tested bridge
   elements or a narrow, separately reviewed backend extension where necessary;
   an axis-aligned bounding box is insufficient evidence for rotated content.
4. Run real OS-widget/GPU acceptance and the supported platform matrix. Keep
   headless layout/event tests separate from visual/browser acceptance. Review
   an immutable final candidate and require its CI before declaring completion.

The optional adapter stays isolated under `plugins/zpui`, with documentation and
its own CI workflow. Root Cargo dependencies, shared DSL/macros, CE and Kit types
are outside this lane. The pinned ZPUI source remains MIT; this independently
written adapter remains ISC. Broader dependency security remediation is a separate
repository lane and does not justify bypassing a merge gate.
