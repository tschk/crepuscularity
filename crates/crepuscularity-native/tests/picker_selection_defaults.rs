use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_template_to_ir, render_template_to_ir_with_path};
use std::path::Path;

#[test]
fn picker_parent_selection_defaults_fail_with_binding_guidance() {
    for tag in ["select", "picker"] {
        for attribute in ["value", "defaultValue", "default-value"] {
            for value in ["\"b\"", "\"\"", "{null}", "{1 +}"] {
                for binding in ["", " bind=choice"] {
                    let source =
                        format!("{tag}{binding} {attribute}={value}\n  option value=b \"B\"");
                    let error = render_template_to_ir(&source, &TemplateContext::new())
                        .unwrap_err()
                        .to_string();
                    assert!(
                        error.contains(attribute) && error.contains("bind the picker value"),
                        "{source}: {error}"
                    );
                }
            }
        }
    }
}

#[test]
fn markup_selection_defaults_are_rejected_without_changing_bound_pickers() {
    for attribute in ["value", "defaultValue", "default-value"] {
        let source = format!("<select {attribute}=\"b\"><option value=\"b\">B</option></select>");
        let error = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("Entry.jsx")),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains(attribute) && error.contains("bind the picker value"),
            "{error}"
        );
    }
    for tag in ["select", "picker"] {
        let ir = render_template_to_ir(&format!("{tag} bind=choice name=field required\n  option value=a \"A\"\n  option value=b \"B\""), &TemplateContext::new()).unwrap();
        let json = serde_json::to_value(ir).unwrap();
        assert_eq!(json["root"][0]["bind"], "choice");
        assert_eq!(json["root"][0]["options"][1]["value"], "b");
        assert_eq!(json["root"][0]["style"]["formName"], "field");
        assert_eq!(json["root"][0]["style"]["required"], true);
        assert!(render_template_to_ir(
            &format!("{tag}\n  option value=b selected \"B\""),
            &TemplateContext::new()
        )
        .is_err());
    }
}
