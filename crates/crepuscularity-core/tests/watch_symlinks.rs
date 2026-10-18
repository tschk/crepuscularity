#![cfg(all(feature = "notify", unix))]

use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Changed = Arc<Mutex<bool>>;

fn take_changed(changed: &Changed) -> bool {
    std::mem::take(&mut *changed.lock().unwrap())
}

// Drain startup/replacement events before testing a separate, later edit. Both
// a minimum settling period and a quiet period are needed for asynchronous OS
// delivery and the watcher's periodic subscription reconciliation.
fn settle(changed: &Changed) {
    let start = Instant::now();
    let mut quiet_since = start;
    loop {
        if take_changed(changed) {
            quiet_since = Instant::now();
        }
        if start.elapsed() >= Duration::from_millis(1200)
            && quiet_since.elapsed() >= Duration::from_millis(600)
        {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "watcher never settled"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn expect_reload(changed: &Changed, edit: impl FnOnce()) {
    settle(changed);
    edit();
    let start = Instant::now();
    while !take_changed(changed) {
        assert!(
            start.elapsed() < Duration::from_secs(6),
            "edit did not trigger reload"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn replace_link(link: &Path, target: &Path) {
    let replacement = link.with_extension("replacement");
    symlink(target, &replacement).unwrap();
    std::fs::rename(replacement, link).unwrap();
}

#[test]
fn external_entry_keeps_logical_include_and_context_watched() {
    let dir = tempfile::tempdir().unwrap();
    let logical = dir.path().join("logical");
    let external = dir.path().join("external");
    std::fs::create_dir(&logical).unwrap();
    std::fs::create_dir(&external).unwrap();
    let target = external.join("source.crepus");
    write(&target, "div");
    let entry = logical.join("entry.crepus");
    symlink(&target, &entry).unwrap();
    let include = logical.join("child.crepus");
    let context = logical.join("context.toml");
    write(&include, "span");
    write(&context, "name = 'old'");
    let changed = Arc::new(Mutex::new(false));
    let _watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    expect_reload(&changed, || write(&include, "section"));
    expect_reload(&changed, || write(&context, "name = 'new'"));
}

#[test]
fn extensionless_vue_target_edits_trigger_reload() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let target = external.path().join("source");
    write(&target, "<template><div /></template>");
    let entry = dir.path().join("entry.vue");
    symlink(&target, &entry).unwrap();
    let changed = Arc::new(Mutex::new(false));
    let _watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    expect_reload(&changed, || write(&target, "<template><span /></template>"));
}

#[test]
fn entry_retarget_subscribes_to_later_edits_in_new_directory() {
    let dir = tempfile::tempdir().unwrap();
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let old_target = old.path().join("source");
    let new_target = new.path().join("source");
    write(&old_target, "div");
    write(&new_target, "span");
    let entry = dir.path().join("entry.crepus");
    symlink(&old_target, &entry).unwrap();
    let changed = Arc::new(Mutex::new(false));
    let _watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    expect_reload(&changed, || replace_link(&entry, &new_target));
    expect_reload(&changed, || write(&new_target, "section"));
}

#[test]
fn directory_symlink_retarget_watches_new_entry_include_and_context() {
    let dir = tempfile::tempdir().unwrap();
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    for root in [old.path(), new.path()] {
        write(&root.join("entry.crepus"), "div");
        write(&root.join("child.crepus"), "span");
        write(&root.join("context.toml"), "name = 'old'");
    }
    let current = dir.path().join("current");
    symlink(old.path(), &current).unwrap();
    let changed = Arc::new(Mutex::new(false));
    let _watcher = crepuscularity_core::watch::create_watcher(
        current.join("entry.crepus"),
        changed.clone(),
        "test",
    )
    .unwrap();

    expect_reload(&changed, || replace_link(&current, new.path()));
    expect_reload(&changed, || {
        write(&new.path().join("child.crepus"), "section")
    });
    expect_reload(&changed, || {
        write(&new.path().join("context.toml"), "name = 'new'")
    });
    expect_reload(&changed, || {
        write(&new.path().join("entry.crepus"), "article")
    });
}

#[test]
fn temporary_missing_entry_and_atomic_target_replace_recover() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let target = external.path().join("source");
    write(&target, "div");
    let entry = dir.path().join("entry.crepus");
    symlink(&target, &entry).unwrap();
    let changed = Arc::new(Mutex::new(false));
    let _watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    expect_reload(&changed, || std::fs::remove_file(&entry).unwrap());
    // Keep it absent across several reconciliation ticks, then restore it.
    settle(&changed);
    expect_reload(&changed, || symlink(&target, &entry).unwrap());
    expect_reload(&changed, || {
        let replacement = external.path().join("saved");
        write(&replacement, "section");
        std::fs::rename(replacement, &target).unwrap();
    });
    expect_reload(&changed, || write(&target, "article"));
}

#[test]
fn ancestor_target_root_retarget_preserves_logical_descendant_subscription() {
    let dir = tempfile::tempdir().unwrap();
    let logical = dir.path().join("views");
    std::fs::create_dir(&logical).unwrap();
    let old_target = dir.path().join("source");
    write(&old_target, "div");
    let external = tempfile::tempdir().unwrap();
    let new_target = external.path().join("source");
    write(&new_target, "span");
    let entry = logical.join("entry.crepus");
    symlink(&old_target, &entry).unwrap();
    let include = logical.join("child.crepus");
    write(&include, "span");
    let changed = Arc::new(Mutex::new(false));
    let _watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    expect_reload(&changed, || replace_link(&entry, &new_target));
    expect_reload(&changed, || write(&include, "section"));
    expect_reload(&changed, || write(&new_target, "article"));
}

#[test]
fn same_path_directory_replacement_rearms_later_edits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("views");
    let replacement = dir.path().join("replacement");
    for path in [&root, &replacement] {
        std::fs::create_dir(path).unwrap();
        write(&path.join("entry.crepus"), "div");
        write(&path.join("child.crepus"), "span");
    }
    let changed = Arc::new(Mutex::new(false));
    let watcher = crepuscularity_core::watch::create_watcher(
        root.join("entry.crepus"),
        changed.clone(),
        "test",
    )
    .unwrap();

    expect_reload(&changed, || {
        // Replace the directory between reconciliation ticks: the spelling and
        // resolved root stay identical, but the directory identity changes.
        std::fs::rename(&root, dir.path().join("previous")).unwrap();
        std::fs::rename(&replacement, &root).unwrap();
    });
    expect_reload(&changed, || write(&root.join("child.crepus"), "section"));
    expect_reload(&changed, || write(&root.join("entry.crepus"), "article"));
    drop(watcher);
}

#[test]
fn dropping_watcher_eventually_releases_callback_and_monitor_state() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("entry.crepus");
    write(&entry, "div");
    let changed = Arc::new(Mutex::new(false));
    let weak = Arc::downgrade(&changed);
    let watcher =
        crepuscularity_core::watch::create_watcher(&entry, changed.clone(), "test").unwrap();

    drop(changed);
    assert!(weak.upgrade().is_some());
    drop(watcher);
    let start = Instant::now();
    while weak.upgrade().is_some() {
        assert!(
            start.elapsed() < Duration::from_secs(6),
            "dropped watcher retained callback or monitor state"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
