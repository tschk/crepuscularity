use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use notify::{Config, EventHandler, RecommendedWatcher, RecursiveMode, Watcher, WatcherKind};

#[derive(Clone, Copy)]
enum Policy {
    Templates,
    SingleFile,
}

impl Policy {
    fn mode(self) -> RecursiveMode {
        match self {
            Self::Templates => RecursiveMode::Recursive,
            Self::SingleFile => RecursiveMode::NonRecursive,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Paths {
    target: PathBuf,
    roots: Vec<PathBuf>,
}

impl Paths {
    fn resolve_for(path: &Path, previous: Option<&Self>, policy: Policy) -> Self {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let target = path.canonicalize().unwrap_or_else(|_| {
            previous.map_or_else(|| path.to_path_buf(), |paths| paths.target.clone())
        });
        let mut roots = match parent.canonicalize() {
            Ok(parent) => vec![parent],
            Err(_) => previous.map_or_else(|| vec![parent.to_path_buf()], |p| p.roots.clone()),
        };
        if let Some(parent) = target.parent() {
            roots.push(parent.to_path_buf());
        }
        roots.sort();
        roots.dedup();
        if matches!(policy, Policy::Templates)
            && roots.len() == 2
            && roots[1].starts_with(&roots[0])
        {
            roots.pop();
        }
        Self { target, roots }
    }
}

#[derive(PartialEq, Eq)]
struct RootIdentity {
    // shortcut: non-Unix roots use creation time; add native IDs for preserved timestamps.
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

struct WatchedRoot {
    path: PathBuf,
    identity: RootIdentity,
}

impl WatchedRoot {
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_dir() {
            return None;
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            path: path.to_path_buf(),
            identity: RootIdentity {
                created: metadata.created().ok(),
                #[cfg(unix)]
                device: metadata.dev(),
                #[cfg(unix)]
                inode: metadata.ino(),
            },
        })
    }
}

struct ManagedWatcher {
    inner: Option<Arc<Mutex<RecommendedWatcher>>>,
    stop: Option<mpsc::Sender<()>>,
}

impl ManagedWatcher {
    fn inner(&self) -> notify::Result<&Arc<Mutex<RecommendedWatcher>>> {
        self.inner
            .as_ref()
            .ok_or_else(|| notify::Error::generic("file watcher stopped"))
    }
}

impl Watcher for ManagedWatcher {
    fn new<F: EventHandler>(handler: F, config: Config) -> notify::Result<Self> {
        Ok(Self {
            inner: Some(Arc::new(Mutex::new(RecommendedWatcher::new(
                handler, config,
            )?))),
            stop: None,
        })
    }

    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
        self.inner()?.lock().map_err(poisoned)?.watch(path, mode)
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.inner()?.lock().map_err(poisoned)?.unwatch(path)
    }

    fn configure(&mut self, config: Config) -> notify::Result<bool> {
        self.inner()?.lock().map_err(poisoned)?.configure(config)
    }

    fn kind() -> WatcherKind {
        RecommendedWatcher::kind()
    }
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> notify::Error {
    notify::Error::generic("file watcher lock poisoned")
}

impl Drop for ManagedWatcher {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            // Release foreground ownership before waking the worker for teardown.
            drop(self.inner.take());
            let _ = stop.send(());
        }
    }
}

pub(super) fn create(
    path: PathBuf,
    changed: Arc<Mutex<bool>>,
    label: &'static str,
) -> Result<Box<dyn Watcher + Send>, String> {
    create_engine(
        path,
        Policy::Templates,
        Config::default(),
        move |event: notify::Result<notify::Event>| match event {
            Ok(_) => {
                if let Ok(mut changed) = changed.lock() {
                    *changed = true;
                }
            }
            Err(error) => eprintln!("[crepuscularity-{label}] watcher error: {error}"),
        },
        label,
    )
    .map(|watcher| Box::new(watcher) as Box<dyn Watcher + Send>)
    .map_err(|error| error.to_string())
}

