use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn all_emitters_resolve_computed_properties_in_text_labels_and_conditions() {
    let ir = render_template_to_ir(
        "text \"{name.upper.length}\"\nbutton \"Count {items.len}\"\nif name.trim.is_empty\n  text \"Empty\"",
        &TemplateContext::new(),
    ).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Properties"),
        emit_solid_component(&ir, "Properties"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert_eq!(
            output.matches("function __crepusProperty(").count(),
            1,
            "{output}"
        );
        for property in ["upper", "length", "len", "trim", "is_empty"] {
            assert!(
                output.contains(&format!(", \"{property}\")"))
                    || output.contains(&format!(", &quot;{property}&quot;)")),
                "{property}: {output}"
            );
        }
    }
}

#[test]
fn static_components_do_not_emit_property_runtime() {
    let ir = render_template_to_ir("text \"Static\"", &TemplateContext::new()).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Static"),
        emit_solid_component(&ir, "Static"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(!output.contains("__crepusProperty"), "{output}");
    }
}
