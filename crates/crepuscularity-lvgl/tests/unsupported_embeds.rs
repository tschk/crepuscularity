use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn active_embeds_error_in_roots_includes_and_slots() {
    let mut context = TemplateContext::new();
    let files = std::sync::Arc::make_mut(&mut context.virtual_files);
    files.insert(
        "child.crepus".into(),
        "embed widget.js adapter=\"react\"".into(),
    );
    files.insert("shell.crepus".into(), "slot".into());
    files.insert(
        "components.crepus".into(),
        "--- Card\nembed widget.js adapter=\"react\"".into(),
    );
    for source in [
        "embed widget.js adapter=\"react\"",
        "include child.crepus",
        "include components.crepus#Card",
        "include shell.crepus\n  embed widget.js adapter=\"react\"",
    ] {
        let error = render_template_to_lvgl_xml(source, &context).unwrap_err();
        assert!(error.to_string().contains("embed"), "{error}");
        assert!(error.to_string().contains("widget.js"), "{error}");
    }
}

#[test]
fn skipped_web_islands_do_not_block_lvgl_output() {
    let xml = render_template_to_lvgl_xml(
        "if {is_web}\n  embed widget.js adapter=\"react\"\nelse\n  span \"LVGL fallback\"",
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(xml.contains("text=\"LVGL fallback\""), "{xml}");
    assert!(!xml.contains("widget.js"), "{xml}");
}
