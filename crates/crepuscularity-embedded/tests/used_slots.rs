#![cfg(feature = "std")]

use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_embedded::{render_nodes_to_document, EmbeddedNode, ScreenSize};
use std::{path::PathBuf, sync::Arc};

fn texts(nodes: &[EmbeddedNode]) -> Vec<String> {
    let mut out = Vec::new();
    for node in nodes {
        if let Some(text) = &node.text {
            out.push(text.clone());
        }
        out.extend(texts(&node.children));
    }
    out
}

#[test]
fn used_slots_keep_caller_base_directory_through_nested_forwarding() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("root"));
    Arc::make_mut(&mut ctx.virtual_files).extend([
        (
            "root/shell/frame.crepus".into(),
            "include leaf.crepus\n  slot".into(),
        ),
        ("root/shell/leaf.crepus".into(), "slot".into()),
        ("root/piece.crepus".into(), "span \"caller piece\"".into()),
        (
            "root/shell/piece.crepus".into(),
            "span \"wrong piece\"".into(),
        ),
    ]);
    let nodes = parse_template("include shell/frame.crepus\n  include piece.crepus").unwrap();
    let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(texts(&document.root), ["caller piece"]);
}

#[test]
fn used_named_slots_keep_caller_locals_and_loop_aliases_separate_from_props() {
    let mut ctx = TemplateContext::new();
    let rows = ["One", "Two"]
        .into_iter()
        .map(|name| {
            let mut row = TemplateContext::new();
            row.set("name", name);
            row
        })
        .collect();
    ctx.set("rows", TemplateValue::List(rows));
    Arc::make_mut(&mut ctx.virtual_files).insert("cards.crepus".into(),
        "+++\n[Card.defaults]\ntitle = 'Default'\n+++\n--- Card\nspan \"{title}\"\nslot\n  span \"unused fallback\"".into());
    let nodes=parse_template("$: let prefix = \"Caller\"\nfor row in rows\n  include cards.crepus#Card title=\"Child\"\n    span \"{prefix}|{row.name}|{title}\"\ninclude cards.crepus#Card\n  span \"last\"").unwrap();
    let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(
        texts(&document.root),
        [
            "Child",
            "Caller|One|",
            "Child",
            "Caller|Two|",
            "Default",
            "last"
        ]
    );
}

#[test]
fn borrowed_forwarding_keeps_external_owned_slot_context_and_bundle() {
    let mut captured = TemplateContext::new();
    captured.base_dir = Some(PathBuf::from("foreign"));
    Arc::make_mut(&mut captured.virtual_files).insert(
        "foreign/piece.crepus".into(),
        "span \"captured piece\"".into(),
    );
    let mut root = TemplateContext::new();
    root.slot = Some((
        parse_template("include piece.crepus").unwrap(),
        Arc::new(captured),
    ));
    Arc::make_mut(&mut root.virtual_files).extend([
        ("frame.crepus".into(), "slot".into()),
        ("piece.crepus".into(), "span \"wrong piece\"".into()),
    ]);
    let nodes = parse_template("include frame.crepus\n  slot").unwrap();
    let document = render_nodes_to_document(&nodes, &root, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(texts(&document.root), ["captured piece"]);
}
