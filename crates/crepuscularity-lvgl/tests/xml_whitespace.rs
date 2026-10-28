use crepuscularity_core::TemplateContext;

#[test]
fn attribute_whitespace_uses_character_references() {
    let mut ctx = TemplateContext::new();
    ctx.set("value", "A\nB\tC\rD");
    let xml =
        crepuscularity_lvgl::render_template_to_lvgl_xml("input value={value}", &ctx).unwrap();
    assert!(xml.contains("A&#10;B&#9;C&#13;D"), "{xml}");
}
