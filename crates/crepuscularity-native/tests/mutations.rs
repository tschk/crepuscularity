use crepuscularity_native::{apply_mutations, diff_ir, IrMutation, ViewIr, ViewNode};
use serde_json::json;

#[test]
fn diff_ir_version_mismatch() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 2,
        root: vec![],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: new.root.clone()
        }]
    );
}

#[test]
fn diff_ir_version_match() {
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

#[test]
fn apply_mutations_replace_root() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![],
    };
    let new_root =
        vec![serde_json::from_value(json!({"kind": "text", "content": "Hello"})).unwrap()];
    let ops = vec![IrMutation::ReplaceRoot {
        root: new_root.clone(),
    }];

    assert!(apply_mutations(&mut ir, &ops).is_ok());
    assert_eq!(ir.root, new_root);
}

#[test]
fn apply_mutations_invalid_path() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![],
    };

    let ops = vec![IrMutation::RemoveNode { path: vec![0] }];

    assert!(apply_mutations(&mut ir, &ops).is_err());
}

#[test]
fn diff_ir_replace_node() {
    let old = ViewIr {
        version: 1,
        root: vec![serde_json::from_value(json!({"kind": "text", "content": "Before"})).unwrap()],
    };
    let new = ViewIr {
        version: 1,
        root: vec![serde_json::from_value(json!({"kind": "text", "content": "After"})).unwrap()],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateText {
            path: vec![0],
            content: "After".to_string()
        }]
    );
}

#[test]
fn diff_ir_insert_node() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 1,
        root: vec![serde_json::from_value(json!({"kind": "text", "content": "Hello"})).unwrap()],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::InsertNode {
            parent_path: vec![],
            index: 0,
            node: serde_json::from_value(json!({"kind": "text", "content": "Hello"})).unwrap()
        }]
    );
}

#[test]
fn diff_ir_remove_node() {
    let old = ViewIr {
        version: 1,
        root: vec![serde_json::from_value(json!({"kind": "text", "content": "Hello"})).unwrap()],
    };
    let new = ViewIr {
        version: 1,
        root: vec![],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(mutations, vec![IrMutation::RemoveNode { path: vec![0] }]);
}

#[test]
fn apply_mutations_complex() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value(
            json!({"kind": "stack", "axis": "column", "children": []}),
        )
        .unwrap()],
    };

    let new_node: ViewNode =
        serde_json::from_value(json!({"kind": "text", "content": "Child"})).unwrap();
    let ops = vec![IrMutation::InsertNode {
        parent_path: vec![0],
        index: 0,
        node: new_node.clone(),
    }];

    assert!(apply_mutations(&mut ir, &ops).is_ok());
    assert_eq!(
        ir.root[0],
        serde_json::from_value(json!({"kind": "stack", "axis": "column", "children": [new_node]}))
            .unwrap()
    );
}
