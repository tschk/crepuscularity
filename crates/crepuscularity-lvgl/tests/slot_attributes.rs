use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn fallback_slot_wrappers_keep_attributes_and_styles() {
    let mut context = TemplateContext::new();
    context.set("hide", true);
    let xml = render_template_to_lvgl_xml(
        "slot #content p-4 hidden={hide} @click=\"activate\"\n  span \"Fallback\"",
        &context,
    )
    .unwrap();
    for attribute in [
        "id=\"content\"",
        "pad_all=\"16\"",
        "hidden=\"true\"",
        "data_on_click=\"activate\"",
        "text=\"Fallback\"",
    ] {
        assert!(xml.contains(attribute), "{attribute}: {xml}");
    }
}

#[test]
fn slot_wrapper_uses_component_props_and_content_uses_the_caller_context() {
    let mut context = TemplateContext::new();
    context.set("title", "caller");
    std::sync::Arc::make_mut(&mut context.virtual_files).insert(
        "shell.crepus".into(),
        "slot id={title} p-4\n  span \"Fallback\"".into(),
    );
    let xml = render_template_to_lvgl_xml(
        "include shell.crepus title=\"callee\"\n  span \"{title}\"",
        &context,
    )
    .unwrap();
    assert!(xml.contains("id=\"callee\""), "{xml}");
    assert!(xml.contains("pad_all=\"16\""), "{xml}");
    assert!(xml.contains("text=\"caller\""), "{xml}");
    assert!(!xml.contains("Fallback"), "{xml}");
}

#[test]
fn supplied_slot_includes_resolve_relative_to_the_caller() {
    let mut context = TemplateContext::new();
    context.base_dir = Some("/virtual/root".into());
    let files = std::sync::Arc::make_mut(&mut context.virtual_files);
    files.insert(
        "/virtual/root/components/shell.crepus".into(),
        "slot #content".into(),
    );
    files.insert(
        "/virtual/root/leaf.crepus".into(),
        "span \"Caller leaf\"".into(),
    );
    files.insert(
        "/virtual/root/components/leaf.crepus".into(),
        "span \"Wrong leaf\"".into(),
    );
    let xml = render_template_to_lvgl_xml(
        "include components/shell.crepus\n  include leaf.crepus",
        &context,
    )
    .unwrap();
    assert!(xml.contains("id=\"content\""), "{xml}");
    assert!(xml.contains("Caller leaf"), "{xml}");
    assert!(!xml.contains("Wrong leaf"), "{xml}");
}
