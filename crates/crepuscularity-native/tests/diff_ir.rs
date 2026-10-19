use crepuscularity_native::{diff_ir, IrMutation, ViewIr, ViewNode};
use serde_json::json;

fn text(content: &str) -> ViewNode {
    serde_json::from_value(json!({"kind":"text","content":content})).unwrap()
}

#[test]
fn diff_ir_mismatched_versions_yields_replace_root() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Old")],
    };
    let new = ViewIr {
        version: 2,
        root: vec![text("New")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: vec![text("New")],
        }]
    );
}

#[test]
fn diff_ir_matching_versions_yields_node_diffs() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Old")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![text("New")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateText {
            path: vec![0],
            content: "New".to_string(),
        }]
    );
}

#[test]
fn diff_ir_empty_roots_matching_versions_yields_no_mutations() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 1,
        root: vec![],
    };

    let mutations = diff_ir(&old, &new);
    assert!(mutations.is_empty());
}
