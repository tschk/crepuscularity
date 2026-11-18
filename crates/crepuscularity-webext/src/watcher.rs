//! File watcher for auto-detecting capability changes.
//!
//! Watches .crepus files and webext.toml, emitting events when
//! capabilities change or new ones are detected.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};

use crate::capabilities::{Capability, CapabilitySet};
use crate::manifest::ExtensionManifest;
use crate::scanner::scan_directory_for_capabilities;

/// Events emitted by the capability watcher.
#[derive(Clone, Debug)]
pub enum WatchEvent {
    /// A file was changed.
    FileChanged { path: PathBuf },

    /// New capabilities were detected that aren't in the manifest.
    MissingCapabilities {
        capabilities: Vec<Capability>,
        manifest_path: PathBuf,
    },

    /// The manifest was updated.
    ManifestUpdated { path: PathBuf },

    /// An error occurred.
    Error { message: String },
}

/// Watches files for capability changes.
pub struct CapabilityWatcher {
    rx: Receiver<WatchEvent>,
    _watcher: Box<dyn Watcher + Send>,
}

impl CapabilityWatcher {
    /// Create a new watcher for the given project directory.
    ///
    /// Watches:
    /// - `**/*.crepus` files for API usage in expression slots
    /// - `webext.toml` for declared capabilities
    pub fn new(project_dir: &Path) -> Result<Self, notify::Error> {
        let (tx, rx) = channel();
        let project_dir = project_dir.to_path_buf();

        let tx_clone = tx.clone();
        let project_dir_clone = project_dir.clone();

        let mut watcher = recommended_watcher(move |res: notify::Result<Event>| match res {
            Ok(event) => handle_event(event, &project_dir_clone, &tx_clone),
            Err(e) => {
                let _ = tx_clone.send(WatchEvent::Error {
                    message: format!("watcher error: {e}"),
                });
            }
        })?;

        // Watch the entire project directory recursively
        watcher.watch(&project_dir, RecursiveMode::Recursive)?;

        Ok(Self {
            rx,
            _watcher: Box::new(watcher),
        })
    }

    /// Receive the next event, blocking until one is available.
    pub fn recv(&self) -> Option<WatchEvent> {
        self.rx.recv().ok()
    }

    /// Try to receive an event with a timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<WatchEvent> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Get the receiver for custom event handling.
    pub fn receiver(&self) -> &Receiver<WatchEvent> {
        &self.rx
    }
}

fn handle_event(event: Event, project_dir: &Path, tx: &Sender<WatchEvent>) {
    // Accept Remove too: editor "atomic saves" (write-temp-then-rename) deliver
    // a Remove on the original path before a Modify/Create on the new file. If
    // we drop Remove, capability scanning misses the save on Linux/inotify
    // because the underlying inode is gone.
    match event.kind {
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
            let mut scan = false;
            for path in event.paths {
                let ext = path.extension().and_then(|e| e.to_str());
                let file_name = path.file_name().and_then(|n| n.to_str());

                if path.is_dir() {
                    // Moving a populated directory may report only its parent path.
                    scan = true;
                } else if ext == Some("crepus") {
                    let _ = tx.send(WatchEvent::FileChanged { path: path.clone() });
                    scan = true;
                } else if file_name == Some("webext.toml") {
                    let _ = tx.send(WatchEvent::ManifestUpdated { path });
                }
            }
            if scan {
                if let Err(e) = check_capabilities(project_dir, tx) {
                    let _ = tx.send(WatchEvent::Error {
                        message: format!("Failed to check capabilities: {e}"),
                    });
                }
            }
        }
        _ => {}
    }
}

fn check_capabilities(project_dir: &Path, tx: &Sender<WatchEvent>) -> Result<(), String> {
    let manifest_path = project_dir.join("webext.toml");

    // Load manifest if it exists
    let declared_caps = if manifest_path.exists() {
        ExtensionManifest::load(&manifest_path)
            .map(|m| m.to_capability_set())
            .map_err(|e| format!("Failed to load manifest: {e}"))?
    } else {
        CapabilitySet::new()
    };

    // Scan source files
    let used_caps =
        scan_directory_for_capabilities(project_dir).map_err(|e| format!("Scan failed: {e}"))?;

    // Find missing capabilities
    let missing: Vec<Capability> = used_caps
        .missing_from(&declared_caps)
        .into_iter()
        .cloned()
        .collect();

    if !missing.is_empty() {
        let _ = tx.send(WatchEvent::MissingCapabilities {
            capabilities: missing,
            manifest_path,
        });
    }

    Ok(())
}

pub fn check_project_capabilities_with_manifest(
    project_dir: &Path,
    manifest: &ExtensionManifest,
) -> Result<Vec<Capability>, String> {
    let used_caps =
        scan_directory_for_capabilities(project_dir).map_err(|e| format!("Scan failed: {e}"))?;
    Ok(used_caps
        .missing_from(&manifest.to_capability_set())
        .into_iter()
        .cloned()
        .collect())
}

