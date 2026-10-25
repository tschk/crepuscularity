//! Snapshot semantics that lexical runtime scopes must preserve.
use std::collections::HashMap;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::render_from_files;
use serde_json::Value;

fn text_snapshots(node: &Value, output: &mut Vec<String>) {
    if node["kind"] == "text" {
        output.push(node["content"].as_str().unwrap().to_owned());
    }
    if let Some(children) = node["children"].as_array() {
        for child in children {
            text_snapshots(child, output);
        }
    }
}

fn snapshots(files: &[(&str, &str)], ctx: &TemplateContext) -> Vec<String> {
    let files: HashMap<_, _> = files
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let ir = render_from_files(&files, "main.crepus", ctx).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    let mut output = Vec::new();
    for node in json["root"].as_array().unwrap() {
        text_snapshots(node, &mut output);
    }
    output
}

#[test]
fn includes_are_isolated_and_props_read_the_caller() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    ctx.set("secret", "Root secret");
    assert_eq!(
        snapshots(
            &[
                ("main.crepus", "include card.crepus title={title}"),
                ("card.crepus", "text \"{title}\"\ntext \"{secret}\""),
            ],
            &ctx
        ),
        ["Root", ""]
    );
}

#[test]
fn nested_slots_capture_their_own_caller_not_the_nearest_component() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    assert_eq!(
        snapshots(
            &[
                (
                    "main.crepus",
                    "include outer.crepus title=\"Outer\"\n  text \"{title}\""
                ),
                (
                    "outer.crepus",
                    "include inner.crepus title=\"Inner\"\n  text \"{title}\"\n  slot"
                ),
                ("inner.crepus", "text \"{title}\"\nslot"),
            ],
            &ctx
        ),
        ["Inner", "Outer", "Root"]
    );
}

#[test]
fn declarations_are_ordered_and_do_not_escape_the_include() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    assert_eq!(
        snapshots(
            &[
                (
                    "main.crepus",
                    "include card.crepus title={title}\ntext \"{title}\""
                ),
                (
                    "card.crepus",
                    "text \"{title}\"\n$: let title = \"Local\"\ntext \"{title}\""
                ),
            ],
            &ctx
        ),
        ["Root", "Local", "Root"]
    );
}

#[test]
fn explicit_null_overrides_defaults_and_fallback_slots_use_callee_scope() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    assert_eq!(
        snapshots(
            &[
                ("main.crepus", "include components.crepus#Card title={null}"),
                (
                    "components.crepus",
                    "+++\n[Card.defaults]\ntitle = \"Metadata default\"\n+++\n--- Card\n$: default title = \"Declaration default\"\nslot\n  text \"{title}\""
                ),
            ],
            &ctx
        ),
        [""]
    );
}

#[test]
fn mixed_include_text_keeps_its_snapshot_without_disabling_root_siblings() {
    let files = HashMap::from([
        (
            "main.crepus".into(),
            "include card.crepus title=\"Local\"\ntext \"Root: {title}!\"".into(),
        ),
        ("card.crepus".into(), "text \"Card: {title}!\"".into()),
    ]);
    let mut context = TemplateContext::new();
    context.set("title", "Root");
    let ir = render_from_files(&files, "main.crepus", &context).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["content"], "Card: Local!");
    assert!(json["root"][0]["bind"].is_null());
    assert_eq!(json["root"][1]["content"], "Root: Root!");
    assert!(json["root"][1]["bind"].is_string());
}

#[test]
fn mixed_text_after_local_declarations_retains_the_evaluated_snapshot() {
    let files = HashMap::from([(
        "main.crepus".into(),
        "text \"Before: {title}!\"\n$: let title = \"Local\"\ntext \"After: {title}!\"".into(),
    )]);
    let mut context = TemplateContext::new();
    context.set("title", "Root");
    let ir = render_from_files(&files, "main.crepus", &context).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["content"], "Before: Root!");
    assert!(json["root"][0]["bind"].is_string());
    assert_eq!(json["root"][1]["content"], "After: Local!");
    assert!(json["root"][1]["bind"].is_null());
}

#[test]
fn mixed_named_component_default_text_does_not_read_unrelated_host_state() {
    let files = HashMap::from([(
        "components.crepus".into(),
        "+++\n[Card.defaults]\ntitle = \"Metadata default\"\n+++\n--- Card\ntext \"Card: {title}!\"".into(),
    )]);
    let ir = render_from_files(&files, "components.crepus#Card", &TemplateContext::new()).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["content"], "Card: Metadata default!");
    assert!(json["root"][0]["bind"].is_null());
}
