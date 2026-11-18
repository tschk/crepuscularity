//! Debounced filesystem watch for a guest script path. The callback runs on the **debouncer** thread
//! (see `notify-debouncer-mini`); schedule GPUI / UI-thread work yourself (for example via
//! [`gpui::AsyncApp::foreground_executor`]).
//!
//! Hot reload policy: recreate [`crate::V8Host`] before re-`eval` so globals do not survive across
//! reloads (see the package binary).

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};

/// Start watching `guest_file`'s parent directory (non-recursive). On each debounced change that
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

    let parent = guest_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let target = guest_file
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", guest_file.display()))?;

    let fire = on_changed.clone();
    let mut debouncer = new_debouncer(debounce, move |res: DebounceEventResult| match res {
        Ok(events) => {
            for ev in events {
                if paths_refer_to_same_or_overlap(&ev.path, &target) {
                    fire();
                    break;
                }
            }
        }
        Err(e) => eprintln!("crepus-lite guest watch: {e}"),
    })
    .map_err(|e| format!("debouncer: {e}"))?;

    debouncer
        .watcher()
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(|e| format!("watch {}: {e}", parent.display()))?;

    thread::Builder::new()
        .name("crepus-lite-guest-watch".into())
        .spawn(move || {
            let _keep_alive = debouncer;
            thread::park();
        })
        .map_err(|e| format!("spawn guest watcher thread: {e}"))?;

    Ok(())
}

fn paths_refer_to_same_or_overlap(event_path: &Path, guest: &Path) -> bool {
    if event_path == guest {
        return true;
    }
    match (
        std::fs::canonicalize(event_path),
        std::fs::canonicalize(guest),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
