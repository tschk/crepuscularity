use crepuscularity_native::{apply_mutations, IrMutation, ViewIr, ViewNode};
use serde_json::json;

fn text(content: &str) -> ViewNode {
    serde_json::from_value(json!({"kind":"text","content":content})).unwrap()
}

fn original() -> ViewIr {
    ViewIr {
        version: 7,
        root: vec![
            text("Original"),
            serde_json::from_value(json!({"kind":"button","label":"Stable"})).unwrap(),
        ],
    }
}

#[test]
fn failing_later_operation_does_not_commit_any_successful_prefix() {
    let cases = [
        vec![
            IrMutation::UpdateText {
                path: vec![0],
                content: "Changed".into(),
            },
            IrMutation::RemoveNode { path: vec![9] },
        ],
        vec![
            IrMutation::ReplaceRoot {
                root: vec![text("Replacement")],
            },
            IrMutation::InsertNode {
                parent_path: vec![],
                index: 2,
                node: text("Invalid insertion"),
            },
        ],
        vec![
            IrMutation::InsertNode {
                parent_path: vec![],
                index: 0,
                node: text("Inserted"),
            },
            IrMutation::UpdateText {
                path: vec![2],
                content: "Button cannot become text".into(),
            },
        ],
    ];
    for operations in cases {
        let before = original();
        let mut patched = before.clone();
        assert!(apply_mutations(&mut patched, &operations).is_err());
        assert_eq!(patched, before, "failed transaction: {operations:?}");
    }
}

#[test]
fn successful_operations_observe_prior_insertions_removals_and_shifted_paths() {
    let mut patched = original();
    let operations = vec![
        IrMutation::InsertNode {
            parent_path: vec![],
            index: 0,
            node: text("Inserted"),
        },
        IrMutation::UpdateText {
            path: vec![1],
            content: "Updated original".into(),
        },
        IrMutation::RemoveNode { path: vec![0] },
        IrMutation::UpdateText {
            path: vec![0],
            content: "Final".into(),
        },
    ];
    apply_mutations(&mut patched, &operations).unwrap();
    let mut desired = original();
    desired.root[0] = text("Final");
    assert_eq!(patched, desired);
}

#[test]
fn empty_patch_keeps_the_document_unchanged() {
    let before = original();
    let mut patched = before.clone();
    apply_mutations(&mut patched, &[]).unwrap();
    assert_eq!(patched, before);
}
