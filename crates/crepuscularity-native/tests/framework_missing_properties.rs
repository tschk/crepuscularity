use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn generated_property_reads_guard_missing_intermediate_values() {
    let ir = render_template_to_ir(
        "text \"{user.profile.name}\"\nif user.profile.enabled\n  text \"Visible\"\nfor row in user.profile.rows\n  text \"{row.details.name}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for output in [
        emit_moonshine_component(&ir, "Missing"),
        emit_solid_component(&ir, "Missing"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        for path in [
            "user?.profile?.name",
            "user?.profile?.enabled",
            "user?.profile?.rows",
            "_crepusItem0?.details?.name",
        ] {
            assert!(output.contains(path), "missing {path}: {output}");
        }
    }
}
