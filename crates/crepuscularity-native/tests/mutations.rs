use crepuscularity_native::ir::{StackAxis, ViewIr, ViewNode, ViewStyle};
use crepuscularity_native::mutations::{apply_mutations, diff_ir, IrMutation};

#[test]
fn diffing_different_versions_replaces_root() {
    let old = ViewIr {
        version: 1,
        root: vec![],
    };
    let new = ViewIr {
        version: 2,
        root: vec![ViewNode::Text {
            content: "new".into(),
            bind: None,
            style: None,
        }],
    };
    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: vec![ViewNode::Text {
                content: "new".into(),
                bind: None,
                style: None,
            }]
        }]
    );
}

#[test]
fn apply_mutations_works() {
    let mut old = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "old".into(),
            bind: None,
            style: None,
        }],
    };
    let mutations = vec![IrMutation::UpdateText {
        path: vec![0],
        content: "new".into(),
    }];
    apply_mutations(&mut old, &mutations).unwrap();
    assert_eq!(
        old,
        ViewIr {
            version: 1,
            root: vec![ViewNode::Text {
                content: "new".into(),
                bind: None,
                style: None,
            }],
        }
    );
}

#[test]
fn apply_replace_root_mutation() {
    let mut doc = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "old".into(),
            bind: None,
            style: None,
        }],
    };
    let mutations = vec![IrMutation::ReplaceRoot {
        root: vec![ViewNode::Text {
            content: "new".into(),
            bind: None,
            style: None,
        }],
    }];
    apply_mutations(&mut doc, &mutations).unwrap();
    assert_eq!(
        doc.root,
        vec![ViewNode::Text {
            content: "new".into(),
            bind: None,
            style: None,
        }]
    );
}

#[test]
fn diffing_and_applying_mutations() {
    let old = ViewIr {
        version: 1,
        root: vec![
            ViewNode::Text {
                content: "old".into(),
                bind: None,
                style: None,
            },
            ViewNode::Stack {
                axis: StackAxis::Column,
                align_items: None,
                justify_content: None,
                spacing: None,
                on_long_press: None,
                style: None,
                children: vec![ViewNode::Text {
                    content: "child1".into(),
                    bind: None,
                    style: None,
                }],
            },
        ],
    };

    let new = ViewIr {
        version: 1,
        root: vec![
            ViewNode::Text {
                content: "new".into(),
                bind: None,
                style: None,
            },
            ViewNode::Stack {
                axis: StackAxis::Column,
                align_items: None,
                justify_content: None,
                spacing: None,
                on_long_press: None,
                style: None,
                children: vec![
                    ViewNode::Text {
                        content: "child1".into(),
                        bind: None,
                        style: None,
                    },
                    ViewNode::Text {
                        content: "child2".into(),
                        bind: None,
                        style: None,
                    },
                ],
            },
            ViewNode::Text {
                content: "inserted".into(),
                bind: None,
                style: None,
            },
        ],
    };

    let mutations = diff_ir(&old, &new);
    let mut doc = old.clone();
    apply_mutations(&mut doc, &mutations).unwrap();

    assert_eq!(doc, new);
}

#[test]
fn diffing_removes_nodes() {
    let old = ViewIr {
        version: 1,
        root: vec![
            ViewNode::Text {
                content: "1".into(),
                bind: None,
                style: None,
            },
            ViewNode::Text {
                content: "2".into(),
                bind: None,
                style: None,
            },
        ],
    };

    let new = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "1".into(),
            bind: None,
            style: None,
        }],
    };

    let mutations = diff_ir(&old, &new);
    let mut doc = old.clone();
    apply_mutations(&mut doc, &mutations).unwrap();

    assert_eq!(doc, new);
}

#[test]
fn diffing_different_styles() {
    let old_style = ViewStyle {
        id: Some("old_id".into()),
        ..Default::default()
    };

    let new_style = ViewStyle {
        id: Some("new_id".into()),
        ..Default::default()
    };

    let old = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "text".into(),
            bind: None,
            style: Some(old_style.clone()),
        }],
    };

    let new = ViewIr {
        version: 1,
        root: vec![ViewNode::Text {
            content: "text".into(),
            bind: None,
            style: Some(new_style.clone()),
        }],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateStyle {
            path: vec![0],
            style: Some(new_style),
        }]
    );

    let mut doc = old.clone();
    apply_mutations(&mut doc, &mutations).unwrap();

    assert_eq!(doc, new);
}

#[test]
fn errors_out_of_bounds_mutations() {
    let mut doc = ViewIr {
        version: 1,
        root: vec![],
    };

    // Removing from empty root
    let err = apply_mutations(&mut doc, &[IrMutation::RemoveNode { path: vec![0] }]);
    assert!(err.is_err());

    // Inserting out of bounds
    let err = apply_mutations(
        &mut doc,
        &[IrMutation::InsertNode {
            parent_path: vec![],
            index: 1,
            node: ViewNode::Text {
                content: "".into(),
                bind: None,
                style: None,
            },
        }],
    );
    assert!(err.is_err());

    // Update text on non-text node
    doc.root = vec![ViewNode::Stack {
        axis: StackAxis::Column,
        align_items: None,
        justify_content: None,
        spacing: None,
        on_long_press: None,
        style: None,
        children: vec![],
    }];
    let err = apply_mutations(
        &mut doc,
        &[IrMutation::UpdateText {
            path: vec![0],
            content: "test".into(),
        }],
    );
    assert!(err.is_err());
}
