use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::{
    render_template_to_lvgl_xml, render_template_to_lvgl_xml_with_options, LvglOptions,
};

#[test]
fn forbidden_xml_characters_fail_in_dynamic_text_and_attributes() {
    for ch in ['\0', '\u{1}', '\u{b}', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
        let mut ctx = TemplateContext::new();
        ctx.set("value", format!("A{ch}B"));
        for source in ["span \"{value}\"", "input value={value}", "div id={value}"] {
            let error = render_template_to_lvgl_xml(source, &ctx).unwrap_err();
            assert!(
                error.to_string().contains(&format!("U+{:04X}", ch as u32)),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn export_name_is_validated_too() {
    let options = LvglOptions {
        name: "bad\0name".into(),
        ..LvglOptions::default()
    };
    let error =
        render_template_to_lvgl_xml_with_options("span \"OK\"", &TemplateContext::new(), &options)
            .unwrap_err();
    assert!(error.to_string().contains("U+0000"));
}

#[test]
fn valid_xml_boundaries_unicode_and_whitespace_keep_existing_escaping() {
    let value = "\t\n\r &<>\"' \u{20}\u{d7ff}\u{e000}\u{fffd}\u{10000}\u{10ffff}界";
    let mut ctx = TemplateContext::new();
    ctx.set("value", value);
    let xml = render_template_to_lvgl_xml("input value={value}", &ctx).unwrap();
    assert!(xml.contains("&#9;&#10;&#13; &amp;&lt;&gt;&quot;&apos;"));
    assert!(xml.contains("\u{d7ff}\u{e000}\u{fffd}\u{10000}\u{10ffff}界"));
}
