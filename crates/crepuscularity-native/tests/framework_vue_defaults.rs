use crepuscularity_core::TemplateContext;
use crepuscularity_native::{emit_vue_component, render_template_to_ir};

#[test]
fn optional_vue_props_have_independent_empty_defaults() {
    let ir=render_template_to_ir("text \"{title}\"\nif enabled\n  text \"Visible\"\nfor row in rows\n  text \"{row.label}\"\nbutton @click=choose \"Choose\"",&TemplateContext::new()).unwrap();
    let output = emit_vue_component(&ir);
    assert!(output.contains("withDefaults(defineProps<"), "{output}");
    assert!(output.contains("scope: () => ({})"), "{output}");
    assert!(output.contains("handlers: () => ({})"), "{output}");
}