fn create_engine(
    path: PathBuf,
    policy: Policy,
    config: Config,
    handler: impl EventHandler,
    label: &'static str,
) -> notify::Result<ManagedWatcher> {
    let path = std::path::absolute(path).map_err(notify::Error::io)?;
    let paths = Paths::resolve_for(&path, None, policy);
    let snapshot = Arc::new(Mutex::new(paths.clone()));
    let filter = Arc::clone(&snapshot);
    let handler = Arc::new(Mutex::new(handler));
    let event_handler = Arc::clone(&handler);
    let logical = path.clone();
    let mut watcher = ManagedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let event = match event {
                Ok(mut event) if super::is_relevant_kind(&event.kind) => {
                    let relevant = filter.lock().is_ok_and(|paths| {
                        event.paths.iter().any(|path| path == &logical)
                            || match policy {
                                Policy::Templates => paths.roots.iter().any(|root| {
                                    super::event_touches_relevant_path(&event, &paths.target, root)
                                }),
                                Policy::SingleFile => event.paths.iter().any(|path| {
                                    path == &paths.target
                                        || path
                                            .canonicalize()
                                            .is_ok_and(|target| target == paths.target)
                                }),
                            }
                    });
                    if !relevant {
                        return;
                    }
                    if matches!(policy, Policy::SingleFile) {
                        event.paths = vec![logical.clone()];
                    }
                    Ok(event)
                }
                Ok(_) => return,
                Err(error) => Err(error),
            };
            // Neither the snapshot nor native watcher lock is held across callbacks.
            if let Ok(mut handler) = event_handler.lock() {
                handler.handle_event(event);
            }
        },
        config,
    )?;
    let mut watched = Vec::new();
    for root in &paths.roots {
        watched.push(subscribe_with_mode(&mut watcher, root, policy.mode())?);
    }

    let inner = Arc::clone(watcher.inner()?);
    let (stop, stopped) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("crepus-{label}-watch-roots"))
        .spawn(move || {
            let mut paths = paths;
            let mut last_error = None;
            // shortcut: filesystem probes are not cancellable; add remote-filesystem handling if needed.
            while stopped.recv_timeout(Duration::from_millis(super::COOLDOWN_MS))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                let next = Paths::resolve_for(&path, Some(&paths), policy);
                let result = inner.lock().map_err(poisoned).and_then(|mut watcher| {
                    reconcile_with_mode(&mut *watcher, &next, &mut watched, policy.mode())
                });
                let error = match result {
                    Ok((rearmed, cleanup_error)) => {
                        if next != paths || rearmed {
                            if let Ok(mut snapshot) = snapshot.lock() {
                                *snapshot = next.clone();
                            }
                            if let Ok(mut handler) = handler.lock() {
                                handler.handle_event(Ok(notify::Event::new(
                                    notify::EventKind::Modify(notify::event::ModifyKind::Any),
                                )
                                .add_path(path.clone())));
                            }
                        }
                        paths = next;
                        cleanup_error.map(|error| error.to_string())
                    }
                    Err(error) => Some(error.to_string()),
                };
                if error != last_error {
                    if let Some(error) = &error {
                        if let Ok(mut handler) = handler.lock() {
                            handler.handle_event(Err(notify::Error::generic(error)));
                        }
                    }
                    last_error = error;
                }
            }
            // Drop the native watcher on this worker, without joining from the UI thread.
            drop(inner);
        })
        .map_err(notify::Error::io)?;
    watcher.stop = Some(stop);
    Ok(watcher)
}

#[cfg(test)]
fn subscribe(watcher: &mut impl Watcher, root: &Path) -> notify::Result<WatchedRoot> {
    subscribe_with_mode(watcher, root, RecursiveMode::Recursive)
}

fn subscribe_with_mode(
    watcher: &mut impl Watcher,
    root: &Path,
    mode: RecursiveMode,
) -> notify::Result<WatchedRoot> {
    // Capture identity before subscribing so a concurrent replacement forces rearming.
    let identity = WatchedRoot::read(root).ok_or_else(|| {
        notify::Error::generic("watch root unavailable").add_path(root.to_path_buf())
    })?;
    watcher.watch(root, mode)?;
    Ok(identity)
}

#[cfg(test)]
fn reconcile(
    watcher: &mut impl Watcher,
    next: &Paths,
    watched: &mut Vec<WatchedRoot>,
) -> notify::Result<(bool, Option<notify::Error>)> {
    reconcile_with_mode(watcher, next, watched, RecursiveMode::Recursive)
}

fn reconcile_with_mode(
    watcher: &mut impl Watcher,
    next: &Paths,
    watched: &mut Vec<WatchedRoot>,
    mode: RecursiveMode,
) -> notify::Result<(bool, Option<notify::Error>)> {
    watched.retain(|root| {
        if WatchedRoot::read(&root.path).is_some_and(|current| current.identity == root.identity) {
            true
        } else {
            let _ = watcher.unwatch(&root.path);
            false
        }
    });
    let mut rearmed = false;
    for root in &next.roots {
        if !watched.iter().any(|watched| &watched.path == root) {
            watched.push(subscribe_with_mode(watcher, root, mode)?);
            rearmed = true;
        }
    }
    // Native recursive subscriptions can share watches across overlapping roots.
    let mut rearm = Vec::new();
    let mut cleanup_error = None;
    let mut index = 0;
    while index < watched.len() {
        if next.roots.contains(&watched[index].path) {
            index += 1;
        } else {
            match watcher.unwatch(&watched[index].path) {
                Ok(()) => {
                    let removed = watched.remove(index);
                    for root in &next.roots {
                        if (root.starts_with(&removed.path) || removed.path.starts_with(root))
                            && !rearm.contains(root)
                        {
                            rearm.push(root.clone());
                        }
                    }
                }
                Err(error) => {
                    cleanup_error = Some(error);
                    index += 1;
                }
            }
        }
    }
    for root in rearm {
        let _ = watcher.unwatch(&root);
        watched.retain(|watched| watched.path != root);
        watched.push(subscribe_with_mode(watcher, &root, mode)?);
        rearmed = true;
    }
    Ok((rearmed, cleanup_error))
}

