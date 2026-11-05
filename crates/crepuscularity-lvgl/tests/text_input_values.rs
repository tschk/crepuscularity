use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, render_template_to_lvgl_xml};

#[test]
fn input_value_alias_uses_text_across_template_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "A&B\"<tag>");
    for (filename, source) in [
        (
            "form.crepus",
            "input value={label}\ntextarea bind:value={label}",
        ),
        (
            "form.jsx",
            "<><input value={label}/><textarea value={label}/></>",
        ),
        (
            "form.svelte",
            "<input value={label}/><textarea bind:value={label}/>",
        ),
        (
            "form.astro",
            "<input value={label}/><textarea value={label}/>",
        ),
        (
            "form.vue",
            "<template><input :value=\"label\"/><textarea :value=\"label\"/></template>",
        ),
        (
            "form.ng.html",
            "<input [value]=\"label\"/><textarea [value]=\"label\"/>",
        ),
    ] {
        let nodes = crepuscularity_core::parse_template_with_path(
            source,
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        let xml = render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
        assert_eq!(xml.matches("<lv_textarea").count(), 2, "{filename}: {xml}");
        assert_eq!(
            xml.matches(" text=\"A&amp;B&quot;&lt;tag&gt;\"").count(),
            2,
            "{filename}: {xml}"
        );
        assert!(!xml.contains(" value="), "{filename}: {xml}");
    }
}

#[test]
fn value_alias_preserves_attribute_and_body_precedence() {
    for (source, expected) in [
        ("input value=\"static\"", "static"),
        ("textarea value=\"static\"", "static"),
        ("input value=\"first\" text=\"last\"", "last"),
        ("textarea text=\"first\" value=\"last\"", "last"),
        ("textarea value=\"property\"\n  \"body\"", "body"),
    ] {
        let xml = render_template_to_lvgl_xml(source, &TemplateContext::new()).unwrap();
        assert_eq!(xml.matches(" text=").count(), 1, "{xml}");
        assert!(xml.contains(&format!(" text=\"{expected}\"")), "{xml}");
        assert!(!xml.contains(" value="), "{xml}");
    }
    for tag in ["slider", "progress", "meter"] {
        let xml =
            render_template_to_lvgl_xml(&format!("{tag} value={{3}}"), &TemplateContext::new())
                .unwrap();
        assert!(xml.contains(" value=\"3\""), "{xml}");
        assert!(!xml.contains(" text="), "{xml}");
    }
}

#[test]
fn included_named_inputs_resolve_prop_values_before_mapping() {
    let mut ctx = TemplateContext::new();
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "field.crepus".into(),
        "--- Field\ntextarea value={label}".into(),
    );
    let xml =
        render_template_to_lvgl_xml("include field.crepus#Field label=\"Included\"", &ctx).unwrap();
    assert!(xml.contains("<lv_textarea text=\"Included\"/>"), "{xml}");
    assert!(!xml.contains(" value="), "{xml}");
}
