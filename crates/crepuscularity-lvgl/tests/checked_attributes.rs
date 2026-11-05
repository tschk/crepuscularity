use crepuscularity_core::{parse_template_with_path, TemplateContext, TemplateValue};
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, render_template_to_lvgl_xml};
use std::path::Path;

fn checked(xml: &str, expected: bool) {
    assert_eq!(xml.matches(" checked=").count(), 1, "{xml}");
    assert!(xml.contains(&format!(" checked=\"{expected}\"")), "{xml}");
}

#[test]
fn direct_checked_matches_custom_control_coercion() {
    for tag in ["checkbox", "switch"] {
        checked(
            &render_template_to_lvgl_xml(&format!("{tag} checked"), &TemplateContext::new())
                .unwrap(),
            true,
        );
        checked(
            &render_template_to_lvgl_xml(
                &format!("{tag} checked={{missing}}"),
                &TemplateContext::new(),
            )
            .unwrap(),
            false,
        );
        for (value, expected) in [
            (TemplateValue::Null, false),
            (TemplateValue::Bool(false), false),
            (TemplateValue::Bool(true), true),
            (TemplateValue::Int(0), false),
            (TemplateValue::Int(1), true),
            (TemplateValue::Int(2), false),
            (TemplateValue::Float(0.0), false),
            (TemplateValue::Float(1.0), true),
        ]
        .into_iter()
        .chain(
            [
                ("", true),
                ("true", true),
                ("1", true),
                ("yes", true),
                ("on", true),
                ("false", false),
                ("0", false),
                ("no", false),
                ("off", false),
                ("unknown", false),
                ("TRUE", false),
                (" yes ", true),
            ]
            .into_iter()
            .map(|(s, b)| (TemplateValue::Str(s.into()), b)),
        ) {
            let mut ctx = TemplateContext::new();
            ctx.set("state", value);
            checked(
                &render_template_to_lvgl_xml(&format!("{tag} checked={{state}}"), &ctx).unwrap(),
                expected,
            );
        }
    }
}

#[test]
fn dynamic_checked_is_normalized_across_frontends() {
    for (filename, source) in [
        ("A.crepus", "checkbox checked={state}"),
        ("A.jsx", "<checkbox checked={state}/>"),
        ("A.svelte", "<checkbox checked={state}/>"),
        ("A.astro", "<checkbox checked={state}/>"),
        ("A.vue", "<template><checkbox :checked='state'/></template>"),
        ("A.ng", "<checkbox [checked]='state'/>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let mut ctx = TemplateContext::new();
        for (value, expected) in [
            (TemplateValue::Null, false),
            (TemplateValue::Str("off".into()), false),
            (TemplateValue::Str("on".into()), true),
        ] {
            ctx.set("state", value);
            checked(
                &render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap(),
                expected,
            );
        }
    }
}

#[test]
fn checked_order_errors_and_scope_remain_intact() {
    let ctx = TemplateContext::new();
    for (source, expected) in [
        ("switch bind={true} checked={null}", false),
        ("switch checked={null} bind={true}", true),
        ("checkbox checked={true} CHECKED={0}", false),
    ] {
        checked(
            &render_template_to_lvgl_xml(source, &ctx).unwrap(),
            expected,
        );
    }
    let expected = crepuscularity_core::eval::eval_expr("1 +", &ctx)
        .unwrap_err()
        .to_string();
    assert_eq!(
        render_template_to_lvgl_xml("checkbox checked={1 +}", &ctx)
            .unwrap_err()
            .to_string(),
        expected
    );
    assert!(render_template_to_lvgl_xml("if false\n  checkbox checked={1 +}", &ctx).is_ok());
    assert!(render_template_to_lvgl_xml("checkbox bind={0}", &ctx)
        .unwrap_err()
        .to_string()
        .contains("boolean snapshot"));
    let xml = render_template_to_lvgl_xml("button checked={null}", &ctx).unwrap();
    assert!(xml.contains("checked=\"\""), "{xml}");
}
