# Browser Extensions

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md)

> **Repository-only detail:** Extended web + webext notes for compiler authors live in [`CREPUS_WEB_IMPLEMENTATION_SPEC.md` on GitHub](https://github.com/softwarecorporation/crepuscularity/blob/main/docs/CREPUS_WEB_IMPLEMENTATION_SPEC.md) (see **§9** for MV3). This file is not part of the published docs site.

The `crepuscularity-webext` crate provides support for building Chromium, Firefox, and Safari extensions with Manifest V3.

## Quick Start

```bash
crepus webext new my-extension
cd my-extension
crepus build
# Load dist/unpacked/ in chrome://extensions

crepus webext build --browser firefox
# Load dist/firefox/manifest.json in about:debugging#/runtime/this-firefox

crepus webext build --browser safari
```

## Configuration

Extensions are configured via `crepus.toml`:

```toml
[[targets]]
type = "webext"
id = "extension"
app = "."
browsers = ["chromium", "firefox", "safari"]

[targets.extension]
name = "My Extension"
version = "1.0.0"
description = "A browser extension built with crepuscularity"

[targets.capabilities]
storage = true
background-script = true
content-script = true
host-permissions = ["https://example.com/*"]

[targets.safari]
bundle_identifier = "com.example.my-extension"
project_location = "dist/safari-app"
platforms = ["macos", "ios"]
```

`host-permissions` is emitted exactly as configured. Leave it empty for popup-only extensions with no page access, or add the narrow URL patterns your content scripts need. Crepuscularity does not broaden an empty list to `<all_urls>`.

### Capabilities

| Capability | Description |
|------------|-------------|
| `storage` | Access to `chrome.storage` API |
| `background-script` | Service worker script |
| `content-script` | Scripts injected into pages |
| `host-permissions` | Array of URL patterns |
| `tabs` | Access to `chrome.tabs` API |
| `notifications` | Desktop notifications |
| `context-menus` | Right-click context menus |
| `alarms` | Scheduled tasks |

## Project Structure

```
my-extension/
├── crepus.toml        # Extension target and configuration
├── runtime/           # Rust WASM runtime
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── views/             # .crepus templates
│   └── ui.crepus
└── dist/              # Build output
    └── unpacked/
        ├── manifest.json
        ├── popup.html
        └── ...
```

## Manifest Generation

The CLI generates a Chrome Manifest V3 from `crepus.toml`:

```bash
crepus webext manifest
crepus webext manifest --browser firefox
```

Output:
```json
{
  "manifest_version": 3,
  "name": "My Extension",
  "version": "1.0.0",
  "description": "A browser extension built with crepuscularity",
  "permissions": ["storage"],
  "host_permissions": ["https://example.com/*"],
  "background": {
    "service_worker": "src/background.js",
    "type": "module"
  },
  "content_scripts": [{
    "matches": ["https://example.com/*"],
    "js": ["src/content.js"]
  }],
  "content_security_policy": {
    "extension_pages": "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"
  }
}
```

`crepus webext build --browser chromium` writes `dist/chromium/`; `--browser firefox` writes `dist/firefox/`; `--browser safari` writes `dist/safari/` then generates the configured Xcode project. Without `--browser`, `browsers` builds each configured target; omit `browsers` to retain `dist/unpacked/` for the existing Chromium load flow. Safari packaging requires macOS with Xcode and a `bundle_identifier`; `platforms` accepts `["macos"]`, `["ios"]`, or `["macos", "ios"]`.

## Positioning

Crepuscularity is not trying to be only an extension framework. Treat webext as the browser-extension target inside a broader `.crepus` systems UI pipeline: the same source language can also target GPUI, Ratatui, web output, View IR for native shells, embedded framebuffers, and LVGL Pro.

Oxichrome is stronger when the job is “write the whole extension as a Leptos-style Rust app with proc-macro entrypoints.” Crepuscularity should compete where the target is one compact UI language, Rust-owned build/runtime plumbing, and multiple non-browser surfaces from the same template model. The webext target should keep improving around typed browser APIs, generated entrypoint glue, and browser-specific bundles while preserving least-privilege manifests.

## WASM Runtime

Extensions can include a Rust WASM runtime for complex logic:

```rust
// runtime/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process_data(input: &str) -> String {
    // Complex processing in Rust
    format!("Processed: {}", input)
}
```

Build with:
```bash
crepus webext build
```

Debug builds run `wasm-bindgen` and skip `wasm-opt` by default. Release builds run `wasm-opt -O2` on `runtime_bg.wasm` when Binaryen is installed; use `--opt-level none|fast|size|aggressive` to override the post-build optimization level.

## Templates

Use `.crepus` templates for extension UI:

```text
# views/popup.crepus
div popup flex flex-col gap-4 p-4
  h1 text-xl font-bold
    "My Extension"
  button px-4 py-2 bg-blue-500 text-white rounded @click=handle_action
    "Do Something"
```

## Capability Auto-Detection

The `crepuscularity-webext` scanner detects API usage in templates and suggests capabilities:

```rust
use crepuscularity_webext::{scan_crepus_for_capabilities, CapabilityUsage};

let usage = scan_crepus_for_capabilities("views/popup.crepus")?;
for cap in usage.suggested {
    println!("Suggest adding: {}", cap);
}
```

## API Reference

### Browser APIs

With the `wasm` feature enabled, `crepuscularity_webext::wasm` exposes async Rust wrappers around the browser namespace. The reference target is the same practical surface Oxichrome documents today: storage, runtime messaging, and tabs.

```rust
use crepuscularity_webext::wasm::{runtime, storage, tabs};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Settings {
    theme: String,
}

async fn load_settings() -> crepuscularity_webext::wasm::Result<Option<Settings>> {
    storage::get("settings").await
}

async fn save_settings(settings: &Settings) -> crepuscularity_webext::wasm::Result<()> {
    storage::set("settings", settings).await
}

async fn open_docs() -> crepuscularity_webext::wasm::Result<()> {
    let _tab = tabs::create(&tabs::CreateProperties {
        url: Some(runtime::get_url("docs.html")?),
        active: Some(true),
        ..Default::default()
    })
    .await?;
    Ok(())
}
```

Use area-specific storage when needed:

```rust
let count: Option<i32> = storage::sync().get_key("count").await?;
storage::session().set_key("draft", &serde_json::json!({"open": true})).await?;
storage::remove("settings").await?;
```

### BrowserProgram

Fluent builder for generating extension JavaScript:

```rust
use crepuscularity_webext::{BrowserProgram, StorageArea, JsExpr};

let program = BrowserProgram::new()
    .storage_get(StorageArea::Local, "key", "value")
    .storage_set(StorageArea::Sync, "key", JsExpr::literal("value"))
    .send_message("action", serde_json::json!({"data": "test"}))
    .finish();

println!("{}", program.to_js());
```

### ExtensionManifest

Use `crepus.toml` for project builds. The lower-level `ExtensionManifest` type remains available when you already have extension metadata in Rust:

```rust
use crepuscularity_webext::ExtensionManifest;

let manifest: ExtensionManifest = toml::from_str(source)?;
println!("{}", manifest.to_manifest_v3_json());
```
