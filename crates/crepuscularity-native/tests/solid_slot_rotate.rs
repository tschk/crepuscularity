use crepuscularity_native::{emit_solid_component, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    emit_solid_component(&ir, "Rotating")
}

#[test]
fn solid_rotation_preserves_phrases_identity_and_owned_mount_cleanup() {
    let phrases = ["First|literal", "Quote\" {braces} </script>\\"];
    let output = emit(
        json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":0,"style":{"id":"rotation","classes":["rounded","px-2"]}}]),
    );
    let encoded = serde_json::to_string(&phrases).unwrap();
    assert!(
        output.contains(&encoded) || output.contains(&encoded.replace('<', "\\u003c")),
        "{output}"
    );
    for retained in [
        "id=\"rotation\"",
        "rounded px-2",
        "First|literal",
        "onMount as __crepusOnMount",
        "onCleanup as __crepusOnCleanup",
        "ref=",
        "__crepusOnMount(",
        "__crepusOnCleanup(",
        "destroy",
    ] {
        assert!(output.contains(retained), "missing {retained}: {output}");
    }
}

#[test]
fn empty_and_single_solid_groups_keep_safe_initial_output_and_short_group_guard() {
    for phrases in [Vec::<&str>::new(), vec!["Only {literal}"]] {
        let output = emit(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":500}]));
        assert!(output.contains("<span"), "{output}");
        assert!(
            output.contains("phrases.length < 2") || output.contains("phrases.length <= 1"),
            "{output}"
        );
        assert!(!output.contains("undefined</span>"), "{output}");
        if !phrases.is_empty() {
            assert!(output.contains("Only"), "{output}");
        }
    }
}

#[test]
fn unrelated_solid_output_omits_rotation_helpers_and_lifecycle_imports() {
    let output = emit(json!([{"kind":"text","content":"Ordinary"}]));
    for excluded in [
        "__crepusRotate",
        "__crepusOnMount",
        "__crepusOnCleanup",
        "setInterval",
        "prefers-reduced-motion",
    ] {
        assert!(
            !output.contains(excluded),
            "unexpected {excluded}: {output}"
        );
    }
}
