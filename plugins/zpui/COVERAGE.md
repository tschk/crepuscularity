# ZPUI completion goal

Finish all **25 View IR v7 node kinds, 71 ViewStyle fields, and five event contracts**
with working behavior and acceptance evidence. `coverage.json` inventories the
requirements and prevents a complete status while gaps remain. The checker verifies
inventory agreement; it does not establish implementation correctness.

## Tested 0.2.0 checkpoint

All 25 node shapes decode. Twenty-four are implemented directly, including retained
input, scoped conditions/collections, timed text, file operations and icon tabs.
The macOS browser reference navigates actual WKWebView content and passes title,
pixel and teardown acceptance. Windows/Linux browsers still need host factories
and platform validation, so browser coverage remains partial.

The style inventory records 62 implemented, three partial and six pending fields.
Translations preserve flow while moving paint/hit/AX bounds. All object positions
use real image UV crops. Text transforms cover Unicode upper/lower casing and
inheritance/reset; capitalization and transformed editor display remain unfinished.
Static/relative/absolute positioning and supported cursors have explicit contracts.

Validation: 21 decoder tests on Zig 0.16/0.17; 44 native headless tests with the
pinned backend; five Rust Unicode tests and strict Clippy; actual macOS input and
WKWebView acceptance with allocator teardown. Current-candidate independent review
and CI remain required before release. A host-factory witness alone is insufficient
for editor/browser implementation claims.

## Work required to finish

1. Finish `letterSpacing`, `zIndex`, `scaleX`, `scaleY`, `rotate`, `userSelect`,
   capitalization, transformed input display, fixed positioning and wait cursor.
2. Provide Windows/Linux browser references and native OS/widget/GPU acceptance
   for the supported platform matrix.
3. Independently review the immutable final candidate and pass its CI.

The pinned ZPUI backend exposes no whole-element affine transform, text letter
spacing, selectable-text policy or viewport-fixed positioning surface. Scale and
rotation require a narrow backend extension that covers scene/shader primitives,
clipping, inverse pointer coordinates and native-view placement. Axis-aligned
bounding-box approximations would not satisfy this goal. Z-index also needs a
stacking-order contract beyond the backend's global deferred-draw priority.

The adapter stays under `plugins/zpui` with its own locked Unicode helper and CI.
Root Cargo/shared DSL/CE/Kit files remain outside this lane. The CLI 0.17.0 version
patch is prepared separately; publication requires coordinated root version/lock
changes and full CLI validation. A separate proposal resolves ammonia 4.1.7 and
quinn-proto 0.11.18 and passes audit; it remains unapplied to this PR.
No dependency audit or merge gate is weakened. ZPUI remains MIT; the adapter is ISC.
