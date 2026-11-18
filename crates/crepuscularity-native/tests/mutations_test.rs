use crepuscularity_native::ir::{ViewIr, ViewNode, ViewStyle};
use crepuscularity_native::mutations::{apply_mutations, diff_ir, IrMutation};

fn text(content: &str) -> ViewNode {
    ViewNode::Text {
        content: content.to_string(),
        bind: None,
        style: None,
    }
}

fn text_with_style(content: &str, style: ViewStyle) -> ViewNode {
    ViewNode::Text {
        content: content.to_string(),
        bind: None,
        style: Some(style),
    }
}

fn button(label: &str) -> ViewNode {
    ViewNode::Button {
        label: label.to_string(),
        label_parts: vec![],
        on_click: None,
        on_long_press: None,
        style: None,
    }
}

#[test]
fn test_diff_ir_different_version() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };
    let new = ViewIr {
        version: 2,
        root: vec![text("World")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceRoot {
            root: vec![text("World")]
        }]
    );
}

#[test]
fn test_diff_ir_same_version_insert_node() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![text("Hello"), text("World")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::InsertNode {
            parent_path: vec![],
            index: 1,
            node: text("World")
        }]
    );
}

#[test]
fn test_diff_ir_same_version_remove_node() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello"), text("World")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(mutations, vec![IrMutation::RemoveNode { path: vec![1] }]);
}

#[test]
fn test_diff_ir_same_version_replace_node() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![button("Hello")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::ReplaceNode {
            path: vec![0],
            node: button("Hello")
        }]
    );
}

#[test]
fn test_diff_ir_update_text() {
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![text("World")],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateText {
            path: vec![0],
            content: "World".to_string()
        }]
    );
}

#[test]
fn test_diff_ir_update_style() {
    let style = ViewStyle {
        background_color: Some(String::from("red")),
        ..Default::default()
    };
    let old = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };
    let new = ViewIr {
        version: 1,
        root: vec![text_with_style("Hello", style.clone())],
    };

    let mutations = diff_ir(&old, &new);
    assert_eq!(
        mutations,
        vec![IrMutation::UpdateStyle {
            path: vec![0],
            style: Some(style)
        }]
    );
}

#[test]
fn test_apply_mutations() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };

    let ops = vec![
        IrMutation::UpdateText {
            path: vec![0],
            content: "World".to_string(),
        },
        IrMutation::InsertNode {
            parent_path: vec![],
            index: 1,
            node: button("Click Me"),
        },
    ];

    apply_mutations(&mut ir, &ops).unwrap();

    assert_eq!(
        ir,
        ViewIr {
            version: 1,
            root: vec![text("World"), button("Click Me")],
        }
    );
}

#[test]
fn test_apply_mutations_error() {
    let mut ir = ViewIr {
        version: 1,
        root: vec![text("Hello")],
    };

    let ops = vec![
        IrMutation::UpdateText {
            path: vec![0],
            content: "World".to_string(),
        },
        IrMutation::RemoveNode { path: vec![99] },
    ];

    let result = apply_mutations(&mut ir, &ops);
    assert!(result.is_err());

    // Should remain unchanged due to atomic application
    assert_eq!(
        ir,
        ViewIr {
            version: 1,
            root: vec![text("Hello")],
        }
    );
}
