# Tauri native compatibility

This matrix defines the native Crepuscularity replacement contract. A row is complete only when its adapter API, native conversion behavior, and platform smoke test exist.

| Tauri surface | Status | Native target |
|---|---|---|
| v1 `distDir` / v2 `frontendDist` | supported | Crepus bundle discovery |
| JSON, JSON5, TOML configuration | supported | config discovery |
| product name, version, identifier, primary title | supported | Android/iOS project metadata |
| `.crepus` static bundle | supported | View IR, SwiftUI, Compose |
| `#[tauri::command]` | supported for serializable sync/async functions | native command bridge |
| `generate_handler!`, `generate_context!`, `Builder::invoke_handler` | supported | native command bridge |
| `Builder::manage`, `State<'_, T>` | supported | managed native process state |
| `AppHandle`, primary `Window`, `Manager` events | supported | native event bridge |
| `async_runtime::spawn` | supported | shared Tokio runtime |
| clipboard, dialog, opener, haptics, share plugin requests | supported request mapping | native capability request |
| multiple configured windows | supported | one GPUI native window per Tauri window |
| tray, menu, webview APIs | unsupported | no native equivalent yet |
| Tauri capability/allowlist files | unsupported | permissions must be mapped per target |
| filesystem, HTTP, store plugins | backend required | Rust action backend adapter |
| updater, shell, SQL, stronghold, sidecars | unsupported | no native equivalent yet |
| arbitrary HTML/JS frontend | unsupported | conversion requires `crepus-bundle.json` |

`crepus tauri audit --dir <project>` must classify every discovered input. `crepus tauri convert` fails before generating files when the audit contains a `backend` or `unsupported` item.

Conversion preserves the bundled source graph in both `views/` and desktop
`views/`. A `main.crepus` entry remains the authored source. If another entry
shares its graph with a root `main.crepus`, sources move together under
`__crepus_source/`, with a generated `main.crepus` wrapper pointing to the entry;
relative includes retain their layout.

For Rust command invocation from async tasks, use
`app.invoke_async(name, payload).await`. It runs the registered handler on the
adapter's blocking pool, allowing the caller's runtime to progress while the
handler waits. Borrowed arguments and non-`Send` command futures are supported
because those futures are created on the worker. Handlers have no host-thread
affinity on this route, and dropping the invocation future does not cancel an
already dispatched handler. Synchronous `app.invoke` keeps its calling-thread
behavior; invoking an async macro command synchronously from an async task
still hits Tokio's nested-runtime restriction.

The reference inputs are the [Tauri v2 configuration](https://v2.tauri.app/reference/config/), [capabilities](https://v2.tauri.app/security/capabilities/), [commands and events](https://v2.tauri.app/develop/calling-rust/), [mobile plugins](https://v2.tauri.app/develop/plugins/develop-mobile/), and [Tauri v1 configuration](https://v1.tauri.app/v1/api/config/).
