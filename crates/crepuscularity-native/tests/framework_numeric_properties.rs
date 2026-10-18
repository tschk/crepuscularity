use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_component, emit_moonshine_dom_component,
    emit_moonshine_ssr_component, emit_react_component, emit_solid_component,
    emit_svelte_component, emit_vue_component, render_template_to_ir,
};

#[test]
fn numeric_properties_reach_every_framework_helper() {
    let mut context = TemplateContext::new();
    context.set("amount", -2.5);
    let ir = render_template_to_ir(
        "text \"{amount.abs} {amount.floor} {amount.ceil} {amount.round.abs}\"",
        &context,
    )
    .unwrap();
    let imba = emit_imba_component(&ir).unwrap();
    for output in [
        emit_react_component(&ir, "Numeric"),
        emit_moonshine_component(&ir, "Numeric"),
        emit_moonshine_dom_component(&ir, "Numeric").unwrap(),
        emit_moonshine_ssr_component(&ir, "Numeric").unwrap(),
        emit_solid_component(&ir, "Numeric"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
        imba.bindings,
    ] {
        assert_eq!(
            output.matches("function __crepusProperty(").count(),
            1,
            "{output}"
        );
        for property in ["abs", "floor", "ceil", "round"] {
            assert!(
                output.contains(&format!(", \"{property}\")"))
                    || output.contains(&format!(", &quot;{property}&quot;)")),
                "{property}: {output}"
            );
        }
        assert!(
            output.contains("numeric property result out of integer range"),
            "{output}"
        );
    }
}

#[test]
fn error_component_names_do_not_capture_numeric_error_constructors() {
    let mut context = TemplateContext::new();
    context.set("amount", -2.5);
    let ir = render_template_to_ir("text \"{amount.round}\"", &context).unwrap();
    for output in [
        emit_react_component(&ir, "Error"),
        emit_solid_component(&ir, "Error"),
    ] {
        assert!(output.contains("new globalThis.Error("), "{output}");
        assert!(!output.contains("new Error("), "{output}");
    }
}
