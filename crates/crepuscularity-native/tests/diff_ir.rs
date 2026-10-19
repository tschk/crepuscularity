use crepuscularity_native::{diff_ir, IrMutation, ViewIr};
use serde_json::json;

#[test]
fn different_versions_return_replace_root() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };

    let new_with_root: ViewIr = serde_json::from_value(json!({
        "version": 2,
        "root": [
            {"kind": "text", "content": "Hello"}
        ]
    }))
    .unwrap();

    let mutations = diff_ir(&old, &new_with_root);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: new_with_root.root.clone()
        }]
    );
}

#[test]
fn diff_ir_diffs_roots_correctly() {
    let old: ViewIr = serde_json::from_value(json!({
        "version": 1,
        "root": [
            {"kind": "text", "content": "Old"}
        ]
    }))
    .unwrap();

    let new: ViewIr = serde_json::from_value(json!({
        "version": 1,
        "root": [
            {"kind": "text", "content": "New"}
        ]
    }))
    .unwrap();

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateText {
            path: vec![0],
            content: "New".to_string()
        }]
    );
}
