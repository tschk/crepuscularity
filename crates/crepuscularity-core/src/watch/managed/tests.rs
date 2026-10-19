use super::*;

#[derive(Default)]
struct MockWatcher {
    operations: Vec<(&'static str, PathBuf)>,
    watches: Vec<PathBuf>,
    unwatches: Vec<PathBuf>,
    fail_watch: Option<PathBuf>,
    fail_unwatch: Option<PathBuf>,
    replace_on_watch: Option<PathBuf>,
}

impl Watcher for MockWatcher {
    fn new<F: EventHandler>(_: F, _: Config) -> notify::Result<Self> {
        Ok(Self::default())
    }

    fn watch(&mut self, path: &Path, _: RecursiveMode) -> notify::Result<()> {
        self.operations.push(("watch", path.to_path_buf()));
        self.watches.push(path.to_path_buf());
        if self.replace_on_watch.as_deref() == Some(path) {
            self.replace_on_watch = None;
            std::fs::rename(path, path.with_extension("previous")).unwrap();
            std::fs::create_dir(path).unwrap();
        }
        if self.fail_watch.as_deref() == Some(path) || !path.is_dir() {
            Err(notify::Error::generic("subscribe failed"))
        } else {
            Ok(())
        }
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.operations.push(("unwatch", path.to_path_buf()));
        self.unwatches.push(path.to_path_buf());
        if self.fail_unwatch.as_deref() == Some(path) {
            Err(notify::Error::generic("unsubscribe failed"))
        } else {
            Ok(())
        }
    }

    fn kind() -> WatcherKind {
        WatcherKind::PollWatcher
    }
}

#[cfg(unix)]
#[test]
fn replacement_during_subscribe_retains_old_identity_until_reconcile_rearms() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("watched");
    std::fs::create_dir(&root).unwrap();
    let old_identity = WatchedRoot::read(&root).unwrap().identity;
    let mut watcher = MockWatcher {
        replace_on_watch: Some(root.clone()),
        ..Default::default()
    };

    let subscribed = subscribe(&mut watcher, &root).unwrap();
    assert!(subscribed.identity == old_identity);
    assert!(subscribed.identity != WatchedRoot::read(&root).unwrap().identity);
    let mut watched = vec![subscribed];
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &roots(&[&root]), &mut watched).unwrap();
    assert!(rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(watched_paths(&watched), vec![root.as_path()]);
    assert!(watched[0].identity == WatchedRoot::read(&root).unwrap().identity);
    assert_eq!(watcher.watches, vec![root.clone(), root.clone()]);
    assert_eq!(watcher.unwatches, vec![root]);
}

#[test]
fn overlapping_root_transition_rearms_desired_after_obsolete_unwatch() {
    let dir = tempfile::tempdir().unwrap();
    let ancestor = dir.path().join("ancestor");
    let descendant = ancestor.join("descendant");
    std::fs::create_dir_all(&descendant).unwrap();
    for (obsolete, desired) in [(&ancestor, &descendant), (&descendant, &ancestor)] {
        let mut watcher = MockWatcher::default();
        let mut watched = vec![WatchedRoot::read(obsolete).unwrap()];
        let next = roots(&[desired]);

        let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
        assert!(rearmed);
        assert!(cleanup_error.is_none());
        assert_eq!(watched_paths(&watched), vec![desired.as_path()]);
        let removed = watcher
            .operations
            .iter()
            .position(|(operation, path)| *operation == "unwatch" && path == obsolete)
            .unwrap();
        let final_watch = watcher
            .operations
            .iter()
            .rposition(|(operation, path)| *operation == "watch" && path == desired)
            .unwrap();
        assert!(
            final_watch > removed,
            "desired root must be rearmed after overlapping removal"
        );
        assert_eq!(
            watcher.operations.last(),
            Some(&("watch", desired.to_path_buf()))
        );
    }
}

fn watched_paths(watched: &[WatchedRoot]) -> Vec<&Path> {
    watched.iter().map(|root| root.path.as_path()).collect()
}

