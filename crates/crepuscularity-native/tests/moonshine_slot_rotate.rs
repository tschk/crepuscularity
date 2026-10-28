use crepuscularity_native::{emit_moonshine_dom_component, emit_moonshine_ssr_component, ViewIr};
use serde_json::{json, Value};

fn ir(root: Value) -> ViewIr {
    serde_json::from_value(json!({"version":7,"root":root})).unwrap()
}

#[test]
fn browser_slot_rotation_preserves_phrases_and_imports_the_shared_runtime_once() {
    let phrases = ["First|literal", "Quote\" slash\\ {braces}"];
    let tree = ir(json!([
        {"kind":"slotRotate","phrases":phrases,"intervalMs":0,"style":{"id":"rotating","classes":["rounded","px-2"]}},
        {"kind":"slotRotate","phrases":["Another","Next"],"intervalMs":4000}
    ]));
    let output = emit_moonshine_dom_component(&tree, "Rotating").unwrap();
    assert!(
        output.contains(&serde_json::to_string(&phrases).unwrap()),
        "{output}"
    );
    assert!(output.contains("id: \"rotating\""), "{output}");
    assert!(output.contains("class: \"rounded px-2\""), "{output}");
    assert!(
        output.contains("createRotatingText as __crepusSlotRotate"),
        "{output}"
    );
    assert_eq!(
        output.matches("@tschk/moonshine-dom/rotating-text").count(),
        1,
        "{output}"
    );
    for excluded in ["setInterval", "matchMedia", "onCleanup", "getOwner"] {
        assert!(
            !output.contains(excluded),
            "unexpected inline runtime {excluded}: {output}"
        );
    }
}

#[test]
fn server_slot_rotation_returns_initial_text_without_browser_runtime_imports() {
    for phrases in [
        Vec::<&str>::new(),
        vec!["Only {literal}"],
        vec!["First|literal", "Second"],
    ] {
        let tree = ir(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":4000}]));
        let output = emit_moonshine_ssr_component(&tree, "Rotating").unwrap();
        assert!(
            output
                .contains(&serde_json::to_string(phrases.first().copied().unwrap_or("")).unwrap()),
            "{output}"
        );
        for excluded in [
            "rotating-text",
            "setInterval",
            "clearInterval",
            "createSignal",
            "onCleanup",
            "getOwner",
        ] {
            assert!(
                !output.contains(excluded),
                "unexpected {excluded}: {output}"
            );
        }
        emit_moonshine_dom_component(&tree, "Rotating").unwrap();
    }
}

#[test]
fn unrelated_ir_does_not_import_slot_rotation_runtime() {
    let tree = ir(json!([{"kind":"text","content":"Ordinary"}]));
    for output in [
        emit_moonshine_dom_component(&tree, "Plain").unwrap(),
        emit_moonshine_ssr_component(&tree, "Plain").unwrap(),
    ] {
        assert!(!output.contains("rotating-text"), "{output}");
        assert!(!output.contains("__crepusSlotRotate"), "{output}");
    }
}
