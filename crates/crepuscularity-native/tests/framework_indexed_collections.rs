use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_component, emit_moonshine_dom_component,
    emit_moonshine_ssr_component, emit_react_component, emit_solid_component,
    emit_svelte_component, emit_vue_component, render_template_to_ir,
};

#[test]
fn indexed_collection_paths_use_valid_optional_bracket_access() {
    let ir = render_template_to_ir(
        "for item in rows.0.children\n  text \"{item.name}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for output in [
        emit_react_component(&ir, "Indexed"),
        emit_moonshine_component(&ir, "Indexed"),
        emit_moonshine_dom_component(&ir, "Indexed").unwrap(),
        emit_moonshine_ssr_component(&ir, "Indexed").unwrap(),
        emit_solid_component(&ir, "Indexed"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
        emit_imba_component(&ir).unwrap().bindings,
    ] {
        let output = output.replace("&quot;", "\"");
        assert!(output.contains("?.rows?.[\"0\"]?.children"), "{output}");
        assert!(!output.contains("rows.0"), "{output}");
    }
}
