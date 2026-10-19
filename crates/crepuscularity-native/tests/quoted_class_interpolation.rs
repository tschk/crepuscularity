use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir;
use serde_json::json;

#[test]
fn quoted_class_expressions_with_spaces_lower_to_evaluated_style_tokens() {
    let mut ctx = TemplateContext::new();
    for tone in ["#ff0000", "#0000ff"] {
        ctx.set("tone", tone);
        let ir = render_template_to_ir(
            "span class=\"text-[{tone + ''}] font-bold\"\n  \"VALUE\"",
            &ctx,
        )
        .unwrap();
        let value = serde_json::to_value(ir).unwrap();
        assert_eq!(
            value["root"][0]["style"]["classes"],
            json!([format!("text-[{tone}]"), "font-bold"])
        );
    }
}

#[test]
fn quoted_when_class_values_are_lazy_and_lower_when_enabled() {
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", false);
    let ir = render_template_to_ir(
        "span when:{enabled}=\"text-{1 +} font-bold\"\n  \"VALUE\"",
        &ctx,
    )
    .unwrap();
    let value = serde_json::to_value(ir).unwrap();
    let classes = &value["root"][0]["style"]["classes"];
    assert!(classes.is_null() || classes == &json!([]), "{value}");
    ctx.set("enabled", true);
    for tone in ["#ff0000", "#0000ff"] {
        ctx.set("tone", tone);
        let ir = render_template_to_ir(
            "span when:{enabled}=\"text-[{tone + ''}] font-bold\"\n  \"VALUE\"",
            &ctx,
        )
        .unwrap();
        let value = serde_json::to_value(ir).unwrap();
        assert_eq!(
            value["root"][0]["style"]["classes"],
            json!([format!("text-[{tone}]"), "font-bold"])
        );
    }
}
