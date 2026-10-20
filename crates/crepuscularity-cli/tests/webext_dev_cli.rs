use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
struct Dev(Child);
impl Drop for Dev {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn log(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}
fn reload(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}
fn wait(dev: &mut Dev, logfile: &Path, description: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(
            dev.0.try_wait().unwrap().is_none(),
            "dev exited waiting for {description}: {}",
            log(logfile)
        );
        if predicate() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timeout waiting for {description}: {}",
            log(logfile)
        );
        thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn dev_rebuilds_directory_removal_and_recreation_and_recovers_after_errors() {
    let site = tempfile::tempdir().unwrap();
    let root = site.path();
    fs::write(root.join("crepus.toml"),"[[targets]]\ntype='webext'\nid='extension'\napp='.'\nbrowsers=['chromium']\n[targets.extension]\nname='Dev output'\nversion='1.0.0'\n").unwrap();
    fs::create_dir_all(root.join("pages")).unwrap();
    fs::create_dir_all(root.join("resources")).unwrap();
    let page = root.join("pages/popup.crepus");
    fs::write(&page, "span \"Original page\"").unwrap();
    fs::write(root.join("resources/old.txt"), "old").unwrap();
    // Keep logs outside the watched app so test observations do not trigger rebuilds.
    let logs = tempfile::tempdir().unwrap();
    let logfile = logs.path().join("stderr.log");
    let child = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(root)
        .args(["webext", "dev", "--browser", "chromium"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(fs::File::create(&logfile).unwrap()))
        .spawn()
        .unwrap();
    let mut dev = Dev(child);
    let dist = root.join("dist/chromium");
    let marker = dist.join("src/.reload-id");
    wait(&mut dev, &logfile, "watch registration", || {
        log(&logfile).contains("watching for changes") && reload(&marker).is_some()
    });
    let initial = reload(&marker).unwrap();
    assert!(dist.join("resources/old.txt").exists());
    fs::remove_dir_all(root.join("resources")).unwrap();
    wait(&mut dev, &logfile, "removed resource directory", || {
        !dist.join("resources/old.txt").exists() && reload(&marker).is_some_and(|id| id > initial)
    });
    let removed = reload(&marker).unwrap();
    fs::create_dir_all(root.join("resources")).unwrap();
    fs::write(root.join("resources/new.txt"), "new").unwrap();
    wait(&mut dev, &logfile, "recreated resource directory", || {
        fs::read_to_string(dist.join("resources/new.txt"))
            .ok()
            .as_deref()
            == Some("new")
            && reload(&marker).is_some_and(|id| id > removed)
    });
    let before_id = reload(&marker).unwrap();
    let before_page = fs::read(dist.join("pages/popup.html")).unwrap();
    let offset = log(&logfile).len();
    fs::write(&page, "include missing-dev-proof.crepus").unwrap();
    wait(&mut dev, &logfile, "invalid page warning", || {
        log(&logfile)
            .get(offset..)
            .is_some_and(|tail| tail.contains("missing-dev-proof.crepus"))
    });
    assert_eq!(
        fs::read(dist.join("pages/popup.html")).unwrap(),
        before_page
    );
    assert_eq!(reload(&marker), Some(before_id));
    assert!(dev.0.try_wait().unwrap().is_none());
    fs::write(&page, "span \"Recovered page\"").unwrap();
    wait(&mut dev, &logfile, "successful recovery", || {
        fs::read_to_string(dist.join("pages/popup.html"))
            .is_ok_and(|html| html.contains("Recovered page"))
            && reload(&marker).is_some_and(|id| id > before_id)
    });
    let recovered_id = reload(&marker).unwrap();
    fs::create_dir_all(root.join("views")).unwrap();
    let popup = root.join("views/popup.crepus");
    fs::write(&popup, "span \"Original authored popup\"").unwrap();
    wait(&mut dev, &logfile, "new authored popup directory", || {
        fs::read_to_string(dist.join("src/popup.html"))
            .is_ok_and(|html| html.contains("Original authored popup"))
            && reload(&marker).is_some_and(|id| id > recovered_id)
    });
    let before_popup_id = reload(&marker).unwrap();
    let before_popup = fs::read(dist.join("src/popup.html")).unwrap();
    let offset = log(&logfile).len();
    fs::write(&popup, "include missing-popup-dev-proof.crepus").unwrap();
    wait(&mut dev, &logfile, "invalid authored popup warning", || {
        log(&logfile)
            .get(offset..)
            .is_some_and(|tail| tail.contains("missing-popup-dev-proof.crepus"))
    });
    assert_eq!(fs::read(dist.join("src/popup.html")).unwrap(), before_popup);
    assert_eq!(reload(&marker), Some(before_popup_id));
    fs::write(&popup, "span \"Recovered authored popup\"").unwrap();
    wait(&mut dev, &logfile, "authored popup recovery", || {
        fs::read_to_string(dist.join("src/popup.html"))
            .is_ok_and(|html| html.contains("Recovered authored popup"))
            && reload(&marker).is_some_and(|id| id > before_popup_id)
    });
    let mut idle_id = reload(&marker);
    let mut unchanged_since = Instant::now();
    wait(
        &mut dev,
        &logfile,
        "idle after generated output writes",
        || {
            let observed = reload(&marker);
            if observed != idle_id {
                idle_id = observed;
                unchanged_since = Instant::now();
            }
            unchanged_since.elapsed() >= Duration::from_secs(1)
        },
    );
}
