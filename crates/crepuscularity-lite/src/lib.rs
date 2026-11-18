//! **crepuscularity-lite** — embed a V8 guest + Rust native-capability bridge inside a GPUI (or other) host.
//!
//! Development can run TypeScript / TSX guest entries through the built-in Oxc transpiler. Production
//! bundling, chunking, and code splitting are still the embedder’s or CLI toolchain’s job; use the
//! `cl build` esbuild path for bundled output, or multiple sources with
//! [`config::CrepusLiteConfig::guest_prelude`] / separate worker scripts (see `docs/THREADING.md`).
//!
//! ## How this relates to GPUI and Rust
//!
//! - **Rust plugins** are normal Rust: types implementing [`NativePlugin`]. They are registered on a [`Bridge`] and reached from JS via `Crepus.invoke(...)`.
//! - **GPUI is not “hooked” automatically.** V8 does not register GPUI `actions!`, keymaps, or subscriptions for you. You call [`V8Host::eval`](crate::V8Host) (or run scripts) from *your* GPUI event closures—the same places you would call any other Rust code.
//! - **Window / UI-thread work** from plugins is deferred (e.g. `HostDeferred::SetWindowTitle`). After guest code runs, call [`integration::apply_window_deferred`] with a live GPUI `Window` so those operations hit the real window.
//! - **Hot reload:** [`guest_watch::spawn_guest_file_watcher`] watches guest file paths; the callback must be `Send` and should only signal the UI thread (see `docs/THREADING.md` and the package binary).
//! - **Bridge in V8:** each [`V8Host`] installs an embedder slot on the isolate’s context (`CrepusBridgeSlot` in `v8_host.rs`); **`Crepus.invoke`** reads the bridge from the current context (no process-global `ACTIVE_BRIDGE`).
//! - **Optional TOML caps:** [`config::CrepusLiteConfig::build_bridge`] respects `[capabilities]` (see `examples/*/crepus-lite.example.toml`).
//!
//! For a full demo, run the package binary (see `src/main.rs`).

#![allow(rustdoc::broken_intra_doc_links)]
pub mod bench_plugin;
pub mod bridge;
pub mod clipboard;
pub mod download_plugin;
mod fs_paths;
pub mod host;
pub mod host_queue;
pub mod inauguration_plugin;
pub mod integration;
pub mod plugins;

pub mod config;
pub mod guest_watch;

#[cfg(all(feature = "v8", feature = "oxc"))]
pub mod bench_eval;
#[cfg(feature = "oxc")]
pub mod guest_compiler;
#[cfg(feature = "v8")]
pub mod v8_host;
#[cfg(feature = "v8")]
pub mod v8_thread;
#[cfg(feature = "v8")]
pub mod worker;

#[cfg(all(feature = "v8", feature = "oxc"))]
pub use bench_eval::{
    bench_matrix_shared_for_configs, eval_guest_from_config_file, BenchMatrixShared,
};
pub use bench_plugin::BenchPlugin;
pub use bridge::{Bridge, BridgeError, Capability, NativePlugin};
pub use download_plugin::DownloadPlugin;
#[cfg(feature = "oxc")]
pub use guest_compiler::prepare_guest_source;
pub use host::{HostEventRecord, HostNode, HostRoute, HostSnapshot, HostState, HostStyle};
pub use host_queue::{DeferredWindowDecorations, HostCommandQueue, HostDeferred};
pub use inauguration_plugin::InaugurationPlugin;
pub use plugins::{AppPlugin, ClipboardPlugin, CorePlugin, FsPlugin, HostPlugin, WindowPlugin};
#[cfg(feature = "v8")]
pub use v8_host::V8Host;
#[cfg(feature = "v8")]
pub use v8_thread::{V8ThreadHandle, V8ThreadRequest, V8ThreadRuntime};
#[cfg(feature = "v8")]
pub use worker::{WorkerHandle, WorkerRuntime};

pub fn parse_hex_color(raw: &str) -> Option<gpui::Hsla> {
    use palette::IntoColor;
    let hex = raw.trim().trim_start_matches('#');
    let parsed = match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok().map(gpui::rgb),
        _ => None,
    }?;
    Some(parsed.into_color())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert!(parse_hex_color("#ff0000").is_some());
        assert!(parse_hex_color("00ff00").is_some());
        assert!(parse_hex_color(" #0000ff ").is_some());

        assert!(parse_hex_color("#ff0").is_none());
        assert!(parse_hex_color("ff00000").is_none());
        assert!(parse_hex_color("#xyz123").is_none());
        assert!(parse_hex_color("").is_none());
        assert!(parse_hex_color("   ").is_none());
    }
}
