use crepuscularity_native::{emit_react_component, emit_solid_component, ViewIr};
use serde_json::json;

fn outputs(name: &str) -> [String; 2] {
    let ir: ViewIr =
        serde_json::from_value(json!({"version":7,"root":[{"kind":"text","content":"Panel"}]}))
            .unwrap();
    [
        emit_react_component(&ir, name),
        emit_solid_component(&ir, name),
    ]
}

#[test]
fn keyword_and_strict_mode_names_use_safe_internal_component_declarations() {
    for name in ["class", "await", "let", "eval", "arguments", "enum"] {
        for output in outputs(name) {
            assert!(
                output.contains("function __CrepusComponent("),
                "{name}: {output}"
            );
            assert!(
                output.contains(&format!("__CrepusComponent as {name}")),
                "{name}: {output}"
            );
            assert!(
                !output.contains(&format!("function {name}(")),
                "{name}: {output}"
            );
            assert!(
                output.contains("export default __CrepusComponent"),
                "{output}"
            );
        }
    }
}

#[test]
fn default_component_name_does_not_create_a_duplicate_default_export() {
    for output in outputs("default") {
        assert!(output.contains("function __CrepusComponent("), "{output}");
        assert_eq!(output.matches("export default").count(), 1, "{output}");
        assert!(!output.contains(" as default"), "{output}");
        assert!(!output.contains("function default("), "{output}");
    }
}

#[test]
fn ordinary_component_names_keep_existing_public_declarations() {
    for output in outputs("Panel") {
        assert!(output.contains("export function Panel("), "{output}");
        assert!(output.contains("export default Panel"), "{output}");
        assert!(!output.contains("function __CrepusComponent("), "{output}");
    }
}
