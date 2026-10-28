use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn sole_child_labels_keep_their_identity_styles_and_handlers() {
    for parent in ["div", "button", "span"] {
        for child in [
            "span #child \"Label\"",
            "span text-red-500 \"Label\"",
            "span @click=\"select\" \"Label\"",
        ] {
            let source = format!("{parent}\n  {child}");
            let xml = render_template_to_lvgl_xml(&source, &TemplateContext::new()).unwrap();
            let expected = if child.contains("#child") {
                "id=\"child\""
            } else if child.contains("text-red") {
                "text_color=\"#fb2c36\""
            } else {
                "data_on_click=\"select\""
            };
            assert!(xml.contains(expected), "{source}: {xml}");
            assert!(xml.contains("text=\"Label\""), "{xml}");
        }
    }
}

#[test]
fn direct_text_still_uses_the_existing_label_and_button_shapes() {
    let context = TemplateContext::new();
    let label = render_template_to_lvgl_xml("span text-red-500 \"Label\"", &context).unwrap();
    assert!(
        label.contains("<lv_label text_color=\"#fb2c36\" text=\"Label\"/>"),
        "{label}"
    );
    let button = render_template_to_lvgl_xml("button\n  \"Label\"", &context).unwrap();
    assert!(
        button.contains("<lv_button>\n      <lv_label text=\"Label\"/>\n    </lv_button>"),
        "{button}"
    );
}