/// Check a project for missing capabilities (one-shot, no watching).
pub fn check_project_capabilities(project_dir: &Path) -> Result<Vec<Capability>, String> {
    let manifest_path = project_dir.join("webext.toml");

    let declared_caps = if manifest_path.exists() {
        ExtensionManifest::load(&manifest_path)
            .map(|m| m.to_capability_set())
            .map_err(|e| format!("Failed to load manifest: {e}"))?
    } else {
        CapabilitySet::new()
    };

    let used_caps =
        scan_directory_for_capabilities(project_dir).map_err(|e| format!("Scan failed: {e}"))?;

    Ok(used_caps
        .missing_from(&declared_caps)
        .into_iter()
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn test_watcher_creation() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let watcher = CapabilityWatcher::new(dir.path())?;
        assert!(watcher.recv_timeout(Duration::from_millis(10)).is_none());
        Ok(())
    }

    fn event_targets_file(event_path: &Path, expected: &Path) -> bool {
        event_path.file_name() == expected.file_name()
    }

    fn wait_for_file_event<F>(watcher: &CapabilityWatcher, expected: &Path, matches: F) -> bool
    where
        F: Fn(&WatchEvent, &Path) -> bool,
    {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if let Some(event) = watcher.recv_timeout(Duration::from_millis(100)) {
                if matches(&event, expected) {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn test_watcher_file_changed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let watcher = CapabilityWatcher::new(dir.path())?;

        let file_path = dir.path().join("test.crepus");
        fs::write(&file_path, "div")?;

        let received = wait_for_file_event(&watcher, &file_path, |event, expected| {
            matches!(
                event,
                WatchEvent::FileChanged { path } if event_targets_file(path, expected)
            )
        });
        assert!(received, "Did not receive FileChanged event");
        Ok(())
    }

    #[test]
    fn test_watcher_manifest_updated() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let watcher = CapabilityWatcher::new(dir.path())?;

        let file_path = dir.path().join("webext.toml");
        fs::write(&file_path, "name = \"test\"")?;

        let received = wait_for_file_event(&watcher, &file_path, |event, expected| {
            matches!(
                event,
                WatchEvent::ManifestUpdated { path } if event_targets_file(path, expected)
            )
        });
        assert!(received, "Did not receive ManifestUpdated event");
        Ok(())
    }

    #[test]
    fn test_check_project_capabilities() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.crepus");
        fs::write(&file_path, "div on-click={browser.storage.local.get()}")?;

        let missing = check_project_capabilities(dir.path())?;
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], Capability::Storage);
        Ok(())
    }

    #[test]
    fn invalid_manifest_reports_an_error_without_missing_capability_advice() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("test.crepus");
        fs::write(&source, "div on-click={browser.storage.local.get()}").unwrap();
        fs::write(dir.path().join("webext.toml"), "name = [").unwrap();
        let folder = dir.path().join("incoming");
        fs::create_dir(&folder).unwrap();
        for (kind, path) in [
            (EventKind::Create(notify::event::CreateKind::File), source),
            (EventKind::Create(notify::event::CreateKind::Folder), folder),
        ] {
            let (tx, rx) = channel();
            handle_event(Event::new(kind).add_path(path), dir.path(), &tx);
            let events: Vec<_> = rx.try_iter().collect();
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, WatchEvent::Error { message }
            if message.contains("Failed to load manifest:"))),
                "{events:?}"
            );
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, WatchEvent::MissingCapabilities { .. })),
                "{events:?}"
            );
        }
    }

    #[test]
    fn populated_directory_events_scan_nested_capabilities() {
        for kind in [
            EventKind::Create(notify::event::CreateKind::Folder),
            EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both,
            )),
        ] {
            let project = tempdir().unwrap();
            let folder = project.path().join("incoming");
            fs::create_dir_all(folder.join("nested")).unwrap();
            fs::write(
                folder.join("nested/api.crepus"),
                "div on-click={browser.storage.local.get()}",
            )
            .unwrap();
            let (tx, rx) = channel();
            handle_event(Event::new(kind).add_path(folder), project.path(), &tx);
            let events: Vec<_> = rx.try_iter().collect();
            assert!(
                events.iter().any(|event| matches!(event,
                    WatchEvent::MissingCapabilities { capabilities, .. }
                    if capabilities.contains(&Capability::Storage))),
                "{kind:?}: {events:?}"
            );
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, WatchEvent::FileChanged { .. })),
                "directory events must not invent a changed source file: {events:?}"
            );
        }
    }

    #[test]
    fn watcher_detects_capabilities_in_moved_populated_directory() {
        // Sibling directories ensure rename stays on the same filesystem.
        let root = tempdir().unwrap();
        let project = root.path().join("project");
        let incoming = root.path().join("incoming");
        fs::create_dir(&project).unwrap();
        fs::create_dir_all(incoming.join("nested")).unwrap();
        fs::write(
            incoming.join("nested/api.crepus"),
            "div on-click={browser.storage.local.get()}",
        )
        .unwrap();
        let watcher = CapabilityWatcher::new(&project).unwrap();
        fs::rename(&incoming, project.join("imported")).unwrap();
        assert!(
            wait_for_file_event(&watcher, &project, |event, _| matches!(event,
                WatchEvent::MissingCapabilities { capabilities, .. }
                if capabilities.contains(&Capability::Storage))),
            "moving a populated directory did not report its nested Storage capability"
        );
    }

    #[test]
    fn unrelated_regular_file_events_do_not_scan_capabilities() {
        let project = tempdir().unwrap();
        fs::write(
            project.path().join("api.crepus"),
            "div on-click={browser.storage.local.get()}",
        )
        .unwrap();
        let unrelated = project.path().join("notes.txt");
        fs::write(&unrelated, "notes").unwrap();
        let (tx, rx) = channel();
        handle_event(
            Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path(unrelated),
            project.path(),
            &tx,
        );
        assert!(rx.try_iter().next().is_none());
    }
}
