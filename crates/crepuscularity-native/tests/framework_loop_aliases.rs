use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn loop_names_do_not_capture_props_helpers_or_generated_indices() {
    let mut source = String::new();
    for name in [
        "scope",
        "handlers",
        "toArray",
        "index",
        "Fragment",
        "For",
        "Show",
        "class",
        "_crepusItem0",
    ] {
        source.push_str(&format!(
            "for {name} in rows\n  text \"{{{name}.label}}\"\n  button @click=choose \"Choose\"\n"
        ));
    }
    source.push_str("for scope in rows\n  for handlers in scope.children\n    input bind=handlers.label\n    text \"{scope.label}\"\n    text \"{title}\"");
    let ir = render_template_to_ir(&source, &TemplateContext::new()).unwrap();
    for (name, output, root) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Loops"),
            "scope.",
        ),
        ("solid", emit_solid_component(&ir, "Loops"), "scope()."),
        ("svelte", emit_svelte_component(&ir), "scope."),
        ("vue", emit_vue_component(&ir), "scope."),
    ] {
        assert!(output.contains("_crepusItem0?.label"), "{name}: {output}");
        assert!(
            output.contains("toArray(_crepusItem0?.children)"),
            "{name}: {output}"
        );
        assert!(output.contains("_crepusItem1?.label"), "{name}: {output}");
        assert!(output.contains(&format!("{root}title")), "{name}: {output}");
        assert!(output.contains("_crepusIndex0"), "{name}: {output}");
        assert!(!output.contains("(index, index)"), "{name}: {output}");
    }
}
