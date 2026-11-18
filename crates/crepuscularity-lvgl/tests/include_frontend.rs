use crepuscularity_core::TemplateContext;

#[test]
fn include_depth_boundary_and_named_cycles_are_bounded() {
    let mut ctx = TemplateContext::new();
    let files = std::sync::Arc::make_mut(&mut ctx.virtual_files);
    for index in 0..63 {
        files.insert(
            format!("{index}.crepus"),
            format!("include {}.crepus", index + 1),
        );
    }
    files.insert("63.crepus".into(), "span \"Boundary\"".into());
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml("include 0.crepus", &ctx).unwrap();
    assert!(xml.contains("Boundary"));
    let files = std::sync::Arc::make_mut(&mut ctx.virtual_files);
    files.insert("63.crepus".into(), "include 64.crepus".into());
    files.insert("64.crepus".into(), "span".into());
    files.insert(
        "ui.crepus".into(),
        "--- Card\ninclude ui.crepus#Card".into(),
    );
    for entry in ["include 0.crepus", "include ui.crepus#Card"] {
        let error = crepuscularity_lvgl::render_template_to_lvgl_xml(entry, &ctx).unwrap_err();
        assert!(error.to_string().contains("maximum include depth (64)"));
    }
}

#[test]
fn element_nesting_does_not_consume_include_depth() {
    let mut ctx = TemplateContext::new();
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "span \"Deep child\"".into());
    let mut source = String::new();
    for depth in 0..70 {
        source.push_str(&format!("{}div\n", "  ".repeat(depth)));
    }
    source.push_str(&format!("{}include child.crepus", "  ".repeat(70)));
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(&source, &ctx).unwrap();
    assert!(xml.contains("Deep child"));
}

#[test]
fn includes_use_filename_frontend() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("child.vue"),
        "<script>const ignored = 1 < 2;</script><template><div>Vue label</div></template>",
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml("include child.vue", &ctx).unwrap();
    assert!(xml.contains("Vue label"), "{xml}");
    assert!(!xml.contains("const ignored"));
}
