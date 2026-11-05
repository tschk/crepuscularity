use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::{
    render_template_to_lvgl_xml, render_template_to_lvgl_xml_with_options, LvglOptions,
};

#[test]
fn admitted_invalid_attribute_names_fail_explicitly() {
    for source in [
        "div 1foo=\"bar\"",
        "div bind:bad!={1}",
        "div bind:.bad={1}",
        "div bind:\u{300}bad={1}",
        "div bind:\u{f0000}bad={1}",
    ] {
        let error = render_template_to_lvgl_xml(source, &TemplateContext::new()).unwrap_err();
        assert!(
            error.to_string().contains("not a valid XML Name"),
            "{source}: {error}"
        );
    }
}

#[test]
fn admitted_valid_unknown_names_preserve_xml_name_boundaries() {
    for name in [
        "_private",
        ":custom",
        "custom:part",
        "a.1",
        "a\u{300}\u{b7}\u{203f}\u{2040}",
        "\u{c0}",
        "\u{d6}",
        "\u{d8}",
        "\u{f6}",
        "\u{f8}",
        "\u{2ff}",
        "\u{370}",
        "\u{37d}",
        "\u{37f}",
        "\u{200c}",
        "\u{200d}",
        "\u{2070}",
        "\u{218f}",
        "\u{2c00}",
        "\u{2fef}",
        "\u{3001}",
        "\u{d7ff}",
        "\u{f900}",
        "\u{fdcf}",
        "\u{fdf0}",
        "\u{fffd}",
        "\u{10000}",
        "\u{effff}",
    ] {
        let source = format!("div bind:{name}={{1}}");
        let xml = render_template_to_lvgl_xml(&source, &TemplateContext::new()).unwrap();
        assert!(xml.contains(&format!(" {name}=\"1\"")), "{source}: {xml}");
    }
    let xml =
        render_template_to_lvgl_xml("custom-widget data-custom=\"ok\"", &TemplateContext::new())
            .unwrap();
    assert!(xml.contains("<lv_obj data_custom=\"ok\""));
}

#[test]
fn export_name_remains_free_text_rather_than_an_xml_identifier() {
    let options = LvglOptions {
        name: "1. free name: 界".into(),
        ..LvglOptions::default()
    };
    let xml =
        render_template_to_lvgl_xml_with_options("div", &TemplateContext::new(), &options).unwrap();
    assert!(xml.contains("name=\"1. free name: 界\""));
}
