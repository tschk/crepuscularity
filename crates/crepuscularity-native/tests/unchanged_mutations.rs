use crepuscularity_native::{apply_mutations, diff_ir, IrMutation, ViewIr};
use serde_json::{json, Value};

fn nodes() -> Vec<Value> {
    vec![
        json!({"kind":"text","content":"Stable"}),
        json!({"kind":"if","condition":"visible","thenChildren":[],"elseChildren":[]}),
        json!({"kind":"forEach","bind":"rows","itemName":"row","itemBody":[]}),
        json!({"kind":"stack","axis":"column","children":[]}),
        json!({"kind":"button","label":"Stable","onClick":"click"}),
        json!({"kind":"toggle","label":"Stable","checked":true}),
        json!({"kind":"checkbox","label":"Stable","checked":false}),
        json!({"kind":"slider","value":2,"min":0,"max":10}),
        json!({"kind":"progress","value":2,"max":10}),
        json!({"kind":"meter","value":2,"min":0,"max":10}),
        json!({"kind":"badge","label":"Stable"}),
        json!({"kind":"divider","axis":"row"}),
        json!({"kind":"spacer","size":8}),
        json!({"kind":"dropzone","label":"Stable","children":[]}),
        json!({"kind":"filePicker","label":"Stable"}),
        json!({"kind":"image","src":"stable.png"}),
        json!({"kind":"link","href":"/stable","children":[]}),
        json!({"kind":"webView","src":"https://example.test"}),
        json!({"kind":"scroll","axis":"column","children":[]}),
        json!({"kind":"list","ordered":true,"children":[]}),
        json!({"kind":"listItem","children":[]}),
        json!({"kind":"slotRotate","phrases":["First","Second"],"intervalMs":1000}),
        json!({"kind":"input","placeholder":"Stable","bind":"name"}),
        json!({"kind":"picker","bind":"choice","options":[]}),
        json!({"kind":"tabs","bind":"tab","tabs":[]}),
    ]
}

fn ir(root: Vec<Value>) -> ViewIr {
    serde_json::from_value(json!({"version":7,"root":root})).unwrap()
}

#[test]
fn every_unchanged_variant_and_complete_tree_produce_no_mutations() {
    for node in nodes() {
        let tree = ir(vec![node]);
        assert!(diff_ir(&tree, &tree.clone()).is_empty(), "{tree:?}");
    }
    let tree = ir(nodes());
    assert!(diff_ir(&tree, &tree.clone()).is_empty());
}

#[test]
fn changed_text_does_not_replace_unchanged_sibling_variants() {
    for node in nodes() {
        let old = ir(vec![json!({"kind":"stack","axis":"column","children":[
            {"kind":"text","content":"Before"},node.clone()
        ]})]);
        let desired = ir(vec![json!({"kind":"stack","axis":"column","children":[
            {"kind":"text","content":"After"},node
        ]})]);
        let mutations = diff_ir(&old, &desired);
        assert_eq!(
            mutations,
            vec![IrMutation::UpdateText {
                path: vec![0, 0],
                content: "After".into(),
            }],
            "unchanged sibling must retain its identity"
        );
        let mut patched = old;
        apply_mutations(&mut patched, &mutations).unwrap();
        assert_eq!(patched, desired);
    }
}

#[test]
fn hot_reload_keeps_unchanged_controls_and_only_patches_changed_text() {
    use crepuscularity_core::TemplateContext;
    use crepuscularity_native::{plan_hot_reload, HotReloadMessage};

    let context = TemplateContext::new();
    for control in ["checkbox label=\"Stable\"", "toggle label=\"Stable\""] {
        let before = format!("div\n  span \"Before\"\n  {control}");
        assert_eq!(
            plan_hot_reload(&before, &before, &context),
            HotReloadMessage::Noop
        );
        let after = before.replace("Before", "After");
        let message = plan_hot_reload(&before, &after, &context);
        let HotReloadMessage::Patch { mutations } = message else {
            panic!("expected text-only patch: {message:?}");
        };
        assert_eq!(mutations.len(), 1, "{mutations:?}");
        assert!(
            matches!(&mutations[0], IrMutation::UpdateText { content, .. } if content == "After")
        );
    }
}
