# crepuscularity-flutter

> **Naming:** the package directory, the `crepus flutter` CLI, and all docs use
> the hyphenated `crepuscularity-flutter`, matching the sibling crates
> (`crepuscularity-native`, `crepuscularity-gpui`, …). The one exception is the
> Dart `pubspec.yaml` `name:` field, which must be `crepuscularity_flutter`
> because pub.dev names cannot contain hyphens (`^[a-z][a-z0-9_]*$`). The `import`
> and the dependency key in a consumer's `pubspec.yaml` therefore also use the
> underscore form — that is the pub package name, not an inconsistency.

Flutter runtime renderer for Crepuscularity — the Flutter counterpart to the
SwiftUI and Jetpack Compose native shells in
[`examples/native-shells`](../../examples/native-shells). It turns Crepuscularity
UI into Flutter widgets, from either the **View IR** wire format or `.crepus`
source parsed in pure Dart (no Rust round-trip).

Requires Flutter 3.35 or newer with Dart 3.9 or newer.

## Usage

```dart
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';

// From decoded View IR JSON ({ version, root: [...] }).
CrepusView.fromIr(irMap, onAction: (action) => handle(action));

// From .crepus source, rendered directly.
CrepusView.fromSource('''
stack col gap-2
  text font-semibold "Reminder"
  text text-sm "Pay rent tomorrow"
  button "Do it now" onclick=prompt:Draft a rent reminder
''', onAction: (action) => handle(action));
```

Both entry points produce the same widget tree; `fromSource(x)` and
`fromIr(lower(x))` agree on the shared fixtures (see `test/conformance_test.dart`).
`test/round_trip_test.dart` also compares resolved styles and rendered widgets
against Rust-generated `test/fixtures/styles.json`. Regenerate that fixture from
the repository root with `cargo run --locked -p crepuscularity-cli -- native ir
plugins/crepuscularity-flutter/test/fixtures/styles.crepus --pretty`, then save
the JSON output as `styles.json`.

## Vocabulary

Constrained, non-executing subset of the View IR `ViewNode` kinds
(`crates/crepuscularity-native/src/ir.rs`):

Targets View IR version 7 (`IR_VERSION`).

`text`, `stack` (row/column, Tailwind-ish spacing/alignment), `scroll`,
`button`, `toggle`, `checkbox`, `progress`, `timer`, `meter`, `sparkline`,
`badge`, `divider`, `spacer`, `image`, `link`, `if`, `forEach`, `list`,
`listItem`.

`ViewStyle` also carries the source `#id` and the raw class tokens alongside the
resolved layout/theming hints.

Source scroll containers keep their content stack’s direction, gap, and alignment
inside a separate viewport. A single `overflow-x-auto`/`overflow-x-scroll` utility
selects horizontal scrolling, and `overflow-y-*` selects vertical scrolling.
Generic or paired x/y utilities use the content direction; simultaneous two-axis
scrolling is not represented. The inner stack counts toward node and depth caps.

Anything outside the allowlist (`webView`, `filePicker`, `dropzone`, free
`input`/`picker`/`slider`, `tabs`, `slot`, …) renders nothing — it never throws.

## Source styles

Margin hints (`m-*`, standard negative forms such as `-mx-2`, and signed
arbitrary values) survive source parsing and JSON round trips. All, axis, and
edge margin layers apply additively outside decoration and opacity. Signed
layers shrink the layout slot and place content outside it without forcing
loose content to grow. Start/end placement follows text direction, including
zero-size slots; dry layout, intrinsic dimensions, and baselines include the
margin. Flow, decoration, and opacity wrappers preserve overflow painting and
hit testing, including gesture parents and positive padding around signed
layers. Gesture wrappers retain tap/hold semantics and dispose their recognizers
when removed. Margin magnitudes use the renderer's existing 4000-point limit. Signed
flows still paint overflowing content when constrained to zero height. List
marker rows preserve overflow interaction in bounded and unbounded layouts;
bounded rows keep a flexible item, while unbounded rows use intrinsic sizing.

