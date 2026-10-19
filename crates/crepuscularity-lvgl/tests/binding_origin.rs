use crepuscularity_core::{parse_template_with_path, TemplateContext};
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, LvglOptions};
use std::path::Path;

#[test]
fn angular_attributes_omit_null_and_preserve_literal_and_presence_values() {
    let nodes = parse_template_with_path("<div [attr.title]=\"label\" [attr.HIDDEN]=\"false\" [attr.CHECKED]=\"false\" [attr.viewBox]=\"box\" [attr.disabled]=\"null\" [attr.CLASS]=\"classes\"></div>", Some(Path::new("view.ng"))).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("label", "{missing}");
    ctx.set("classes", "w-10 {missing}");
    ctx.set("box", "0 0 10 10");
    let xml = render_nodes_to_lvgl_xml(&nodes, &ctx, &LvglOptions::default()).unwrap();
    assert!(xml.contains("title=\"{missing}\""), "{xml}");
    assert!(xml.contains("hidden=\"true\""), "{xml}");
    assert!(xml.contains("checked=\"true\""), "{xml}");
    assert!(xml.contains("viewBox=\"0 0 10 10\""), "{xml}");
    assert!(!xml.contains("viewbox="), "{xml}");
    assert!(!xml.contains("disabled="), "{xml}");
    assert!(!xml.contains("class="), "{xml}");
    assert!(xml.contains("width=\"40\""), "{xml}");
}

#[test]
fn ordinary_boolean_binding_retains_value_and_null_attribute_class_is_absent() {
    let nodes = parse_template_with_path(
        "<div [hidden]=\"false\" [attr.CLASS]=\"null\"></div>",
        Some(Path::new("view.ng")),
    )
    .unwrap();
    let xml =
        render_nodes_to_lvgl_xml(&nodes, &TemplateContext::new(), &LvglOptions::default()).unwrap();
    assert!(xml.contains("hidden=\"false\""), "{xml}");
    assert!(!xml.contains("class="), "{xml}");
}

#[test]
fn malformed_attribute_expression_propagates() {
    use crepuscularity_core::ast::{Binding, BindingOrigin, Node};
    let mut nodes = parse_template_with_path("<div></div>", Some(Path::new("view.ng"))).unwrap();
    let Node::Element(el) = &mut nodes[0] else {
        panic!("element expected")
    };
    el.bindings.push(Binding {
        origin: BindingOrigin::Attribute,
        prop: "title".into(),
        value: "true foo".into(),
    });
    assert!(crepuscularity_core::eval::eval_expr("true foo", &TemplateContext::new()).is_err());
    assert!(
        render_nodes_to_lvgl_xml(&nodes, &TemplateContext::new(), &LvglOptions::default()).is_err()
    );
}

#[test]
fn known_html_attribute_names_map_case_insensitively_without_changing_custom_xml_names() {
    let nodes = parse_template_with_path(
        r#"<input [attr.PLACEHOLDER]="'Hint'" [attr.VALUE]="'typed'" [attr.SRC]="'image.png'" [attr.viewBox]="'0 0 10 10'"/>"#,
        Some(Path::new("Entry.ng")),
    ).unwrap();
    let xml =
        render_nodes_to_lvgl_xml(&nodes, &TemplateContext::new(), &LvglOptions::default()).unwrap();
    for expected in [
        "placeholder_text=\"Hint\"",
        "value=\"typed\"",
        "src=\"image.png\"",
        "viewBox=\"0 0 10 10\"",
    ] {
        assert!(xml.contains(expected), "{xml}");
    }
    for unexpected in ["PLACEHOLDER=", "VALUE=", "SRC=", "viewbox="] {
        assert!(!xml.contains(unexpected), "{xml}");
    }
}
