use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn canonical_paths_and_expressions_resolve_control_snapshots() {
    for (tag, value, attr, expected) in [
        ("input", TemplateValue::Str("hello".into()), "text", "hello"),
        (
            "textarea",
            TemplateValue::Str("hello".into()),
            "text",
            "hello",
        ),
        ("checkbox", TemplateValue::Bool(true), "checked", "true"),
        ("switch", TemplateValue::Bool(false), "checked", "false"),
        ("select", TemplateValue::Int(0), "selected", "0"),
        ("dropdown", TemplateValue::Int(1), "selected", "1"),
        ("slider", TemplateValue::Int(-4), "value", "-4"),
        (
            "slider",
            TemplateValue::Int(i32::MIN.into()),
            "value",
            "-2147483648",
        ),
        (
            "slider",
            TemplateValue::Int(i32::MAX.into()),
            "value",
            "2147483647",
        ),
        (
            "dropdown",
            TemplateValue::Int(i32::MAX.into()),
            "selected",
            "2147483647",
        ),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("field", value);
        ctx.set("hello", "WRONG second lookup");
        for binding in ["field", "{field}", "\"field\""] {
            let xml = render_template_to_lvgl_xml(&format!("{tag} bind={binding}"), &ctx).unwrap();
            assert!(xml.contains(&format!(" {attr}=\"{expected}\"")), "{xml}");
            assert!(!xml.contains(" bind="), "{xml}");
            assert!(!xml.contains(" bind_value="), "{xml}");
        }
    }
}

#[test]
fn canonical_bindings_keep_scopes_and_attribute_precedence() {
    let mut ctx = TemplateContext::new();
    let mut nested = TemplateContext::new();
    nested.set("enabled", true);
    ctx.set("settings", TemplateValue::Scope(nested));
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "field.crepus".into(),
        "--- Field\nswitch bind=enabled".into(),
    );
    for source in [
        "switch bind=settings.enabled",
        "$: let enabled = true\nswitch bind=enabled",
        "include field.crepus#Field enabled={settings.enabled}",
    ] {
        let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
        assert!(xml.contains(" checked=\"true\""), "{xml}");
    }
    let mut first = TemplateContext::new();
    first.set("value", true);
    let mut second = TemplateContext::new();
    second.set("value", false);
    ctx.set("flags", TemplateValue::List(vec![first, second]));
    let xml = render_template_to_lvgl_xml("for flag in flags\n  checkbox bind=flag", &ctx).unwrap();
    assert!(
        xml.contains("<lv_checkbox checked=\"true\"/>\n    <lv_checkbox checked=\"false\"/>"),
        "{xml}"
    );
    for (source, expected) in [
        ("switch bind={true} checked={false}", "false"),
        ("switch checked={false} bind={true}", "true"),
    ] {
        let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
        assert_eq!(xml.matches(" checked=").count(), 1, "{xml}");
        assert!(xml.contains(&format!(" checked=\"{expected}\"")), "{xml}");
    }
}

#[test]
fn svelte_select_and_named_defaults_resolve_canonical_bindings() {
    let mut ctx = TemplateContext::new();
    ctx.set("selected", 1_i64);
    let nodes = crepuscularity_core::parse_template_with_path(
        "<select bind:value={selected}/>",
        Some(std::path::Path::new("Form.svelte")),
    )
    .unwrap();
    let xml =
        crepuscularity_lvgl::render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
    assert!(xml.contains("<lv_dropdown selected=\"1\"/>"), "{xml}");
    let xml = crepuscularity_lvgl::render_component_file_to_lvgl_xml(
        "+++\n[Toggle.defaults]\nenabled = true\n+++\n--- Toggle\nswitch bind=enabled",
        "Toggle",
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(xml.contains("<lv_switch checked=\"true\"/>"), "{xml}");
}

#[test]
fn typed_controls_reject_incompatible_snapshots() {
    for source in [
        "checkbox bind={1}",
        "switch bind={\"yes\"}",
        "select bind={-1}",
        "dropdown bind={1.5}",
        "select bind={\"option\"}",
        "slider bind={3.5}",
        "slider bind={2147483648}",
        "slider bind={-2147483649}",
        "dropdown bind={2147483648}",
        "switch bind=missing",
        "div bind={1}",
    ] {
        let result = render_template_to_lvgl_xml(source, &TemplateContext::new());
        assert!(
            result.is_err(),
            "accepted incompatible snapshot: {source}: {result:?}"
        );
    }
}
