//! Lower `.crepus` templates to a JSON view tree for SwiftUI, Jetpack Compose, and future
//! `android-activity` / `objc2` shells.
//!
//! ## Crate layout
//! - [`ir`] — serializable `ViewIr` / `ViewNode` / `ViewStyle` (the **contract** with shells).
//! - [`style`] — Tailwind-like class → style hints (extend here for GPUI parity).
//! - `include_expand` — child nodes/context and a guard that bounds nested includes.
//! - `render` — AST lowering and public `render_*` entry points.
//! - [`mutations`] — renderer-agnostic, path-based IR mutations and diffing.
//! - [`to_json_owned`] — compact encoding with iterative child traversal for deep IR.
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
pub mod imba;
mod include_expand;
pub mod ir;
pub mod moonshine;
pub mod moonshine_dom;
#[cfg(feature = "web-modules")]
pub mod moonshine_module;
pub mod mutations;
pub mod native;
mod render;
mod scoped_expression;
pub mod solid;
#[cfg(feature = "web-modules")]
mod solid_component_resolver;
#[cfg(feature = "web-modules")]
mod solid_lists;
#[cfg(feature = "web-modules")]
mod solid_source;
#[cfg(feature = "web-modules")]
mod source_globals;
pub mod style;
pub mod svelte;
#[cfg(feature = "web-modules")]
mod svelte_attributes;
#[cfg(feature = "web-modules")]
mod svelte_await;
#[cfg(feature = "web-modules")]
mod svelte_bindings;
#[cfg(feature = "web-modules")]
mod svelte_groups;
#[cfg(feature = "web-modules")]
mod svelte_boundary;
#[cfg(feature = "web-modules")]
mod svelte_children;
#[cfg(feature = "web-modules")]
mod svelte_class;
#[cfg(feature = "web-modules")]
mod svelte_component_props;
#[cfg(feature = "web-modules")]
mod svelte_component_references;
#[cfg(feature = "web-modules")]
mod svelte_component_values;
#[cfg(feature = "web-modules")]
mod svelte_components;
#[cfg(feature = "web-modules")]
mod svelte_const;
#[cfg(feature = "web-modules")]
mod svelte_const_emit;
#[cfg(feature = "web-modules")]
mod svelte_css;
#[cfg(feature = "web-modules")]
mod svelte_directives;
#[cfg(feature = "web-modules")]
mod svelte_each;
#[cfg(feature = "web-modules")]
mod svelte_effect;
#[cfg(feature = "web-modules")]
use crepuscularity_core::parser::svelte_entities;
#[cfg(feature = "web-modules")]
mod svelte_events;
#[cfg(feature = "web-modules")]
mod svelte_exports;
#[cfg(feature = "web-modules")]
mod svelte_expression;
#[cfg(feature = "web-modules")]
mod svelte_html;
#[cfg(feature = "web-modules")]
mod svelte_intrinsics;
#[cfg(feature = "web-modules")]
mod svelte_javascript;
#[cfg(feature = "web-modules")]
mod svelte_module;
#[cfg(feature = "web-modules")]
mod svelte_mount;
#[cfg(feature = "web-modules")]
mod svelte_mount_types;
#[cfg(feature = "web-modules")]
mod svelte_pattern;
#[cfg(feature = "web-modules")]
mod svelte_props;
#[cfg(feature = "web-modules")]
mod svelte_scheduling;
#[cfg(feature = "web-modules")]
mod svelte_script;
#[cfg(feature = "web-modules")]
mod svelte_snippets;
#[cfg(feature = "web-modules")]
mod svelte_spreads;
#[cfg(feature = "web-modules")]
mod svelte_store;
#[cfg(feature = "web-modules")]
mod svelte_text;
#[cfg(feature = "web-modules")]
mod svelte_head;
#[cfg(feature = "web-modules")]
mod svelte_element;
#[cfg(feature = "web-modules")]
pub use svelte_module::{compile_moonshine_svelte, rewrite_moonshine_imports};
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
pub use hot_reload::{
    ast_shape_compatible, plan_hot_reload, plan_hot_reload_with_path, HotReloadEnvelope,
    HotReloadMessage,
};
pub use imba::{emit_imba_component, ImbaComponent};
pub use ir::{LabelPart, PickerOption, StackAxis, ViewIr, ViewNode, ViewStyle, IR_VERSION};
/// Emit an importable React TSX component with optional scope and handler props.
pub use moonshine::emit_moonshine_component as emit_react_component;
pub use moonshine::{emit_moonshine_app, emit_moonshine_component};
pub use moonshine_dom::{emit_moonshine_dom_component, emit_moonshine_ssr_component};
#[cfg(feature = "web-modules")]
pub use moonshine_module::{
    compile_moonshine_module, compile_moonshine_module_with_resolver,
    try_compile_moonshine_module, try_compile_moonshine_module_with_resolver, MoonshineModule,
    MoonshineModuleLanguage, MoonshineModuleTarget,
};
#[cfg(feature = "web-modules")]
pub use solid_component_resolver::SolidComponentModule;
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

mod json_owned;
pub use json_owned::to_json_owned;

/// Serialize IR to pretty-printed JSON (fixtures / debugging).
pub fn to_json_pretty(ir: &ViewIr) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(ir)
}

#[cfg(test)]
mod tests;

#[cfg(feature = "web-modules")]
mod vue_module;
#[cfg(feature = "web-modules")]
pub use vue_module::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, VueTypeModule, VueTypeResolver,
};

#[cfg(feature = "web-modules")]
mod astro_module;
#[cfg(feature = "web-modules")]
pub use astro_module::compile_moonshine_astro;
