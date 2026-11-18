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

`cargo run -p crepuscularity-lsp -- --stdio` starts `crepus-lsp` over standard
input/output. The server provides diagnostics, completions, and hover information
with full-document synchronization. Published diagnostics include the document
version. Stale updates are ignored, and closing a document clears its diagnostics.
Completion and hover use the document filename to select the template context.

## VS Code executable discovery

`crepus.languageServerPath` takes precedence and is passed unchanged as an
executable path or command name, without a shell. With an empty setting, the
extension searches executable files on `PATH`, then the first workspace folder's
`target/debug/crepus-lsp`. Windows uses `crepus-lsp.exe` for that fallback and
only native `.exe`/`.com` entries from `PATHEXT` during automatic discovery.
Batch scripts are not automatic candidates. Missing servers produce an error
that names the setting.

Restart and deactivation invalidate pending discovery results. Shutdown waits
for startup and serializes stop attempts before a replacement can launch.
Failed shutdowns retain the client for a later retry. A failed asynchronous
deactivation rejects rather than reporting successful shutdown.

The isolated editor checks run with `bun test editors/vscode/tests` from the
repository root. Lifecycle checks substitute VS Code and LanguageClient
collaborators; executable discovery uses real temporary files. Windows paths
are simulated, not a Windows process-launch test.

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
