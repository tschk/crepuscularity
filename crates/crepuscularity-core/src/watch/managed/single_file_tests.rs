use super::*;

fn settle(receive: &mpsc::Receiver<notify::Result<notify::Event>>) {
    for _ in 0..20 {
        if receive.recv_timeout(Duration::from_millis(250)).is_err() {
            return;
        }
    }
    panic!("watcher did not settle");
}

#[cfg(unix)]
#[test]
fn nonrecursive_file_roots_retain_nested_target_parent() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let target = nested.join("guest.js");
    std::fs::write(&target, "guest").unwrap();
    let logical = dir.path().join("guest.js");
    std::os::unix::fs::symlink(&target, &logical).unwrap();
    let single = Paths::resolve_for(&logical, None, Policy::SingleFile);
    let templates = Paths::resolve_for(&logical, None, Policy::Templates);
    assert_eq!(single.roots.len(), 2);
    assert_eq!(templates.roots.len(), 1);
    assert_eq!(Policy::SingleFile.mode(), RecursiveMode::NonRecursive);
}

#[test]
fn lazy_single_file_watcher_unwatches_and_can_watch_again() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.js");
    let second = dir.path().join("second.ts");
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    let (send, receive) = mpsc::channel();
    let mut watcher = SingleFileWatcher::new(
        move |event| {
            let _ = send.send(event);
        },
        Config::default(),
    )
    .unwrap();
    assert!(watcher.active.is_none());
    assert!(watcher.watch(&first, RecursiveMode::Recursive).is_err());
    watcher.watch(&first, RecursiveMode::NonRecursive).unwrap();
    watcher.watch(&first, RecursiveMode::NonRecursive).unwrap();
    assert!(watcher.watch(&second, RecursiveMode::NonRecursive).is_err());
    assert!(watcher.unwatch(&second).is_err());
    watcher.unwatch(&first).unwrap();
    watcher.watch(&second, RecursiveMode::NonRecursive).unwrap();
    // Drain any subscription/editor setup events before the separately timed edit.
    settle(&receive);
    std::fs::write(&second, "updated").unwrap();
    let event = receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(event.paths, vec![std::path::absolute(&second).unwrap()]);
    watcher.unwatch(&second).unwrap();
    assert!(watcher.active.is_none());
}

#[test]
fn single_file_policy_excludes_sibling_templates_and_context() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("guest.js");
    std::fs::write(&path, "guest").unwrap();
    let (send, receive) = mpsc::channel();
    let mut watcher = SingleFileWatcher::new(
        move |event| {
            let _ = send.send(event);
        },
        Config::default(),
    )
    .unwrap();
    watcher.watch(&path, RecursiveMode::NonRecursive).unwrap();
    settle(&receive);
    std::fs::write(dir.path().join("sibling.crepus"), "div").unwrap();
    std::fs::write(dir.path().join("context.toml"), "name='unused'").unwrap();
    assert!(receive.recv_timeout(Duration::from_millis(500)).is_err());
    std::fs::write(&path, "updated").unwrap();
    let event = receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(event.paths, vec![std::path::absolute(&path).unwrap()]);
}
