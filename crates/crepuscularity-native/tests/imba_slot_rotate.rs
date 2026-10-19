use crepuscularity_native::{emit_imba_component, ImbaComponent, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> ImbaComponent {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    emit_imba_component(&ir).unwrap()
}

#[test]
fn imba_rotation_uses_a_span_subclass_with_owned_cleanup_and_exact_phrase_data() {
    let phrases = ["First|literal", "Quote\" {braces} </script>\\"];
    let output = emit(
        json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":500,"style":{"id":"rotation","classes":["rounded","px-2"]}}]),
    );
    for retained in [
        "tag CrepusRotatingText < span",
        "def mount",
        "def unmount",
        "self.cleanup",
        "phrases=(bindings.",
        "id=(\"rotation\")",
        "className=(\"rounded px-2\")",
    ] {
        assert!(
            output.source.contains(retained),
            "missing {retained}: {}",
            output.source
        );
    }
    let encoded = serde_json::to_string(&phrases).unwrap();
    assert!(
        output.bindings.contains(&encoded)
            || output.bindings.contains(&encoded.replace('<', "\\u003c")),
        "{}",
        output.bindings
    );
    assert!(
        output.bindings.contains("__crepusRotate"),
        "{}",
        output.bindings
    );
    assert!(output.bindings.contains("destroy"), "{}", output.bindings);
}

#[test]
fn empty_and_single_imba_groups_keep_the_shared_short_group_guard() {
    for phrases in [Vec::<&str>::new(), vec!["Only {literal}"]] {
        let output = emit(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":0}]));
        assert!(
            output.source.contains("<CrepusRotatingText"),
            "{}",
            output.source
        );
        assert!(
            output.bindings.contains("phrases.length < 2")
                || output.bindings.contains("phrases.length <= 1"),
            "{}",
            output.bindings
        );
        assert!(!output.source.contains("undefined"), "{}", output.source);
    }
}

#[test]
fn unrelated_imba_components_do_not_include_rotation_tags_or_runtime() {
    let output = emit(json!([{"kind":"text","content":"Ordinary"}]));
    assert!(!output.source.contains("CrepusRotatingText"));
    for excluded in ["__crepusRotate", "setInterval", "prefers-reduced-motion"] {
        assert!(
            !output.bindings.contains(excluded),
            "unexpected {excluded}: {}",
            output.bindings
        );
    }
}
