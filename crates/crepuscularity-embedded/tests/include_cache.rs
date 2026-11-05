#![cfg(feature = "std")]

use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_embedded::{
    render_nodes_to_document, Rgb565Buffer, ScreenSize, Template, DEFAULT_BG,
};
use std::sync::Arc;

#[test]
fn each_draw_observes_disk_and_virtual_include_changes() {
    let dir = tempfile::tempdir().unwrap();
    let child = dir.path().join("child.crepus");
    std::fs::write(&child, "span \"First\"").unwrap();
    let screen = ScreenSize::new(128, 64);
    let mut template = Template::from_source_with_path(
        "include child.crepus",
        dir.path().join("entry.crepus"),
        screen,
    );
    let mut fb = Rgb565Buffer::new(screen, DEFAULT_BG);
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("First"));
    std::fs::write(&child, "span \"Later\"").unwrap();
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("Later"));
    Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("child.crepus".into(), "span \"Virtual one\"".into());
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("Virtual one"));
    Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("child.crepus".into(), "span \"Virtual two\"".into());
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("Virtual two"));
}

#[test]
fn slot_context_uses_its_own_virtual_bundle_for_same_path() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "span \"Outer\"".into());
    let mut slot_ctx = TemplateContext::new();
    Arc::make_mut(&mut slot_ctx.virtual_files)
        .insert("child.crepus".into(), "span \"Inner\"".into());
    ctx.slot = Some((
        parse_template("include child.crepus").unwrap(),
        Arc::new(slot_ctx),
    ));
    let nodes = parse_template("include child.crepus\nslot").unwrap();
    let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(document.root[0].text.as_deref(), Some("Outer"));
    assert_eq!(document.root[1].children[0].text.as_deref(), Some("Inner"));
}
