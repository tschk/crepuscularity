use crepuscularity_native::ir::ViewNode;
use crepuscularity_native::{apply_mutations, diff_ir, IrMutation, ViewIr};
use serde_json::json;

#[test]
fn test_diff_ir_version_mismatch() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 2,
        root: vec![],
    };
    let mutations = diff_ir(&old, &new);
    assert_eq!(mutations.len(), 1);
    assert_eq!(
        mutations[0],
        IrMutation::ReplaceRoot {
            root: new.root.clone()
        }
    );
}

#[test]
fn test_apply_mutations_empty() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![],
    };
    apply_mutations(&mut ir, &[]).unwrap();
    assert_eq!(ir.version, 1);
    assert_eq!(ir.root.len(), 0);
}

#[test]
fn test_apply_mutations_invalid_path() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![],
    };

    let ops = vec![IrMutation::RemoveNode { path: vec![] }];
    let result = apply_mutations(&mut ir, &ops);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "cannot remove root list directly; use ReplaceRoot"
    );

    let ops2 = vec![IrMutation::RemoveNode { path: vec![0] }];
    let result2 = apply_mutations(&mut ir, &ops2);
    assert!(result2.is_err());
    assert_eq!(
        result2.unwrap_err(),
        "remove index 0 out of bounds for parent path []"
    );
}

#[test]
fn test_diff_ir_same_version() {
    let old = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "old".to_string(),
            bind: None,
            style: None,
        }],
    };
    let new = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "new".to_string(),
            bind: None,
            style: None,
        }],
    };
    let mutations = diff_ir(&old, &new);
    assert_eq!(mutations.len(), 1);
    assert_eq!(
        mutations[0],
        IrMutation::UpdateText {
            path: vec![0],
            content: "new".to_string(),
        }
    );
}

#[test]
fn test_apply_mutations_replace_root() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![],
    };
    let new_root = vec![ViewNode::Text {
        content: "new".to_string(),
        bind: None,
        style: None,
    }];
    let ops = vec![IrMutation::ReplaceRoot {
        root: new_root.clone(),
    }];
    apply_mutations(&mut ir, &ops).unwrap();
    assert_eq!(ir.root, new_root);
}

#[test]
fn test_diff_ir_and_apply_mutations_integration() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "old".to_string(),
            bind: None,
            style: None,
        }],
    };
    let new_ir = ViewIr {
        version: 1,
        root: vec![
            ViewNode::Text {
                content: "new".to_string(),
                bind: None,
                style: None,
            },
            ViewNode::Text {
                content: "new2".to_string(),
                bind: None,
                style: None,
            },
        ],
    };
    let mutations = diff_ir(&ir, &new_ir);
    apply_mutations(&mut ir, &mutations).unwrap();
    assert_eq!(ir, new_ir);
}

#[test]
fn apply_mutations_insert_node() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "stack",
            "axis": "column",
            "children": []
        }))
        .unwrap()],
    };

    let node = serde_json::from_value::<ViewNode>(json!({
        "kind": "text",
        "content": "Hello"
    }))
    .unwrap();

    let ops = vec![IrMutation::InsertNode {
        parent_path: vec![0],
        index: 0,
        node: node.clone(),
    }];

    apply_mutations(&mut ir, &ops).unwrap();

    assert_eq!(
        ir.root,
        vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "stack",
            "axis": "column",
            "children": [
                {
                    "kind": "text",
                    "content": "Hello"
                }
            ]
        }))
        .unwrap()]
    );
}

#[test]
fn apply_mutations_remove_node() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "stack",
            "axis": "column",
            "children": [
                {
                    "kind": "text",
                    "content": "Hello"
                }
            ]
        }))
        .unwrap()],
    };

    let ops = vec![IrMutation::RemoveNode { path: vec![0, 0] }];

    apply_mutations(&mut ir, &ops).unwrap();

    assert_eq!(
        ir.root,
        vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "stack",
            "axis": "column",
            "children": []
        }))
        .unwrap()]
    );
}

#[test]
fn apply_mutations_replace_node() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "Old"
        }))
        .unwrap()],
    };

    let new_node = serde_json::from_value::<ViewNode>(json!({
        "kind": "text",
        "content": "New"
    }))
    .unwrap();

    let ops = vec![IrMutation::ReplaceNode {
        path: vec![0],
        node: new_node.clone(),
    }];

    apply_mutations(&mut ir, &ops).unwrap();

    assert_eq!(ir.root, vec![new_node]);
}

