use std::collections::HashMap;

use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_from_files, render_template_to_ir};
use serde_json::json;

#[test]
fn fallback_slot_keeps_wrapper_classes_layout_and_handler() {
    let ir = render_template_to_ir(
        "slot #content flex-row gap-2 p-4 @longpress=hold\n  span \"Fallback\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let node = serde_json::to_value(&ir).unwrap();
    let slot = &node["root"][0];
    assert_eq!(slot["style"]["id"], "content");
    assert_eq!(slot["axis"], "row");
    assert_eq!(slot["spacing"], 8.0);
    assert_eq!(slot["style"]["padding"], 16.0);
    assert_eq!(
        slot["style"]["classes"],
        json!(["flex-row", "gap-2", "p-4"])
    );
    assert_eq!(slot["onLongPress"], "hold");
    assert_eq!(slot["children"][0]["content"], "Fallback");
}

#[test]
fn supplied_slot_uses_callee_wrapper_classes_and_caller_text() {
    let files = HashMap::from([
        (
            "main.crepus".into(),
            "include card.crepus padding=\"p-4\"\n  span \"{title}\"".into(),
        ),
        (
            "card.crepus".into(),
            "slot {padding}\n  span \"Fallback\"".into(),
        ),
    ]);
    let mut context = TemplateContext::new();
    context.set("title", "Caller text");
    context.set("padding", "p-1");
    let ir = render_from_files(&files, "main.crepus", &context).unwrap();
    let node = serde_json::to_value(&ir).unwrap();
    assert_eq!(node["root"][0]["style"]["padding"], 16.0);
    assert_eq!(node["root"][0]["children"][0]["content"], "Caller text");
}
