use crepuscularity_core::{parse_template_with_path, TemplateContext, TemplateValue};
use crepuscularity_lvgl::render_nodes_to_lvgl_xml;
use std::path::Path;

#[test]
fn angular_attribute_values_are_literal_and_do_not_activate_controls() {
    let nodes = parse_template_with_path("<switch [attr.checked]='state'/><button [attr.disabled]='state'/><div [attr.hidden]='state'/><slider [attr.bind]='state' [attr.step]='state'/><input [attr.value]='state'/>", Some(Path::new("a.ng"))).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("state", false);
    let xml = render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
    assert!(xml.contains("checked=\"true\""), "{xml}");
    assert!(xml.contains("disabled=\"true\""), "{xml}");
    assert!(xml.contains("hidden=\"true\""), "{xml}");
    assert!(xml.contains("bind=\"false\" step=\"false\""), "{xml}");
    assert!(xml.contains("value=\"false\""), "{xml}");
    ctx.set("state", TemplateValue::Null);
    let xml = render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
    for attr in [
        "checked=",
        "disabled=",
        "hidden=",
        "bind=",
        "step=",
        "value=",
    ] {
        assert!(!xml.contains(attr), "{xml}");
    }
}

#[test]
fn angular_attribute_classes_do_not_interpolate_returned_text() {
    let nodes = parse_template_with_path(
        "<div [attr.id]='name' [attr.class]='classes'/>",
        Some(Path::new("a.ng")),
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("name", "{missing}");
    ctx.set("classes", "bg-[#ff0000]");
    let xml = render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
    assert!(xml.contains("id=\"{missing}\""), "{xml}");
    assert!(xml.contains("bg_color="), "{xml}");
}
