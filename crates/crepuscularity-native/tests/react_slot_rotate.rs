use crepuscularity_native::{emit_moonshine_app, emit_react_component, ViewIr};
use serde_json::{json, Value};

fn outputs(root: Value) -> [String; 2] {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    [
        emit_react_component(&ir, "Rotating"),
        emit_moonshine_app(&ir),
    ]
}

#[test]
fn react_and_moonshine_rotation_keep_safe_phrases_and_owned_hook_lifecycle() {
    let phrases = ["First|literal", "Quote\" {braces} </script>\\"];
    for output in outputs(
        json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":500,"style":{"id":"rotation","classes":["rounded","px-2"]}}]),
    ) {
        let encoded = serde_json::to_string(&phrases).unwrap();
        assert!(
            output.contains(&encoded) || output.contains(&encoded.replace('<', "\\u003c")),
            "{output}"
        );
        for retained in [
            "__CrepusRotatingText",
            "useEffect",
            "useRef",
            "id=\"rotation\"",
            "rounded px-2",
            "First|literal",
            "destroy",
        ] {
            assert!(output.contains(retained), "missing {retained}: {output}");
        }
        let declaration = output
            .find("function __CrepusRotatingText")
            .expect("module helper");
        let component = output
            .find("export function Rotating")
            .or_else(|| output.find("export function App"))
            .expect("public component");
        assert!(
            declaration < component,
            "helper must be module-level: {output}"
        );
    }
}

#[test]
fn short_react_rotation_groups_keep_safe_initial_text_and_runtime_guard() {
    for phrases in [Vec::<&str>::new(), vec!["Only {literal}"]] {
        for output in outputs(json!([{"kind":"slotRotate","phrases":phrases,"intervalMs":0}])) {
            assert!(output.contains("__CrepusRotatingText"), "{output}");
            assert!(
                output.contains("phrases.length < 2") || output.contains("phrases.length <= 1"),
                "{output}"
            );
            assert!(!output.contains("undefined</span>"), "{output}");
        }
    }
}

#[test]
fn ordinary_react_and_moonshine_output_omit_rotation_hooks() {
    for output in outputs(json!([{"kind":"text","content":"Ordinary"}])) {
        for excluded in [
            "__CrepusRotatingText",
            "useEffect",
            "useRef",
            "setInterval",
            "prefers-reduced-motion",
        ] {
            assert!(
                !output.contains(excluded),
                "unexpected {excluded}: {output}"
            );
        }
    }
}
