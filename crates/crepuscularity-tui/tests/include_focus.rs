use crepuscularity_tui::Template;

#[test]
fn includes_discover_scoped_ids_and_caller_slots() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("child.vue"), r#"<template><div><button :id="name" class="focusable">Included</button><slot><button id="fallback" class="focusable">Fallback</button></slot></div></template>"#).unwrap();
    let mut template = Template::from_source_with_path(
        "include child.vue name={child_name}\n  button bind:id={caller_name} focusable\n    \"Caller\"",
        dir.path().join("entry.crepus"),
    );
    template.set("child_name", "included");
    template.set("caller_name", "caller");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["included", "caller"]);
    template.set_source("include child.vue name={child_name}");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["included", "fallback"]);
}

#[test]
fn virtual_named_component_focus_and_recursive_include_recovery() {
    let mut template = Template::from_source("include child.crepus#Card");
    std::sync::Arc::make_mut(&mut template.context_mut().virtual_files).insert(
        "child.crepus".into(),
        "--- Card\n$: default name = \"default-id\"\nbutton bind:id={name} focusable\n  \"Card\""
            .into(),
    );
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["default-id"]);
    template.set_source("include cycle.crepus");
    std::sync::Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("cycle.crepus".into(), "include cycle.crepus".into());
    template.refresh_focus();
    assert!(template.focus().focusable_ids.is_empty());
    template.set_source("button #recovered focusable");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["recovered"]);
}

#[cfg(unix)]
#[test]
fn ordinary_symlink_include_uses_authored_angular_frontend_for_focus_and_render() {
    use ratatui::{backend::TestBackend, Terminal};
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("source.txt");
    std::fs::write(
        &target,
        r#"<button [attr.id]="name" class="focusable">Angular included</button>"#,
    )
    .unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join("child.component.html")).unwrap();
    let mut template = Template::from_source_with_path(
        "include child.component.html name={name}",
        dir.path().join("entry.crepus"),
    );
    template.set("name", "included-angular");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["included-angular"]);
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    for _ in 0..2 {
        terminal
            .draw(|frame| template.draw_full(frame).unwrap())
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Angular included"), "{text}");
        assert!(!text.contains("parse error"), "{text}");
    }
}
