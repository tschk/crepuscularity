use crepuscularity_lite::guest_watch::spawn_guest_file_watcher;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn watch(path: &Path) -> Receiver<()> {
    let (tx, rx) = channel();
    let caller = std::thread::current().id();
    spawn_guest_file_watcher(path.into(), Duration::from_millis(80), move || {
        assert_ne!(std::thread::current().id(), caller);
        let _ = tx.send(());
    })
    .unwrap();
    rx
}

fn changed(rx: &Receiver<()>) {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("guest change was not delivered");
}

fn quiet(rx: &Receiver<()>) {
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "unrelated or duplicate callback"
    );
}

// A retarget can yield both native and reconciliation events, legitimately in
// separate debounce batches. Drain across two reconciliation intervals before
// asserting that later edits reach only the new target.
fn settle_retarget(rx: &Receiver<()>) {
    let minimum = Instant::now() + Duration::from_millis(500);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let now = Instant::now();
        assert!(now < deadline, "retarget notifications did not settle");
        let wait = minimum
            .saturating_duration_since(now)
            .max(Duration::from_millis(300));
        match rx.recv_timeout(wait.min(deadline.saturating_duration_since(now))) {
            Ok(()) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if Instant::now() >= minimum => return,
            Err(error) => panic!("retarget watcher disconnected: {error}"),
        }
    }
}

#[test]
fn regular_guest_filters_neighbours_and_bounds_burst_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let guest = dir.path().join("guest.ts");
    std::fs::write(&guest, "initial").unwrap();
    let rx = watch(&guest);
    std::fs::write(dir.path().join("nearby.crepus"), "span").unwrap();
    std::fs::write(dir.path().join("context.toml"), "value=1").unwrap();
    quiet(&rx);
    for value in 0..10 {
        std::fs::write(&guest, value.to_string()).unwrap();
    }
    changed(&rx);
    // Mini may emit AnyContinuous at the insertion deadline and a final Any
    // at the last-update deadline; preserve that existing two-batch contract.
    let mut callbacks = 1;
    loop {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(()) => {
                callbacks += 1;
                assert!(callbacks <= 2, "burst produced {callbacks} callbacks");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            Err(error) => panic!("guest watcher disconnected: {error}"),
        }
    }
}

#[test]
fn atomic_save_and_delete_recreate_keep_the_logical_guest_watched() {
    let dir = tempfile::tempdir().unwrap();
    let guest = dir.path().join("guest.ts");
    std::fs::write(&guest, "initial").unwrap();
    let rx = watch(&guest);
    let staged = dir.path().join("staged");
    std::fs::write(&staged, "replacement").unwrap();
    std::fs::rename(staged, &guest).unwrap();
    changed(&rx);
    quiet(&rx);
    std::fs::remove_file(&guest).unwrap();
    changed(&rx);
    quiet(&rx);
    std::fs::write(&guest, "recreated").unwrap();
    changed(&rx);
}

#[cfg(unix)]
#[test]
fn external_file_symlink_retarget_follows_new_directory() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let old_guest = old.path().join("guest.ts");
    let new_guest = new.path().join("guest.ts");
    std::fs::write(&old_guest, "old").unwrap();
    std::fs::write(&new_guest, "new").unwrap();
    let logical = root.path().join("guest.ts");
    symlink(&old_guest, &logical).unwrap();
    let rx = watch(&logical);
    std::fs::write(&old_guest, "edit old").unwrap();
    changed(&rx);
    quiet(&rx);
    let staged = root.path().join("staged");
    symlink(&new_guest, &staged).unwrap();
    std::fs::rename(staged, &logical).unwrap();
    changed(&rx);
    settle_retarget(&rx);
    std::fs::write(&new_guest, "edit new").unwrap();
    changed(&rx);
    quiet(&rx);
    std::fs::write(&old_guest, "obsolete").unwrap();
    quiet(&rx);
}

#[cfg(unix)]
#[test]
fn directory_symlink_retarget_follows_guest() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    std::fs::write(old.path().join("guest.ts"), "old").unwrap();
    std::fs::write(new.path().join("guest.ts"), "new").unwrap();
    let logical_dir = root.path().join("scripts");
    symlink(old.path(), &logical_dir).unwrap();
    let rx = watch(&logical_dir.join("guest.ts"));
    let staged = root.path().join("staged");
    symlink(new.path(), &staged).unwrap();
    std::fs::rename(staged, logical_dir).unwrap();
    changed(&rx);
    settle_retarget(&rx);
    std::fs::write(new.path().join("guest.ts"), "edit new").unwrap();
    changed(&rx);
}

#[cfg(unix)]
#[test]
fn nested_target_parent_is_watched_nonrecursively_as_its_own_root() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let target = nested.join("real.js");
    std::fs::write(&target, "initial").unwrap();
    let entry = root.path().join("entry.js");
    symlink(&target, &entry).unwrap();
    let rx = watch(&entry);
    std::fs::write(&target, "changed").unwrap();
    changed(&rx);
    quiet(&rx);
}
