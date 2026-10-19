use crepuscularity_native::ir::{ViewIr, ViewNode};
use crepuscularity_native::mutations::{diff_ir, IrMutation};

#[test]
fn test_diff_ir_different_versions() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 2,
        root: vec![ViewNode::Text {
            content: "Hello".into(),
            bind: None,
            style: None,
        }],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: vec![ViewNode::Text {
                content: "Hello".into(),
                bind: None,
                style: None,
            }],
        }]
    );
}

#[test]
fn test_diff_ir_same_version() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "Hello".into(),
            bind: None,
            style: None,
        }],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::InsertNode {
            parent_path: vec![],
            index: 0,
            node: ViewNode::Text {
                content: "Hello".into(),
                bind: None,
                style: None,
            },
        }]
    );
}
