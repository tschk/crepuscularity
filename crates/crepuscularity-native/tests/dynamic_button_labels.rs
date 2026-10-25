use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir, LabelPart, NativeCodegenTarget, ViewIr,
    ViewNode,
};

#[test]
fn lowering_preserves_mixed_label_parts_and_old_json() {
    let ir = render_template_to_ir(
        "for row in rows\n  button @click=choose \"Open {row.label}\"\nbutton \"Static\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let json = serde_json::to_value(&ir).unwrap();
    assert_eq!(json["root"][0]["itemBody"][0]["label"], "Open ");
    assert_eq!(
        json["root"][0]["itemBody"][0]["labelParts"],
        serde_json::json!([
            {"kind":"literal","value":"Open "}, {"kind":"expr","value":"row.label"}
        ])
    );
    assert!(json["root"][1].get("labelParts").is_none());
    let old: ViewIr = serde_json::from_value(serde_json::json!({"version":ir.version,"root":[{"kind":"button","label":"Literal {name}"}]})).unwrap();
    assert!(serde_json::to_value(old).unwrap()["root"][0]
        .get("labelParts")
        .is_none());
}

#[test]
fn all_framework_emitters_resolve_label_parts_in_loop_scope() {
    let mut ir = render_template_to_ir(
        "for row in rows\n  button @click=choose \"Open {row.label}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    if let ViewNode::ForEach { item_body, .. } = &mut ir.root[0] {
        if let ViewNode::Button { label_parts, .. } = &mut item_body[0] {
            label_parts.push(LabelPart::Literal("{literal}".into()));
        }
    }
    for output in [
        emit_moonshine_component(&ir, "Labels"),
        emit_solid_component(&ir, "Labels"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        let output = output.replace("&quot;", "\"");
        assert!(
            output.contains("__crepusText(_crepusItem0?.label)"),
            "{output}"
        );
        assert!(output.contains(r"\u007bliteral\u007d"), "{output}");
    }
}

#[test]
fn native_generators_read_dynamic_label_expressions() {
    let ir = render_template_to_ir(
        "for row in rows\n  button @click=choose \"Open {row.label}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let output = generate_native_source(&ir, target, "Labels");
        assert!(output.contains("\"Open \" + "), "{output}");
        assert!(output.contains(".text(\"row.label\""), "{output}");
    }
}

#[test]
fn label_metadata_changes_trigger_node_replacement() {
    let old = render_template_to_ir("button \"{first}\"", &TemplateContext::new()).unwrap();
    let new = render_template_to_ir("button \"{second}\"", &TemplateContext::new()).unwrap();
    let mut patched = old.clone();
    let changes = crepuscularity_native::mutations::diff_ir(&old, &new);
    assert_eq!(changes.len(), 1);
    crepuscularity_native::mutations::apply_mutations(&mut patched, &changes).unwrap();
    assert_eq!(patched, new);
}

#[test]
fn native_labels_keep_snapshots_for_expressions_the_bridge_cannot_resolve() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 4);
    for expression in ["1 + 2", "count + 1", "count.upper + '!'", "--2"] {
        let source = format!("button \"Value {{{expression}}}\"");
        let ir = render_template_to_ir(&source, &ctx).unwrap();
        let mut snapshot = ir.clone();
        if let ViewNode::Button { label_parts, .. } = &mut snapshot.root[0] {
            assert!(!label_parts.is_empty());
            label_parts.clear();
        }
        for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
            assert_eq!(
                generate_native_source(&ir, target, "Labels"),
                generate_native_source(&snapshot, target, "Labels"),
                "{target:?}: {expression}"
            );
        }
    }
}
