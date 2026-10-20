//! Actual Notify registration helper; no GPUI runner or visual evidence.
#[path = "../src/dev_watch.rs"]
mod dev_watch;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn receives_change(relative: &str, manifest: &str) {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    let path = root_path.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "fn main() {}\n").unwrap();
    std::fs::write(root_path.join("Cargo.toml"), manifest).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if let Ok(event) = result {
            let _ = tx.send(event);
        }
    })
    .unwrap();
    dev_watch::watch_project(&mut watcher, &root_path);
    let changed = if relative == "Cargo.toml" {
        root_path.join("Cargo.toml")
    } else {
        path
    };
    std::fs::write(&changed, "// changed source fixture\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|error| panic!("no event for {}: {error}", changed.display()));
        if event.paths.iter().any(|path| same_path(path, &changed)) {
            break;
        }
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let resolve = |path: &Path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    resolve(left) == resolve(right)
}

#[test]
fn custom_declared_binary_source_is_watched_without_src_directory() {
    receives_change(
        "app/main.rs",
        "[package]\nname='host'\nversion='0.0.0'\n[[bin]]\nname='worker'\npath='app/main.rs'\n",
    );
}

#[test]
fn conventional_src_and_manifest_remain_watched() {
    receives_change("src/main.rs", "[package]\nname='host'\nversion='0.0.0'\n");
    receives_change("Cargo.toml", "[package]\nname='host'\nversion='0.0.0'\n");
}

#[test]
fn declared_library_and_build_script_files_are_watched() {
    receives_change(
        "library/root.rs",
        "[package]\nname='host'\nversion='0.0.0'\n[lib]\npath='library/root.rs'\n",
    );
    receives_change(
        "tools/build.rs",
        "[package]\nname='host'\nversion='0.0.0'\nbuild='tools/build.rs'\n",
    );
    receives_change("build.rs", "[package]\nname='host'\nversion='0.0.0'\n");
}

#[test]
#[cfg(target_os = "macos")]
fn macos_atomic_replacement_and_later_edits_remain_watched() {
    for relative in ["app/main.rs", "Cargo.toml"] {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let manifest =
            "[package]\nname='host'\nversion='0.0.0'\n[[bin]]\nname='worker'\npath='app/main.rs'\n";
        std::fs::create_dir(root_path.join("app")).unwrap();
        std::fs::write(root_path.join("Cargo.toml"), manifest).unwrap();
        std::fs::write(root_path.join("app/main.rs"), "fn main() {}\n").unwrap();
        let target = root_path.join(relative);
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result {
                    let _ = tx.send(event);
                }
            })
            .unwrap();
        dev_watch::watch_project(&mut watcher, &root_path);
        let replacement = target.with_extension("replacement");
        let source = if relative == "Cargo.toml" {
            manifest
        } else {
            "fn main() { let _ = 1; }\n"
        };
        std::fs::write(&replacement, source).unwrap();
        std::fs::rename(&replacement, &target).unwrap();
        let receive_target = |phase: &str| {
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                let event = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|error| panic!("{relative} {phase}: {error}"));
                if event.paths.iter().any(|path| same_path(path, &target)) {
                    break;
                }
            }
        };
        receive_target("atomic replacement");
        dev_watch::watch_project(&mut watcher, &root_path);
        // Require a full quiet interval so delayed replacement events cannot
        // satisfy the assertion for the later edit.
        let drain_deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Ok(_) => assert!(Instant::now() < drain_deadline, "watcher did not settle"),
                Err(error) => panic!("watcher disconnected: {error}"),
            }
        }
        let comment = if relative == "Cargo.toml" {
            "# later edit"
        } else {
            "// later edit"
        };
        std::fs::write(&target, format!("{source}\n{comment}\n")).unwrap();
        receive_target("second edit after registration refresh");
    }
}
