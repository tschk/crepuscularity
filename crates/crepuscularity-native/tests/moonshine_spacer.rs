use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_dom_component, emit_moonshine_ssr_component, render_template_to_ir_with_path,
    ViewIr,
};
use serde_json::json;

fn outputs(ir: &ViewIr) -> [String; 2] {
    [
        emit_moonshine_dom_component(ir, "Spacing").unwrap(),
        emit_moonshine_ssr_component(ir, "Spacing").unwrap(),
    ]
}

#[test]
fn spacers_preserve_identity_and_classes_and_are_hidden_from_accessibility() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"spacer", "size":12,
        "style":{"id":"space","classes":["grow","mx-2"]}
    }]}))
    .unwrap();
    for output in outputs(&ir) {
        for retained in [
            "h(\"div\"",
            "\"aria-hidden\": \"true\"",
            "id: \"space\"",
            "class: \"grow mx-2\"",
        ] {
            assert!(output.contains(retained), "missing {retained}: {output}");
        }
    }
}

#[test]
fn old_spacer_ir_does_not_invent_optional_identity_attributes() {
    let ir: ViewIr =
        serde_json::from_value(json!({"version":7,"root":[{"kind":"spacer"}]})).unwrap();
    for output in outputs(&ir) {
        assert!(output.contains("\"aria-hidden\": \"true\""), "{output}");
        assert!(!output.contains("id:"), "{output}");
        assert!(!output.contains("class:"), "{output}");
    }
}

#[test]
fn source_spacers_lower_and_emit_in_both_direct_hosts() {
    let ir = render_template_to_ir_with_path(
        "spacer size=12 class=grow id=space",
        &TemplateContext::new(),
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&ir).unwrap()["root"][0]["kind"],
        "spacer"
    );
    for output in outputs(&ir) {
        assert!(output.contains("\"aria-hidden\": \"true\""), "{output}");
        assert!(output.contains("id: \"space\""), "{output}");
        assert!(output.contains("class: \"grow\""), "{output}");
    }
}