fn roots(paths: &[&Path]) -> Paths {
    Paths {
        target: paths[0].join("entry.crepus"),
        roots: paths.iter().map(|path| path.to_path_buf()).collect(),
    }
}

#[test]
fn partial_subscribe_failure_keeps_old_roots_and_retries_only_failed_add() {
    let old = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut watcher = MockWatcher {
        fail_watch: Some(second.path().to_path_buf()),
        ..Default::default()
    };
    let mut watched = vec![WatchedRoot::read(old.path()).unwrap()];
    let next = roots(&[first.path(), second.path()]);

    assert!(reconcile(&mut watcher, &next, &mut watched).is_err());
    assert_eq!(watched_paths(&watched), vec![old.path(), first.path()]);
    assert!(watcher.unwatches.is_empty());

    watcher.fail_watch = None;
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(
        watched_paths(&watched),
        next.roots.iter().map(PathBuf::as_path).collect::<Vec<_>>()
    );
    assert_eq!(
        watcher.watches,
        vec![first.path(), second.path(), second.path()]
    );
    assert_eq!(watcher.unwatches, vec![old.path()]);
}

#[test]
fn obsolete_unwatch_failure_allows_new_snapshot_and_retries_cleanup() {
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let mut watcher = MockWatcher {
        fail_unwatch: Some(old.path().to_path_buf()),
        ..Default::default()
    };
    let mut watched = vec![WatchedRoot::read(old.path()).unwrap()];
    let next = roots(&[new.path()]);

    // Ok permits publishing the new Paths despite an obsolete-root cleanup error.
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(rearmed);
    assert!(cleanup_error.is_some());
    assert_eq!(watched_paths(&watched), vec![old.path(), new.path()]);

    watcher.fail_unwatch = None;
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(!rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(
        watched_paths(&watched),
        next.roots.iter().map(PathBuf::as_path).collect::<Vec<_>>()
    );
    assert_eq!(watcher.watches, vec![new.path()]);
    assert_eq!(watcher.unwatches, vec![old.path(), old.path()]);
}

#[test]
fn missing_watched_directory_is_removed_and_rearmed_after_recreation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("watched");
    std::fs::create_dir(&root).unwrap();
    let next = roots(&[&root]);
    let mut watched = vec![WatchedRoot::read(&root).unwrap()];
    let mut watcher = MockWatcher::default();

    std::fs::remove_dir(&root).unwrap();
    assert!(reconcile(&mut watcher, &next, &mut watched).is_err());
    assert!(watched.is_empty());
    assert_eq!(watcher.unwatches, vec![root.clone()]);

    std::fs::create_dir(&root).unwrap();
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(
        watched_paths(&watched),
        next.roots.iter().map(PathBuf::as_path).collect::<Vec<_>>()
    );
    assert_eq!(watcher.watches, vec![root]);
}

#[cfg(unix)]
#[test]
fn same_path_replaced_directory_rearms_its_subscription() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("watched");
    let replacement = dir.path().join("replacement");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&replacement).unwrap();
    let next = roots(&[&root]);
    let mut watched = vec![WatchedRoot::read(&root).unwrap()];
    let old_identity = WatchedRoot::read(&root).unwrap().identity;
    let mut watcher = MockWatcher::default();

    std::fs::rename(&root, dir.path().join("previous")).unwrap();
    std::fs::rename(&replacement, &root).unwrap();
    assert!(WatchedRoot::read(&root).unwrap().identity != old_identity);
    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(watcher.unwatches, vec![root.clone()]);
    assert_eq!(watcher.watches, vec![root.clone()]);
    assert_eq!(watched_paths(&watched), vec![root.as_path()]);
    assert!(watched[0].identity == WatchedRoot::read(&root).unwrap().identity);

    let (rearmed, cleanup_error) = reconcile(&mut watcher, &next, &mut watched).unwrap();
    assert!(!rearmed);
    assert!(cleanup_error.is_none());
    assert_eq!(watcher.watches.len(), 1);
    assert_eq!(watcher.unwatches.len(), 1);
}
