use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn checked_bindings_reach_all_framework_emitters_and_keep_loop_locals() {
    let ir = render_template_to_ir(
        "toggle bind=enabled checked=true\ncheckbox bind=enabled checked=true\nfor row in rows\n  checkbox bind=row.selected\nfor toChecked in rows\n  checkbox bind=toChecked.selected\nfor value in rows\n  checkbox bind=value.selected",
        &TemplateContext::new(),
    )
    .unwrap();
    for (name, output, scope) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Controls"),
            "scope?.",
        ),
        ("solid", emit_solid_component(&ir, "Controls"), "scope()?."),
        ("svelte", emit_svelte_component(&ir), "scope?."),
        ("vue", emit_vue_component(&ir), "scope?."),
    ] {
        let bound = format!(
            "((value = true) => typeof value === \"boolean\" ? value : true)({scope}enabled)"
        );
        let output = output.replace("&quot;", "\"").replace("&gt;", ">");
        assert_eq!(output.matches(&bound).count(), 2, "{name}: {output}");
        assert!(
            output.contains(
                "((value = false) => typeof value === \"boolean\" ? value : false)(_crepusItem0?.selected)"
            ),
            "{name}: {output}"
        );
        assert!(!output.contains("scope.row.selected"), "{name}: {output}");
        assert_eq!(
            output
                .matches("? value : false)(_crepusItem0?.selected)")
                .count(),
            3,
            "{name}: {output}"
        );
    }
}

#[test]
fn react_checkboxes_only_become_controlled_when_bound() {
    let ir = render_template_to_ir("checkbox checked=true", &TemplateContext::new()).unwrap();
    let output = emit_moonshine_component(&ir, "Unbound");
    assert!(output.contains("defaultChecked={true}"), "{output}");
    let ir = render_template_to_ir(
        "checkbox bind=enabled checked=true",
        &TemplateContext::new(),
    )
    .unwrap();
    let output = emit_moonshine_component(&ir, "Bound");
    assert!(
        output.contains("checked={((value = true) => typeof value === \"boolean\" ? value : true)(scope?.enabled)}"),
        "{output}"
    );
    assert!(!output.contains("defaultChecked="), "{output}");
}

#[test]
fn checked_paths_guard_each_parent_and_preserve_numeric_keys() {
    let ir = render_template_to_ir(
        "toggle bind=prefs.enabled checked=true\ncheckbox bind=rows.0.enabled\ncheckbox bind=rows.01.enabled\nfor row in rows\n  checkbox bind=row.enabled",
        &TemplateContext::new(),
    ).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Controls"),
        emit_solid_component(&ir, "Controls"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        let output = output.replace("&quot;", "\"").replace("&gt;", ">");
        assert!(output.contains("?.prefs?.enabled"), "{output}");
        assert!(output.contains("?.rows?.[\"0\"]?.enabled"), "{output}");
        assert!(output.contains("?.rows?.[\"01\"]?.enabled"), "{output}");
        assert!(output.contains("(_crepusItem0?.enabled)"), "{output}");
    }
}
