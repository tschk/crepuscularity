use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn bound_text_and_label_parts_use_the_shared_display_rules() {
    let ir = render_template_to_ir(
        "text \"{value}\"\nbutton \"Value {value}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for output in [
        emit_moonshine_component(&ir, "Values"),
        emit_solid_component(&ir, "Values"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert_eq!(
            output.matches("function __crepusText(").count(),
            1,
            "{output}"
        );
        assert!(output.matches("__crepusText(").count() >= 3, "{output}");
        assert!(!output.contains("String(scope?.value"), "{output}");
    }
}

#[test]
fn static_text_has_no_display_runtime() {
    let ir = render_template_to_ir("text \"Static\"", &TemplateContext::new()).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Static"),
        emit_solid_component(&ir, "Static"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(!output.contains("__crepusText"), "{output}");
    }
}
