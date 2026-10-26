# crepuscularity-tauri

Native Tauri-compatible command, event, and plugin primitives for Crepuscularity projects.

```toml
[dependencies]
tauri = { package = "crepuscularity-tauri", version = "0.1.1", features = ["native"] }
serde_json = "1"
```

```rust
#[tauri::command]
async fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {name}"))
}

let app = tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![greet])
    .run(tauri::generate_context!())?;
```

Use `#[tauri::command(rename_all = "camelCase")]` or
`#[tauri::command(rename_all = "snake_case")]` to convert JSON argument keys.
This affects payload arguments, including borrowed strings; injected `State`,
`AppHandle` and `Window` parameters are unchanged. Omitting the option preserves
the Rust parameter spelling with any `r#` prefix removed for compatibility with
existing adapter callers. Upstream Tauri defaults to camelCase, so specify it
explicitly when migrating those payloads. Both explicit modes use the same
[case-conversion library as upstream Tauri](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-macros/src/command/wrapper.rs).

Unsupported command options, duplicate options and malformed values now fail
compilation instead of being ignored. This includes the bare `async` option;
`async fn` commands and their invocation routes remain supported as described below.
Distinct payload parameters that convert to the same JSON key also fail
compilation; injected parameters do not claim payload keys.

`crepus tauri audit --dir <project>` classifies Tauri surfaces before native conversion. `crepus tauri convert` only generates SwiftUI and Compose output when every detected surface has a native adapter.

For native conversion, a bundle entry may select a named component, such as
`ui.crepus#Card`, from the `ui.crepus` file.

`app.invoke(name, payload)` runs on the caller's thread and blocks until the
handler returns. For invocation from an async task, use
`app.invoke_async(name, payload).await`: it runs the registered handler on the
adapter's blocking pool while the caller's runtime remains free to progress.
This also supports async commands with borrowed arguments or non-`Send` local
futures; the command future is created on that worker. Handlers on this route
have no host-thread affinity. Command errors are preserved and worker panics
return errors.

Synchronous `invoke` of an async macro command from an async task still reaches
Tokio's nested `block_on` restriction. The generic `block_on` helper retains its
existing synchronous contract; use `invoke_async` for this situation.
