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

Anything outside the allowlist (`webView`, `filePicker`, `dropzone`, free
`input`/`picker`/`slider`, `tabs`, `slot`, …) renders nothing — it never throws.

## Action model

The package is **generic and action-agnostic**. `on_click` / `on_change` /
`on_long_press` strings are surfaced verbatim to the host via `onAction`; the
package never interprets, maps, or evaluates an action. The host owns action
policy (whitelisting, URL safety, etc.). There is no `eval` anywhere.

## Caps

`CrepusLimits` bounds node count (default 60), depth (8), and source length
(8000). Oversized input yields an empty document, gracefully.

## Distribution

This package is designed to be consumed as a published dependency, fetched and
pinned via the `crepus flutter` CLI affordance (see
`crates/crepuscularity-cli`). Until it is published, host apps pin it with a git
dependency on this repo (`git: { url, ref, path: plugins/flutter }`).
