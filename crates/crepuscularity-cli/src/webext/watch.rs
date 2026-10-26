//! Watch authored extension inputs while keeping build output out of the event stream.
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Sender, Arc, Mutex};

const SOURCES: [&str; 7] = [
    "runtime/src",
    "runtime/views",
    "src",
    "pages",
    "views",
    "icons",
    "resources",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

fn directory_identity(path: &Path) -> Option<DirectoryIdentity> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(DirectoryIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Some(DirectoryIdentity {})
    }
}

pub(super) struct ExtensionWatcher {
    app: PathBuf,
    watcher: RecommendedWatcher,
    watched: BTreeMap<PathBuf, DirectoryIdentity>,
    invalidated: Arc<Mutex<BTreeSet<PathBuf>>>,
}

impl ExtensionWatcher {
    pub(super) fn new(app: &Path, tx: Sender<()>) -> Result<Self, String> {
        let app = app
            .canonicalize()
            .map_err(|error| format!("resolve extension app {}: {error}", app.display()))?;
        if !app.is_dir() {
            return Err(format!(
                "extension app is not a directory: {}",
                app.display()
            ));
        }
        let event_app = app.clone();
        let invalidated = Arc::new(Mutex::new(BTreeSet::new()));
        let event_invalidated = Arc::clone(&invalidated);
        let roots: Vec<_> = SOURCES
            .iter()
            .map(|source| app.join(source))
            .chain(std::iter::once(app.join("runtime")))
            .collect();
        let watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                if relevant_event(&event_app, &event) {
                    if matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Remove(_)
                            | EventKind::Modify(notify::event::ModifyKind::Name(_))
                    ) {
                        let mut invalidated = event_invalidated
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        for root in &roots {
                            if event.paths.iter().any(|path| root.starts_with(path)) {
                                invalidated.insert(root.clone());
                            }
                        }
                    }
                    let _ = tx.send(());
                }
            }
        })
        .map_err(|error| format!("create extension watcher: {error}"))?;
        let mut this = Self {
            app,
            watcher,
            watched: BTreeMap::new(),
            invalidated,
        };
        this.watcher
            .watch(&this.app, RecursiveMode::NonRecursive)
            .map_err(|error| format!("watch extension app {}: {error}", this.app.display()))?;
        this.refresh()?;
        Ok(this)
    }

    pub(super) fn refresh(&mut self) -> Result<(), String> {
        let invalidated = std::mem::take(
            &mut *self
                .invalidated
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let runtime = self.app.join("runtime");
        let desired: BTreeMap<_, _> = SOURCES
            .iter()
            .map(|source| self.app.join(source))
            .chain(std::iter::once(runtime.clone()))
            .filter_map(|path| directory_identity(&path).map(|identity| (path, identity)))
            .collect();
        // FSEvents restarts its whole stream on watch changes. Retain unchanged
        // directories so queued source events are not replayed on every build.
        let removed: Vec<_> = self
            .watched
            .iter()
            .filter(|(path, identity)| {
                desired.get(*path) != Some(*identity)
                    || (!cfg!(unix) && invalidated.contains(*path))
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in removed {
            let _ = self.watcher.unwatch(&path);
            self.watched.remove(&path);
        }
        for (path, identity) in desired {
            if !self.watched.contains_key(&path) {
                let mode = if path == runtime {
                    RecursiveMode::NonRecursive
                } else {
                    RecursiveMode::Recursive
                };
                self.watcher.watch(&path, mode).map_err(|error| {
                    format!("watch extension source {}: {error}", path.display())
                })?;
                self.watched.insert(path, identity);
            }
        }
        Ok(())
    }
}

fn relevant_event(app: &Path, event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|path| {
        if path == &app.join("runtime") {
            matches!(
                event.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(notify::event::ModifyKind::Name(_))
            )
        } else {
            relevant_path(app, path)
        }
    })
}

fn relevant_path(app: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(app) else {
        return false;
    };
    relative == Path::new("crepus.toml")
        || relative == Path::new("runtime")
        || relative == Path::new("runtime/Cargo.toml")
        || SOURCES.iter().any(|source| relative.starts_with(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode};

    #[test]
    fn input_create_modify_remove_and_rename_events_are_relevant() {
        let app = Path::new("/extension");
        for kind in [
            EventKind::Create(CreateKind::Any),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::Any),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        ] {
            for path in [
                "crepus.toml",
                "runtime/Cargo.toml",
                "runtime/src",
                "runtime/src/lib.rs",
                "runtime/views/entry.crepus",
                "src/background.ts",
                "pages/new",
                "views/a.vue",
                "icons/icon.png",
                "resources/data.json",
            ] {
                let event = Event::new(kind).add_path(app.join(path));
                assert!(relevant_event(app, &event), "{kind:?}: {path}");
            }
        }
        let rename = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(app.join(".temporary"))
            .add_path(app.join("crepus.toml"));
        assert!(relevant_event(app, &rename));
    }

    #[test]
    fn runtime_container_metadata_does_not_trigger_its_own_build() {
        let app = Path::new("/extension");
        for kind in [
            ModifyKind::Any,
            ModifyKind::Data(notify::event::DataChange::Any),
            ModifyKind::Metadata(notify::event::MetadataKind::Any),
        ] {
            assert!(!relevant_event(
                app,
                &Event::new(EventKind::Modify(kind)).add_path(app.join("runtime"))
            ));
        }
        for kind in [
            EventKind::Create(CreateKind::Folder),
            EventKind::Remove(RemoveKind::Folder),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        ] {
            assert!(relevant_event(
                app,
                &Event::new(kind).add_path(app.join("runtime"))
            ));
        }
    }

    #[test]
    fn build_outputs_unrelated_paths_and_access_events_are_ignored() {
        let app = Path::new("/extension");
        for path in [
            "dist",
            "dist/unpacked/src/.reload-id",
            "dist/chromium/manifest.json",
            "target/debug/crepus",
            "runtime/target/wasm32-unknown-unknown/release/app.wasm",
            ".crepus-cache/index.json",
            "Cargo.toml",
            "Cargo.lock",
            "README.md",
            "runtime/Cargo.lock",
            "runtime/src-other/file.rs",
        ] {
            let event = Event::new(EventKind::Modify(ModifyKind::Any)).add_path(app.join(path));
            assert!(!relevant_event(app, &event), "{path}");
        }
        for event in [
            Event::new(EventKind::Access(AccessKind::Any)).add_path(app.join("src/file.ts")),
            Event::new(EventKind::Other).add_path(app.join("src/file.ts")),
            Event::new(EventKind::Remove(RemoveKind::Any))
                .add_path(PathBuf::from("/elsewhere/src/file.ts")),
        ] {
            assert!(!relevant_event(app, &event));
        }
    }

    #[test]
    fn refresh_discovers_removed_and_recreated_source_directories() {
        let app = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(app.path().join("runtime/src")).unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut watcher = ExtensionWatcher::new(app.path(), tx).unwrap();
        let source = watcher.app.join("runtime/src");
        assert!(watcher.watched.contains_key(&source));
        std::fs::remove_dir(&source).unwrap();
        watcher.refresh().unwrap();
        assert!(!watcher.watched.contains_key(&source));
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(watcher.app.join("pages")).unwrap();
        watcher.refresh().unwrap();
        assert!(watcher.watched.contains_key(&source));
        assert!(watcher.watched.contains_key(&watcher.app.join("pages")));
        // Same-path replacement between refreshes must also re-arm the OS watch.
        std::fs::remove_dir(&source).unwrap();
        std::fs::create_dir(&source).unwrap();
        watcher.refresh().unwrap();
        assert!(watcher.watched.contains_key(&source));
    }
}
