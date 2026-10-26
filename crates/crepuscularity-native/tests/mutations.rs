use crepuscularity_native::ir::ViewNode;
use crepuscularity_native::{apply_mutations, diff_ir, IrMutation, ViewIr};

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
