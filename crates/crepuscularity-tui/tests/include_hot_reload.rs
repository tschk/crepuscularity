use crepuscularity_tui::{HotTemplate, ReloadOutcome};
use ratatui::{backend::TestBackend, Terminal};

#[cfg(unix)]
#[test]
fn symlink_target_atomic_save_preserves_vue_frontend() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("source.txt");
    let link = dir.path().join("view.vue");
    std::fs::write(&target, "<template><div>Before</div></template>").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut hot = HotTemplate::watch(&link).unwrap();
    let replacement = dir.path().join("replacement.txt");
    std::fs::write(
        &replacement,
        "<script>const ignored = 1 < 2;</script><template><div>Atomic Vue</div></template>",
    )
    .unwrap();
    std::fs::rename(&replacement, &target).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !hot.has_pending_change() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(hot.poll_reload(), ReloadOutcome::Reloaded);
    let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
    terminal
        .draw(|frame| hot.template().draw_full(frame).unwrap())
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Atomic Vue"), "{output}");
}

#[test]
fn included_vue_edit_triggers_reload_and_conditional_redraw() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("entry.crepus");
    let child = dir.path().join("child.vue");
    std::fs::write(&entry, "include child.vue").unwrap();
    std::fs::write(&child, "<template><div>Before</div></template>").unwrap();
    let mut hot = HotTemplate::watch(&entry).unwrap();
    hot.template_mut().mark_rendered();
    std::fs::write(&child, "<template><div>After</div></template>").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !hot.has_pending_change() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        hot.has_pending_change(),
        "included Vue edit did not reach the watcher"
    );
    assert_eq!(hot.poll_reload(), ReloadOutcome::Reloaded);
    let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
    terminal
        .draw(|frame| assert!(hot.template_mut().draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("After"), "{output}");
}

#[test]
fn malformed_include_displays_error_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("entry.crepus");
    let child = dir.path().join("child.vue");
    std::fs::write(&entry, "include child.vue").unwrap();
    std::fs::write(&child, "<template><div>Before</div></template>").unwrap();
    let mut hot = HotTemplate::watch(&entry).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
    terminal
        .draw(|frame| {
            hot.poll_and_draw_full(frame).unwrap();
        })
        .unwrap();
    for (source, valid) in [
        ("<template><div>", false),
        ("<template><div>Recovered</div></template>", true),
    ] {
        std::fs::write(&child, source).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !hot.has_pending_change() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(hot.has_pending_change());
        terminal
            .draw(|frame| assert!(hot.poll_and_draw_full(frame).is_ok()))
            .unwrap();
        if !valid {
            let output: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(output.contains("parse error"), "{output}");
        }
    }
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Recovered"), "{output}");
}
