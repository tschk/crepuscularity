use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, render_template_to_ir,
};

#[test]
fn public_component_names_do_not_capture_runtime_bindings() {
    let ir = render_template_to_ir(
        "for row in rows\n  text \"{row.value}\"\ntext \"{name.upper.length}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for name in [
        "Array",
        "Object",
        "globalThis",
        "undefined",
        "toArray",
        "Fragment",
        "For",
        "Show",
    ] {
        for (output, collides) in [
            (
                emit_moonshine_component(&ir, name),
                !matches!(name, "For" | "Show"),
            ),
            (emit_solid_component(&ir, name), name != "Fragment"),
        ] {
            if collides {
                assert!(
                    output.contains("function __CrepusComponent("),
                    "{name}: {output}"
                );
                assert!(
                    output.contains(&format!("export {{ __CrepusComponent as {name} }};")),
                    "{name}: {output}"
                );
                assert!(
                    output.contains("export default __CrepusComponent;"),
                    "{name}: {output}"
                );
                assert!(
                    !output.contains("export function __CrepusComponent("),
                    "{name}: {output}"
                );
            } else {
                assert!(
                    output.contains(&format!("export function {name}(")),
                    "{name}: {output}"
                );
            }
            assert!(
                output.contains(&format!("export type {name}Props")),
                "{name}: {output}"
            );
        }
    }
}

#[test]
fn ordinary_component_declarations_remain_named() {
    let ir = render_template_to_ir("text \"Hello\"", &TemplateContext::new()).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Panel"),
        emit_solid_component(&ir, "Panel"),
    ] {
        assert!(output.contains("export function Panel("));
        assert!(output.contains("export default Panel;"));
        assert!(!output.contains("__CrepusComponent"));
    }
}
