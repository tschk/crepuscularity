use crepuscularity_native::{emit_svelte_component, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    emit_svelte_component(&ir)
}

#[test]
fn rotating_text_uses_an_owned_action_and_preserves_literal_phrase_data() {
    let phrases = ["First|literal", "</script>{next}\"\\"];
    let output = emit(json!([{
        "kind":"slotRotate", "phrases":phrases, "intervalMs":0,
        "style":{"id":"rotation","classes":["rounded","px-2"]}
    }]));
    assert!(output.contains("use:__crepusRotate"), "{output}");
    let encoded = serde_json::to_string(&phrases)
        .unwrap()
        .replace('<', "\\u003c");
    assert!(output.contains(&encoded), "{output}");
    for retained in [
        "id=\"rotation\"",
        "class=\"rounded px-2\"",
        "First|literal",
        "setInterval",
        "clearInterval",
        "2147483647",
        "prefers-reduced-motion: reduce",
        "destroy",
    ] {
        assert!(output.contains(retained), "missing {retained}: {output}");
    }
    assert!(output.contains("Math.max(1,"), "{output}");
    assert!(output.contains("addEventListener(\"change\","), "{output}");
    assert!(
        output.contains("removeEventListener(\"change\","),
        "{output}"
    );
    assert_eq!(
        output.matches("</script>").count(),
        1,
        "unsafe script literal: {output}"
    );
}

#[test]
fn zero_and_one_phrase_groups_remain_legal_with_stable_initial_text() {
    for phrases in [Vec::<&str>::new(), vec!["Only {literal}"]] {
        let output = emit(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":500}]));
        assert!(output.contains("<span"), "{output}");
        assert!(output.contains("use:__crepusRotate"), "{output}");
        if phrases.is_empty() {
            assert!(!output.contains("undefined</span>"), "{output}");
        } else {
            assert!(output.contains("Only &#123;literal&#125;"), "{output}");
        }
    }
}

#[test]
fn ordinary_components_do_not_include_rotation_actions_or_timers() {
    let output = emit(json!([{"kind":"text","content":"Ordinary"}]));
    for excluded in [
        "__crepusRotate",
        "setInterval",
        "clearInterval",
        "prefers-reduced-motion",
    ] {
        assert!(
            !output.contains(excluded),
            "unexpected {excluded}: {output}"
        );
    }
}
