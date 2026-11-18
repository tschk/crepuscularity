use crepuscularity_core::{parse_template_with_path, TemplateContext, TemplateValue};
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, render_template_to_lvgl_xml};
use std::path::Path;

fn assert_disabled(xml: &str, expected: bool) {
    assert_eq!(xml.matches(" disabled=").count(), 1, "{xml}");
    assert!(xml.contains(&format!(" disabled=\"{expected}\"")), "{xml}");
}

#[test]
fn disabled_presence_is_preserved_across_frontends() {
    for (filename, bare, dynamic) in [
        ("A.crepus", "button disabled", "button disabled={state}"),
        ("A.jsx", "<button disabled/>", "<button disabled={state}/>"),
        (
            "A.svelte",
            "<button disabled/>",
            "<button disabled={state}/>",
        ),
        (
            "A.astro",
            "<button disabled/>",
            "<button disabled={state}/>",
        ),
        (
            "A.vue",
            "<template><button disabled/></template>",
            "<template><button :disabled='state'/></template>",
        ),
        ("A.ng", "<button disabled/>", "<button [disabled]='state'/>"),
    ] {
        let bare = parse_template_with_path(bare, Some(Path::new(filename))).unwrap();
        assert_disabled(
            &render_nodes_to_lvgl_xml(&bare, &TemplateContext::new(), &Default::default()).unwrap(),
            true,
        );
        let nodes = parse_template_with_path(dynamic, Some(Path::new(filename))).unwrap();
        let mut ctx = TemplateContext::new();
        assert_disabled(
            &render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap(),
            false,
        );
        for (value, expected) in [
            (TemplateValue::Null, false),
            (TemplateValue::Bool(false), false),
            (TemplateValue::Bool(true), true),
            (TemplateValue::Str("false".into()), true),
            (TemplateValue::Str(String::new()), true),
            (TemplateValue::Int(0), true),
        ] {
            ctx.set("state", value);
            assert_disabled(
                &render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap(),
                expected,
            );
        }
    }
}

#[test]
fn disabled_normalization_keeps_last_binding_and_evaluation_errors() {
    let ctx = TemplateContext::new();
    for (source, expected) in [
        ("button disabled=\"false\"", true),
        ("button disabled={true} DISABLED={null}", false),
        ("button DISABLED={false} disabled", true),
    ] {
        let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
        assert_disabled(&xml, expected);
        assert!(!xml.contains(" DISABLED="));
    }
    let expected = crepuscularity_core::eval::eval_expr("1 +", &ctx)
        .unwrap_err()
        .to_string();
    let actual = render_template_to_lvgl_xml("button disabled={1 +}", &ctx)
        .unwrap_err()
        .to_string();
    assert_eq!(actual, expected);
    assert!(render_template_to_lvgl_xml("if false\n  button disabled={1 +}", &ctx).is_ok());
}
