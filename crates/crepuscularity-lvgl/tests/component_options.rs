use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::{render_component_file_to_lvgl_xml, LvglOptions, LvglRoot};

#[test]
fn existing_component_api_keeps_its_default_wrapper() {
    let xml = render_component_file_to_lvgl_xml(
        "--- Card\nspan \"Named content\"",
        "Card",
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(xml.contains("<component name=\"Card\">"), "{xml}");
    assert!(xml.contains("Named content"));
}

#[test]
fn options_override_the_wrapper_without_changing_props_or_defaults() {
    let source = "+++\n[Card.defaults]\ntitle = 'Default'\n+++\n--- Card\nspan \"{title}\"";
    let mut context = TemplateContext::new();
    context.set("title", "Override");
    let xml = crepuscularity_lvgl::render_component_file_to_lvgl_xml_with_options(
        source,
        "Card",
        &context,
        &LvglOptions {
            name: "Custom<&".into(),
            root: LvglRoot::Screen,
        },
    )
    .unwrap();
    assert!(xml.contains("<screen name=\"Custom&lt;&amp;\">"), "{xml}");
    assert!(xml.contains("text=\"Override\""), "{xml}");
    assert!(!xml.contains("Default"));
}
