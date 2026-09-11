//! Lower `.crepus` templates to a JSON view tree for SwiftUI, Jetpack Compose, and future
//! `android-activity` / `objc2` shells.
//!
//! ## Crate layout
//! - [`ir`] — serializable `ViewIr` / `ViewNode` / `ViewStyle` (the **contract** with shells).
//! - [`style`] — Tailwind-like class → style hints (extend here for GPUI parity).
//! - `include_expand` — `include` → `(Vec<Node>, TemplateContext)` without IR cycles.
//! - `render` — AST lowering and public `render_*` entry points.
//! - [`mutations`] — renderer-agnostic, path-based IR mutations and diffing.
//! - [`hot_reload`] — structured hot-reload protocol + conservative AST gate.
//!
//! ## Sharing / codegen (bindgen, other repos)
//! - **[`bindgen`](https://docs.rs/bindgen)** generates **Rust FFI** from **C/C++ headers**. It does
//!   not extract types from Rust for Swift/Kotlin, and it does not import arbitrary “other project”
//!   sources—it wraps existing native (C) APIs.
//! - **Single source of truth** options that *do* fit native shells:
//!   - **`schemars`** on `ViewIr` + **JSON Schema** → *quicktype*, *OpenAPI generators*, etc. for Swift/Kotlin.
//!   - A tiny **build.rs** that emits `fixture.json` and optional bindings (same repo or **path dependency** / **git submodule**).
//!   - Publish **`crepuscularity-native`** to crates.io and depend on it from another Rust workspace as usual.
//!
//! ## Coverage vs GPUI / web
//! Not 100% parity with GPUI `styler.rs`—expand [`style`].

pub mod capabilities;
pub mod codegen;
pub mod colors;
pub mod host;
pub mod hot_reload;
mod include_expand;
pub mod ir;
pub mod js_expr;
pub mod moonshine;
pub mod mutations;
pub mod native;
mod render;
pub mod solid;
pub mod style;
pub mod svelte;
mod utils;
pub mod vue;

pub use capabilities::{
    ANDROID_ACCESSIBILITY_INFO, ANDROID_ACTION_SHEET, ANDROID_APP, ANDROID_APPEARANCE,
    ANDROID_APP_STATE, ANDROID_BATTERY, ANDROID_BIOMETRICS, ANDROID_BLUETOOTH,
    ANDROID_BLUETOOTH_BRIDGE, ANDROID_BROWSER, ANDROID_CALENDAR, ANDROID_CAMERA, ANDROID_CLIPBOARD,
    ANDROID_CONTACTS, ANDROID_DEEP_LINKS, ANDROID_DEVICE, ANDROID_DIALOG, ANDROID_DIMENSIONS,
    ANDROID_DOCUMENT_PICKER, ANDROID_GEOLOCATION, ANDROID_GEOLOCATION_BRIDGE, ANDROID_HAPTICS,
    ANDROID_IMAGE_PICKER, ANDROID_IN_APP_BROWSER, ANDROID_KEYBOARD, ANDROID_LOCAL_NOTIFICATIONS,
    ANDROID_MICROPHONE, ANDROID_NETWORK, ANDROID_PERMISSIONS, ANDROID_PHOTO_LIBRARY,
    ANDROID_PREFERENCES, ANDROID_PRIVACY_SCREEN, ANDROID_SCHEDULED_NOTIFICATION_RECEIVER,
    ANDROID_SCREEN_ORIENTATION, ANDROID_SECURE_STORAGE, ANDROID_SENSORS, ANDROID_SENSORS_BRIDGE,
    ANDROID_SETTINGS, ANDROID_SHARE, ANDROID_SYSTEM_BARS, ANDROID_TOAST, ANDROID_VIDEO,
    IOS_ACCESSIBILITY_INFO, IOS_ACTION_SHEET, IOS_ACTION_SHEET_BRIDGE, IOS_APP, IOS_APPEARANCE,
    IOS_APP_STATE, IOS_BATTERY, IOS_BIOMETRICS, IOS_BLUETOOTH, IOS_BLUETOOTH_BRIDGE, IOS_BROWSER,
    IOS_CALENDAR, IOS_CAMERA, IOS_CAMERA_BRIDGE, IOS_CLIPBOARD, IOS_CLIPBOARD_BRIDGE, IOS_CONTACTS,
    IOS_DEEP_LINKS, IOS_DEVICE, IOS_DIALOG, IOS_DIALOG_BRIDGE, IOS_DIMENSIONS, IOS_DOCUMENT_PICKER,
    IOS_GEOLOCATION, IOS_GEOLOCATION_BRIDGE, IOS_HAPTICS, IOS_IMAGE_PICKER,
    IOS_IMAGE_PICKER_BRIDGE, IOS_IN_APP_BROWSER, IOS_KEYBOARD, IOS_LOCAL_NOTIFICATIONS,
    IOS_MICROPHONE, IOS_NETWORK, IOS_PERMISSIONS, IOS_PHOTO_LIBRARY, IOS_PHOTO_LIBRARY_BRIDGE,
    IOS_PREFERENCES, IOS_PRIVACY_SCREEN, IOS_SCREEN_ORIENTATION, IOS_SECURE_STORAGE, IOS_SENSORS,
    IOS_SENSORS_BRIDGE, IOS_SETTINGS, IOS_SHARE, IOS_SYSTEM_BARS, IOS_TOAST, IOS_VIDEO,
    IOS_VIDEO_BRIDGE,
};
pub use codegen::{generate_native_source, NativeCodegenTarget};
pub use colors::resolve_rgba;
pub use crepuscularity_core::CrepusError;
pub use hot_reload::{ast_shape_compatible, plan_hot_reload, HotReloadEnvelope, HotReloadMessage};
pub use ir::{PickerOption, StackAxis, ViewIr, ViewNode, ViewStyle, IR_VERSION};
pub use moonshine::{emit_moonshine_app, emit_moonshine_component};
pub use mutations::{apply_mutations, diff_ir, IrMutation};
pub use native::{
    FilePickerRequest, FilePickerResponse, NativeCapability, NativePluginRequest,
    NativePluginResponse, NativeRequest, NativeResponse, PickedFile, NATIVE_CAPABILITIES,
};
pub use render::{
    render_component_file_to_ir, render_from_files, render_nodes_to_ir, render_template_to_ir,
    render_template_to_ir_with_path,
};
pub use solid::emit_solid_component;
pub use svelte::emit_svelte_component;
pub use vue::emit_vue_component;

/// Serialize IR to JSON (compact).
pub fn to_json(ir: &ViewIr) -> Result<String, serde_json::Error> {
    serde_json::to_string(ir)
}

/// Serialize IR to pretty-printed JSON (fixtures / debugging).
pub fn to_json_pretty(ir: &ViewIr) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(ir)
}

#[cfg(test)]
mod tests;