Recognized classes are applied in source order; the last recognized class wins
for each style field, alignment, justification, and explicit stack direction.
Bare `flex` defaults to a row without overriding `flex-col`. Unknown classes are ignored.
Column `items-stretch` fills a bounded width. Under a row or horizontal scroll,
an unbounded width uses start alignment instead, including flows with signed margins.
Spacing accepts fractional scale values (`p-1.5` means 6 pixels) and bracketed
pixel values (`gap-[2.5px]`). Arbitrary font sizes and corner radii use pixels.
Named font sizes follow the Rust IR ramp (`text-sm`: 14, `text-base`: 16,
`text-lg`: 18), from `text-xs` through `text-9xl`.
Letter spacing accepts `tracking-tighter` through `tracking-widest` and
bracketed pixels such as `tracking-[2.5px]`; finite negative spacing is preserved.
Text nodes support positive `line-clamp-N` limits, with ellipses by default;
`text-clip` clips instead. `truncate` forces one unwrapped line and takes
precedence over a line clamp. `text-ellipsis` applies Flutter ellipsis behavior
without an explicit line limit. These map to IR `lineClamp` and `textOverflow`.
Line height supports `leading-*` with `none`, `tight`, `snug`, `normal`,
`relaxed`, or `loose`, plus numeric and bracketed values. Values map to the
IR `lineHeight` multiplier, including bracketed `px` values; numeric scale steps multiply by
four, matching native IR. Missing or nonpositive values use the existing 1.35
default. Flutter caps the unscaled nominal line height
(`fontSize × lineHeight`) at 4000 pixels; accessibility text scaling still applies.
Supported Tailwind color names use the shared v4 RGB values.
Text, background, and border colors accept palette names and CSS hex values
with 3, 4, 6, or 8 digits; alpha digits come last. A border width without a color
uses the theme border color. `opacity-25` means 0.25; finite percentages are
clamped to 0–100, and malformed or non-finite values are ignored.

Links with a nonempty `href` and list items with a nonempty `onClick` are
keyboard-focusable. Enter, numpad Enter, and Space dispatch the same host
action as a click; primary keyboard focus shows an outline. Nested buttons
retain their own activation, and empty actions remain inert. Accessibility
semantics identify actionable links and expose focusability and primary focus,
respecting `ExcludeFocus`
and Flutter's native iOS focus-action guard.

## Runtime data

Pass `data` to either `CrepusView` constructor. Text interpolation resolves
`{user.name}` through nested maps; missing values become empty strings. These
placeholders are data paths, not Rust or JavaScript expressions. Escaped braces
are not a supported literal-brace convention.

Display strings omit `.0` for finite integral doubles with magnitude below `1e15`,
including negative zero as `0`. This applies to text, dynamic labels and URL data
bindings. Other number formatting follows Dart.

The pure-Dart source parser tolerates unterminated quoted or braced tokens by
consuming through the end of the line. Use the Rust parser when strict source
validation is required; Flutter source parsing is constrained and tolerant.

`forEach.bind` resolves a list from an exact data key first, then a dotted path
such as `data.items`. Each iteration binds the item to `itemName`; nested maps
remain accessible through paths such as `{row.name}`.

`if.condition` supports a bare operand, `!operand`, or one `==`/`!=` comparison.
Operands can be data paths, double-quoted strings, finite numbers, booleans, or
`null`. Double-quoted expression strings decode `\n`, `\t`, `\"`, and `\\`;
other escapes, including `\r` and `\'`, remain literal. Source text uses its
separate text-literal escape rules. Numeric operands
compare by value, so `1 == 1.0` is true. Lists compare ordered elements and maps
compare keys and values structurally, using the same scalar rules for nested
values; collections never equal scalar strings. Other scalars compare as strings,
with null becoming empty and integral doubles omitting `.0` (exponent spelling
still follows Dart). Cyclic host collections terminate using previously visited
pairs; cycles are outside the JSON/Core data domain. Malformed conditions and
non-finite numeric comparisons are false.
Arithmetic and compound logical conditions are not interpreted.

View IR images accept optional `srcBind` and links optional `hrefBind`. The renderer
resolves supported paths/literals and canonical mixed-text bindings against current
root or loop data, including after host updates. Missing/null values become blank;
without a binding the existing URL snapshot is used. Unsupported expressions retain
that snapshot, matching the constrained dynamic-label evaluator. The tolerant
`.crepus` source parser also retains unquoted `src={expression}` and
`href={expression}` bindings. Quoted URLs containing braces remain literal.

## Action model

The package is **generic and action-agnostic**. `on_click` / `on_change` /
`on_long_press` strings are surfaced verbatim to the host via `onAction`; the
package never interprets, maps, or evaluates an action. The host owns action
policy (whitelisting, URL safety, etc.). There is no `eval` anywhere.

## Caps

`CrepusLimits` bounds node count (default 60), depth (8), and source length
(8000). Oversized input yields an empty document, gracefully.

`CrepusLimits.maxNodes` also bounds runtime node visits per build, including
`forEach` expansion. The default is 60; loop output beyond the remaining budget
is omitted. Each rebuild starts with a fresh budget.

## Distribution

This package is designed to be consumed as a published dependency, fetched and
pinned via the `crepus flutter` CLI affordance (see
`crates/crepuscularity-cli`). Until it is published, host apps pin it with a git
dependency on this repo (`git: { url, ref, path: plugins/crepuscularity-flutter }`).
