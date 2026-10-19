//! Shared file-watch primitives for all Crepuscularity dev servers.
//!
//! Every backend that hot-reloads `.crepus` files should use these constants
//! to keep debounce behaviour consistent across GPUI, web, and webext targets.

/// Minimum interval (ms) between consecutive rebuild triggers.
///
/// After the first file change event arrives, subsequent events within this
/// window are coalesced into a single rebuild. This avoids churn when an
/// editor writes several files in quick succession (e.g. `include`d
/// components together with the entry template).
pub const DEBOUNCE_MS: u64 = 50;

/// Cooldown period (ms) after the last received event before a rebuild fires.
///
/// The watcher waits this long after the most recent event before starting a
/// rebuild, so rapid successive saves only produce one rebuild.
pub const COOLDOWN_MS: u64 = 200;

/// Poll interval (ms) for extension-page hot reload (`dev.js`).
///
/// The content script polls `.reload-id` at this cadence. Must be fast
/// enough to feel instant but slow enough to avoid excessive network
/// requests inside the extension sandbox.
pub const EXTENSION_POLL_MS: u64 = 1500;

#[cfg(feature = "notify")]
mod managed;


/// Create a template watcher for the logical directory and resolved entry target.
///
/// A background monitor reconciles roots when entry or ancestor symlinks change.
/// Native file events remain active; dropping the handle signals worker teardown
/// without joining the monitor on the caller's thread.
///
/// The watcher flips `*changed = true` when:
/// - the watched template is modified / created / removed (covers
///   atomic-save editor patterns), or
/// - any sibling/descendant supported template file changes (`include`d components
///   trigger reload), or
/// - a `context.toml` next to the template is updated.
///
/// `label` appears in error messages to distinguish the source (e.g.
/// `"runtime"`, `"tui"`). Drop the returned handle to stop watching cleanly.
#[cfg(feature = "notify")]
pub fn create_watcher(
    path: impl Into<std::path::PathBuf>,
    changed: std::sync::Arc<std::sync::Mutex<bool>>,
    label: &'static str,
) -> Result<Box<dyn notify::Watcher + Send>, String> {
    managed::create(path.into(), changed, label)
}

#[cfg(feature = "notify")]
pub fn handle_watcher_event(
    res: notify::Result<notify::Event>,
    changed: &std::sync::Arc<std::sync::Mutex<bool>>,
    target: &std::path::Path,
    root: &std::path::Path,
    label: &str,
) {
    match res {
        Ok(ev) => {
            if !is_relevant_kind(&ev.kind) {
                return;
            }
            if !event_touches_relevant_path(&ev, target, root) {
                return;
            }
            if let Ok(mut flag) = changed.lock() {
                *flag = true;
            }
        }
        Err(e) => eprintln!("[crepuscularity-{label}] watcher error: {e}"),
    }
}

/// Returns true for Modify, Create, or Remove event kinds.
#[cfg(feature = "notify")]
pub fn is_relevant_kind(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Modify(_) | notify::EventKind::Create(_) | notify::EventKind::Remove(_)
    )
}

#[cfg(feature = "notify")]
pub fn event_touches_relevant_path(
    event: &notify::Event,
    target: &std::path::Path,
    watch_root: &std::path::Path,
) -> bool {
    for path in &event.paths {
        if path == target {
            return true;
        }
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if canon == *target {
            return true;
        }
        let canon_root = watch_root
            .canonicalize()
            .unwrap_or_else(|_| watch_root.to_path_buf());
        if !canon.starts_with(&canon_root) {
            continue;
        }
        match canon.extension().and_then(|e| e.to_str()) {
            Some("crepus" | "vue" | "svelte" | "astro" | "ng" | "jsx" | "tsx" | "csx") => {
                return true
            }
            Some("html")
                if canon
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(".ng.html") || name.ends_with(".component.html")
                    }) =>
            {
                return true
            }
            Some("toml") if canon.file_name().and_then(|n| n.to_str()) == Some("context.toml") => {
                return true
            }
            _ => continue,
        }
    }
    false
}

#[cfg(test)]
#[cfg(feature = "notify")]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};
    use notify::EventKind;

    #[test]
    fn included_frontends_trigger_reload() {
        let root = std::path::Path::new("/crepus-watcher-test");
        let target = root.join("entry.crepus");
        for extension in [
            "vue",
            "svelte",
            "astro",
            "ng",
            "ng.html",
            "component.html",
            "jsx",
            "tsx",
            "csx",
        ] {
            let event = notify::Event::new(EventKind::Remove(RemoveKind::Any))
                .add_path(root.join(format!("child.{extension}")));
            assert!(
                event_touches_relevant_path(&event, &target, root),
                "{extension}"
            );
        }
        let event = notify::Event::new(EventKind::Modify(ModifyKind::Any))
            .add_path(root.join("index.html"));
        assert!(!event_touches_relevant_path(&event, &target, root));
    }

    #[test]
    fn test_is_relevant_kind() {
        assert!(is_relevant_kind(&EventKind::Modify(ModifyKind::Any)));
        assert!(is_relevant_kind(&EventKind::Create(CreateKind::Any)));
        assert!(is_relevant_kind(&EventKind::Remove(RemoveKind::Any)));

        assert!(!is_relevant_kind(&EventKind::Access(AccessKind::Any)));
        assert!(!is_relevant_kind(&EventKind::Any));
        assert!(!is_relevant_kind(&EventKind::Other));
    }
}
