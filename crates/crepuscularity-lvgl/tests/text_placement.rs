use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn non_text_widgets_keep_their_child_label() {
    for (source_tag, xml_tag) in [
        ("div", "lv_obj"),
        ("section", "lv_obj"),
        ("button", "lv_button"),
        ("progress", "lv_bar"),
        ("slider", "lv_slider"),
        ("canvas", "lv_canvas"),
        ("image", "lv_image"),
        ("switch", "lv_switch"),
    ] {
        let source = format!("{source_tag} #parent\n  \"Visible & text\"");
        let xml = render_template_to_lvgl_xml(&source, &TemplateContext::new()).unwrap();
        let expected = format!("<{xml_tag} id=\"parent\">\n      <lv_label text=\"Visible &amp; text\"/>\n    </{xml_tag}>");
        assert!(xml.contains(&expected), "{source}: {xml}");
    }
}

#[test]
fn text_widgets_keep_direct_text_attributes() {
    for (source_tag, xml_tag) in [
        ("span", "lv_label"),
        ("textarea", "lv_textarea"),
        ("checkbox", "lv_checkbox"),
        ("dropdown", "lv_dropdown"),
    ] {
        let source = format!("{source_tag} #parent\n  \"Visible\"");
        let xml = render_template_to_lvgl_xml(&source, &TemplateContext::new()).unwrap();
        assert!(
            xml.contains(&format!("<{xml_tag} id=\"parent\" text=\"Visible\"/>")),
            "{xml}"
        );
    }
}
