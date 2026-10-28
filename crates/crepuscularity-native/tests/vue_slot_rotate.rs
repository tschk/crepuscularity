use crepuscularity_native::{emit_vue_component, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    emit_vue_component(&ir)
}

#[test]
fn vue_rotation_preserves_phrase_data_and_uses_per_node_directive_cleanup() {
    let phrases = ["First|literal", "Quote\" </script> {braces}\\"];
    let output = emit(
        json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":0,"style":{"id":"rotation","classes":["rounded","px-2"]}}]),
    );
    let decoded = output
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let encoded = serde_json::to_string(&phrases).unwrap();
    assert!(
        decoded.contains(&encoded) || decoded.contains(&encoded.replace('<', "\\u003c")),
        "{output}"
    );
    for retained in [
        "v-crepus-rotate",
        "id=\"rotation\"",
        "class=\"rounded px-2\"",
        "First|literal",
        "vCrepusRotate",
        "WeakMap",
        "mounted",
        "unmounted",
        "destroy",
    ] {
        assert!(output.contains(retained), "missing {retained}: {output}");
    }
    assert_eq!(
        output.matches("</script>").count(),
        1,
        "unsafe script literal: {output}"
    );
}

#[test]
fn zero_and_one_phrase_rotations_are_valid_and_share_the_short_group_guard() {
    for phrases in [Vec::<&str>::new(), vec!["Only {literal}"]] {
        let output = emit(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":500}]));
        assert!(output.contains("v-crepus-rotate"), "{output}");
        assert!(
            output.contains("phrases.length < 2") || output.contains("phrases.length <= 1"),
            "missing short-group guard: {output}"
        );
        assert!(!output.contains("undefined</span>"), "{output}");
        if !phrases.is_empty() {
            assert!(output.contains("Only"), "{output}");
        }
    }
}

#[test]
fn ordinary_vue_output_has_no_rotation_directive_or_timer_runtime() {
    let output = emit(json!([{"kind":"text","content":"Ordinary"}]));
    for excluded in [
        "v-crepus-rotate",
        "vCrepusRotate",
        "__crepusRotate",
        "setInterval",
        "prefers-reduced-motion",
    ] {
        assert!(
            !output.contains(excluded),
            "unexpected {excluded}: {output}"
        );
    }
}
