use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, render_template_to_lvgl_xml};
use std::path::Path;

fn hidden(xml: &str, expected: bool) {
    assert_eq!(xml.matches(" hidden=").count(), 1, "{xml}");
    assert!(xml.contains(&format!(" hidden=\"{expected}\"")), "{xml}");
}

#[test]
fn hidden_attributes_follow_presence_semantics_in_all_frontends() {
    let mut ctx = TemplateContext::new();
    for (filename, bare, dynamic) in [
        ("Entry.crepus", "div hidden", "div hidden={state}"),
        ("Entry.jsx", "<div hidden/>", "<div hidden={state}/>"),
        ("Entry.svelte", "<div hidden/>", "<div hidden={state}/>"),
        ("Entry.astro", "<div hidden/>", "<div hidden={state}/>"),
        (
            "Entry.vue",
            "<template><div hidden/></template>",
            "<template><div :hidden='state'/></template>",
        ),
        ("Entry.ng", "<div hidden/>", "<div [hidden]='state'/>"),
    ] {
        let nodes =
            crepuscularity_core::parse_template_with_path(bare, Some(Path::new(filename))).unwrap();
        hidden(
            &render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap(),
            true,
        );
        let nodes =
            crepuscularity_core::parse_template_with_path(dynamic, Some(Path::new(filename)))
                .unwrap();
        for (value, expected) in [
            (TemplateValue::Bool(false), false),
            (TemplateValue::Null, false),
            (TemplateValue::Bool(true), true),
            (TemplateValue::Str("false".into()), true),
            (TemplateValue::Str("".into()), true),
            (TemplateValue::Int(0), true),
        ] {
            ctx.set("state", value);
            hidden(
                &render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap(),
                expected,
            );
        }
    }
}

#[test]
fn hidden_normalization_preserves_attribute_precedence_and_errors() {
    for (source, expected) in [
        ("div hidden", true),
        ("div hidden=\"\"", true),
        ("div hidden=\"false\"", true),
        ("div hidden={false}", false),
        ("div hidden={null}", false),
        ("div class=\"hidden\" hidden={false}", false),
        ("div hidden=true hidden={null}", false),
        ("div hidden={null} hidden=\"false\"", true),
    ] {
        hidden(
            &render_template_to_lvgl_xml(source, &TemplateContext::new()).unwrap(),
            expected,
        );
    }
    assert!(render_template_to_lvgl_xml("div hidden={1 +}", &TemplateContext::new()).is_err());
    assert!(
        render_template_to_lvgl_xml("if false\n  div hidden={1 +}", &TemplateContext::new())
            .is_ok()
    );
}

#[test]
fn ordinary_and_named_include_props_retain_typed_hidden_values() {
    let mut ctx = TemplateContext::new();
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "div hidden={conceal}".into());
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "named.crepus".into(),
        "--- Card\n$: default conceal = true\ndiv hidden={conceal}".into(),
    );
    for include in ["child.crepus", "named.crepus#Card"] {
        for (value, expected) in [("null", false), ("false", false), ("\"false\"", true)] {
            hidden(
                &render_template_to_lvgl_xml(
                    &format!("include {include} conceal={{{value}}}"),
                    &ctx,
                )
                .unwrap(),
                expected,
            );
        }
    }
}
