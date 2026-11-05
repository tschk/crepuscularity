use crepuscularity_core::TemplateContext;

#[test]
fn quoted_attribute_backslashes_are_literal() {
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(
        r#"input placeholder="C:\new\tools""#,
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(xml.contains(r#"placeholder_text="C:\new\tools""#), "{xml}");
}

#[test]
fn markup_numeric_expressions_are_evaluated() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 2_i64);
    for filename in ["view.jsx", "view.svelte", "view.astro"] {
        let nodes = crepuscularity_core::parse_template_with_path(
            "<input value={count + 1} />",
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        let xml = crepuscularity_lvgl::render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default())
            .unwrap();
        assert!(xml.contains("text=\"3\""), "{filename}: {xml}");
    }
}

#[test]
fn conflicting_attributes_emit_once_with_last_value() {
    for (source, expected) in [
        ("div flex-row flex-col", "column"),
        ("div flex-row flex-col flex_flow=\"row\"", "row"),
    ] {
        let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(source, &TemplateContext::new())
            .unwrap();
        assert_eq!(xml.matches("flex_flow=").count(), 1, "{xml}");
        assert!(xml.contains(&format!("flex_flow=\"{expected}\"")), "{xml}");
    }
}

#[test]
fn dynamic_ids_render_once_with_xml_escaping_across_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    for (filename, source) in [
        ("Ids.jsx", "<div id={name} />"),
        ("Ids.svelte", "<div id={name} />"),
        ("Ids.astro", "<div id={name} />"),
        ("Ids.vue", r#"<template><div :id="name" /></template>"#),
        ("Ids.ng.html", r#"<div [id]="name" />"#),
    ] {
        let nodes = crepuscularity_core::parse_template_with_path(
            source,
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        let xml = crepuscularity_lvgl::render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default())
            .unwrap();
        assert_eq!(
            xml.matches(" id=\"entry&amp;anchor\"").count(),
            1,
            "{filename}: {xml}"
        );
    }
}

#[test]
fn dynamic_class_lists_apply_individual_lvgl_styles() {
    let mut ctx = TemplateContext::new();
    ctx.set("theme", "flex flex-col");
    let xml =
        crepuscularity_lvgl::render_template_to_lvgl_xml("div class={ theme }", &ctx).unwrap();
    assert!(xml.contains("layout=\"flex\""), "{xml}");
    assert!(xml.contains("flex_flow=\"column\""), "{xml}");
    assert!(crepuscularity_lvgl::render_template_to_lvgl_xml("div class={1 +}", &ctx).is_err());
}
