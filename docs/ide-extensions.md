# IDE extensions

This doc sketches how editors can integrate with **Crepuscularity** and **Aurorality** for a smoother `.crepus` workflow.

## Goals

- **Syntax highlighting** for `.crepus` (indentation trees + optional JSX-style regions).
- **Snippets** for common blocks (`navigationsplitview`, `for` / `if`, Tailwind-like classes).
- **Tasks / runners** wired to the CLI:
  - `crepus` for web / GPUI targets.
  - `aurorality swiftgen` for SwiftUI emission.
  - `aurorality dev` for hybrid hot reload (IR + optional swiftgen status over WebSocket).

## Language server

Run `cargo run -p crepuscularity-lsp -- --stdio` from the repository to start
`crepus-lsp` over standard input/output. It provides diagnostics, completions,
and hover information, with full-document synchronization on changes. Hover and
completion retain tag, quote, and expression state across lines in documents
selected as JSX by the parser's content rules or a `.jsx`, `.tsx`, or `.csx`
filename. Leading comments and declarations follow the parser's preamble rules.
Indentation syntax uses the current line for context. Other frontend filenames
do not yet select a frontend-specific hover/completion lexer.
Diagnostics carry the version received on document open or change; closing a
document clears its diagnostics without a version. Edits are accepted only for
open documents with a newer version; reopening resets the version sequence.
State changes and diagnostic publication are ordered together, including close
and reopen. Outgoing-channel backpressure can delay other document operations.

## Aurorality dev server

When `aurorality dev` runs with `--swiftgen-view` / `--swiftgen-out`, saves to the watched template can trigger:

1. **IR push** over the existing hot-reload WebSocket (unless `--no-ir`).
2. **`SwiftgenStatus`** envelopes after `swiftgen` runs (success + diagnostics).

Editors can surface diagnostics from `SwiftgenStatus.errors` next to the template path without doing a full Swift build.

## UniFFI / JSON envelopes

Hot reload messages are defined in `crepuscularity-native` (`HotReloadMessage`). Swift clients decode the same shapes as the CLI emits (`DevHello`, template reload JSON, `SwiftgenStatus`).

## Related

- [Aurorality (SwiftUI engine)](aurorality.md)
- [DSL reference](dsl.md)
- [CLI](cli.md)