#[test]
fn diff_ir_version_mismatch_returns_replace_root() {
    let old_ir: ViewIr = serde_json::from_value(json!({
        "version": 1,
        "root": []
    }))
    .unwrap();

    let new_root = vec![serde_json::from_value::<ViewNode>(json!({
        "kind": "text",
        "content": "Hello"
    }))
    .unwrap()];

    let new_ir = ViewIr {
        version: 2,
        root: new_root.clone(),
    };

    let mutations = diff_ir(&old_ir, &new_ir);

    assert_eq!(mutations.len(), 1);
    match &mutations[0] {
        IrMutation::ReplaceRoot { root } => {
            assert_eq!(root, &new_root);
        }
        _ => panic!("Expected ReplaceRoot mutation"),
    }
}

#[test]
fn diff_ir_same_version_returns_node_diffs() {
    let old_ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "Old"
        }))
        .unwrap()],
    };

    let new_ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "New"
        }))
        .unwrap()],
    };

    let mutations = diff_ir(&old_ir, &new_ir);

    assert_eq!(mutations.len(), 1);
    match &mutations[0] {
        IrMutation::UpdateText { path, content } => {
            assert_eq!(path, &vec![0]);
            assert_eq!(content, "New");
        }
        _ => panic!("Expected UpdateText mutation"),
    }
}

#[test]
fn apply_mutations_out_of_bounds_errors() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "stack",
            "axis": "column",
            "children": []
        }))
        .unwrap()],
    };

    let node = serde_json::from_value::<ViewNode>(json!({
        "kind": "text",
        "content": "Hello"
    }))
    .unwrap();

    let ops = vec![IrMutation::InsertNode {
        parent_path: vec![0],
        index: 1,
        node: node.clone(),
    }];

    assert!(apply_mutations(&mut ir, &ops).is_err());

    let ops2 = vec![IrMutation::RemoveNode { path: vec![0, 0] }];
    assert!(apply_mutations(&mut ir, &ops2).is_err());
}

#[test]
fn apply_mutations_empty_ops_does_nothing() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "Old"
        }))
        .unwrap()],
    };
    let ir_clone = ir.clone();

    apply_mutations(&mut ir, &[]).unwrap();
    assert_eq!(ir, ir_clone);
}

#[test]
fn apply_mutations_update_style() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "Old",
            "style": {
                "id": "test"
            }
        }))
        .unwrap()],
    };

    let style = serde_json::from_value(json!({
        "id": "new_test",
        "textColor": "red"
    }))
    .unwrap();

    let ops = vec![IrMutation::UpdateStyle {
        path: vec![0],
        style: Some(style),
    }];

    apply_mutations(&mut ir, &ops).unwrap();

    match &ir.root[0] {
        ViewNode::Text { style: Some(s), .. } => {
            assert_eq!(s.id, Some("new_test".to_string()));
        }
        _ => panic!("Expected text node with style"),
    }
}

#[test]
fn apply_mutations_invalid_path_errors() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "Old"
        }))
        .unwrap()],
    };

    let node = serde_json::from_value::<ViewNode>(json!({
        "kind": "text",
        "content": "Hello"
    }))
    .unwrap();

    let ops = vec![IrMutation::ReplaceNode {
        path: vec![1], // Invalid index
        node: node.clone(),
    }];
    assert!(apply_mutations(&mut ir, &ops).is_err());

    let ops2 = vec![IrMutation::ReplaceNode {
        path: vec![], // Empty path
        node: node.clone(),
    }];
    assert!(apply_mutations(&mut ir, &ops2).is_err());
}

#[test]
fn diff_node_lists_insertions_and_deletions() {
    let old_ir = ViewIr {
        version: 1,
        root: vec![serde_json::from_value::<ViewNode>(json!({
            "kind": "text",
            "content": "1"
        }))
        .unwrap()],
    };

    let new_ir = ViewIr {
        version: 1,
        root: vec![
            serde_json::from_value::<ViewNode>(json!({
                "kind": "text",
                "content": "1"
            }))
            .unwrap(),
            serde_json::from_value::<ViewNode>(json!({
                "kind": "text",
                "content": "2"
            }))
            .unwrap(),
        ],
    };

    let mutations = diff_ir(&old_ir, &new_ir);

    assert_eq!(mutations.len(), 1);
    match &mutations[0] {
        IrMutation::InsertNode {
            parent_path,
            index,
            node: _,
        } => {
            assert_eq!(parent_path, &Vec::<usize>::new());
            assert_eq!(index, &1);
        }
        _ => panic!("Expected InsertNode mutation"),
    }

    let mutations_rev = diff_ir(&new_ir, &old_ir);

    assert_eq!(mutations_rev.len(), 1);
    match &mutations_rev[0] {
        IrMutation::RemoveNode { path } => {
            assert_eq!(path, &vec![1]);
        }
        _ => panic!("Expected RemoveNode mutation"),
    }
}
