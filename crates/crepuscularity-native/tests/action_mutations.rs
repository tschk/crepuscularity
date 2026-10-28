use crepuscularity_native::{apply_mutations, diff_ir, ViewIr};
use serde_json::{json, Value};

fn fixtures() -> Vec<Value> {
    vec![
        json!({"kind":"stack","axis":"column","children":[]}),
        json!({"kind":"button","label":"Before","onClick":"click"}),
        json!({"kind":"image","src":"before.png","alt":"Before"}),
        json!({"kind":"scroll","axis":"column","children":[]}),
        json!({"kind":"toggle","label":"Before","checked":false,"onChange":"change"}),
        json!({"kind":"listItem","children":[]}),
    ]
}

fn ir(node: Value) -> ViewIr {
    serde_json::from_value(json!({"version":7,"root":[node]})).unwrap()
}

fn assert_patch(old: &ViewIr, desired: &ViewIr) {
    let mutations = diff_ir(old, desired);
    assert!(
        !mutations.is_empty(),
        "changed action produced no patch: {desired:?}"
    );
    let mut patched = old.clone();
    apply_mutations(&mut patched, &mutations).unwrap();
    assert_eq!(&patched, desired, "patch: {mutations:?}");
}

#[test]
fn long_press_add_change_and_remove_round_trip_for_every_supported_kind() {
    for node in fixtures() {
        let mut previous = ir(node.clone());
        for action in [Some("first.press"), Some("second.press"), None] {
            let mut next = node.clone();
            if let Some(action) = action {
                next["onLongPress"] = json!(action);
            }
            let desired = ir(next);
            assert_patch(&previous, &desired);
            previous = desired;
        }
    }
}

#[test]
fn nested_action_changes_survive_simultaneous_style_and_content_updates() {
    for mut node in fixtures() {
        node["onLongPress"] = json!("before.press");
        node["style"] = json!({"classes":["before"]});
        let before = json!({"kind":"stack","axis":"column","children":[
            {"kind":"text","content":"unchanged"},
            {"kind":"scroll","axis":"column","children":[node.clone()]}
        ]});
        node["onLongPress"] = json!("after.press");
        node["style"] = json!({"classes":["after"],"id":"updated"});
        match node["kind"].as_str().unwrap() {
            "stack" | "scroll" | "listItem" => {
                node["children"] = json!([{"kind":"text","content":"new child"}]);
            }
            "button" | "toggle" => node["label"] = json!("After"),
            "image" => node["src"] = json!("after.png"),
            _ => unreachable!(),
        }
        let after = json!({"kind":"stack","axis":"column","children":[
            {"kind":"text","content":"unchanged"},
            {"kind":"scroll","axis":"column","children":[node]}
        ]});
        assert_patch(&ir(before), &ir(after));
    }
}

#[test]
fn unchanged_specialized_actions_and_trees_produce_no_mutations() {
    for mut node in fixtures().into_iter().filter(|node| {
        matches!(
            node["kind"].as_str(),
            Some("stack" | "button" | "image" | "scroll")
        )
    }) {
        for action in [None, Some("same.press")] {
            if let Some(action) = action {
                node["onLongPress"] = json!(action);
            }
            let tree = ir(node.clone());
            assert!(diff_ir(&tree, &tree.clone()).is_empty());
        }
    }
}
