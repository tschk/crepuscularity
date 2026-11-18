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

`crepus tauri audit --dir <project>` classifies Tauri surfaces before native conversion. `crepus tauri convert` only generates SwiftUI and Compose output when every detected surface has a native adapter.
