//! Debounced filesystem watch for a guest script path. The callback runs on the **debouncer** thread
//! (see `notify-debouncer-mini`); schedule GPUI / UI-thread work yourself (for example via
//! [`gpui::AsyncApp::foreground_executor`]).
//!
//! Hot reload policy: recreate [`crate::V8Host`] before re-`eval` so globals do not survive across
//! reloads (see the package binary).

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crepuscularity_core::watch::SingleFileWatcher;
use notify_debouncer_mini::{
    new_debouncer_opt, notify::RecursiveMode, Config, DebounceEventResult,
};

/// Watch the logical `guest_file`, following symlink retargets. On each debounced change that
/// targets this path, `on_changed` is called from the debouncer thread.
///
/// A background thread keeps the debouncer alive for the rest of the process.
pub fn spawn_guest_file_watcher(
    guest_file: PathBuf,
    debounce: Duration,
    on_changed: impl Fn() + Send + Clone + 'static,
) -> Result<(), String> {
    if !guest_file.exists() {
        return Err(format!(
            "guest file does not exist: {}",
            guest_file.display()
        ));
    }

    guest_file
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", guest_file.display()))?;

    let fire = on_changed.clone();
    let mut debouncer = new_debouncer_opt::<_, SingleFileWatcher>(
        Config::default().with_timeout(debounce),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                if !events.is_empty() {
                    fire();
                }
            }
            Err(e) => eprintln!("crepus-lite guest watch: {e}"),
        },
    )
    .map_err(|e| format!("debouncer: {e}"))?;

    debouncer
        .watcher()
        .watch(&guest_file, RecursiveMode::NonRecursive)
        .map_err(|e| format!("watch {}: {e}", guest_file.display()))?;

    thread::Builder::new()
        .name("crepus-lite-guest-watch".into())
        .spawn(move || {
            let _keep_alive = debouncer;
            thread::park();
        })
        .map_err(|e| format!("spawn guest watcher thread: {e}"))?;

    Ok(())
}
