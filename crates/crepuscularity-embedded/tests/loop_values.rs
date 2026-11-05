#![cfg(feature = "std")]

use crepuscularity_core::{TemplateContext, TemplateValue};

#[test]
fn loop_bindings_preserve_scalar_types_and_object_scopes() {
    for (value, expression, expected) in [
        (Some(TemplateValue::Int(2)), "item + 1", "3"),
        (Some(TemplateValue::Bool(false)), "item == false", "true"),
        (
            Some(TemplateValue::Str(String::new())),
            "item == ''",
            "true",
        ),
        (Some(TemplateValue::Null), "item == null", "true"),
        (None, "item.name", "雪"),
    ] {
        let mut item = TemplateContext::new();
        item.set("name", "雪");
        if let Some(value) = value {
            item.set("value", value);
        }
        let mut ctx = TemplateContext::new();
        ctx.set("item", "parent value must not leak");
        ctx.set("items", TemplateValue::List(vec![item]));
        let source = format!("for item in items\n  span \"{{{expression}}}\"");
        let nodes = crepuscularity_core::parse_template(&source).unwrap();
        let document = crepuscularity_embedded::render_nodes_to_document(
            &nodes,
            &ctx,
            crepuscularity_embedded::ScreenSize::new(64, 32),
        )
        .unwrap();
        let text = document.root[0].children[0].text.as_deref();
        assert_eq!(text, Some(expected), "{expression}");
    }
}

#[test]
fn loop_iterator_expressions_resolve_nested_lists() {
    let mut item = TemplateContext::new();
    item.set("value", 7_i64);
    let mut data = TemplateContext::new();
    data.set("items", TemplateValue::List(vec![item]));
    let mut ctx = TemplateContext::new();
    ctx.set("data", TemplateValue::Scope(data));
    let source = "for item in data.items\n  span \"Value {item + 1}\"";
    let nodes = crepuscularity_core::parse_template(source).unwrap();
    let document = crepuscularity_embedded::render_nodes_to_document(
        &nodes,
        &ctx,
        crepuscularity_embedded::ScreenSize::new(64, 32),
    )
    .unwrap();
    assert_eq!(
        document.root[0].children[0].text.as_deref(),
        Some("Value 8")
    );
    let size = crepuscularity_embedded::ScreenSize::new(64, 32);
    let mut actual =
        crepuscularity_embedded::Rgb565Buffer::new(size, crepuscularity_embedded::DEFAULT_BG);
    let mut expected =
        crepuscularity_embedded::Rgb565Buffer::new(size, crepuscularity_embedded::DEFAULT_BG);
    crepuscularity_embedded::render_template_to_framebuffer(source, &ctx, size, &mut actual)
        .unwrap();
    crepuscularity_embedded::render_template_to_framebuffer(
        "span \"Value 8\"",
        &ctx,
        size,
        &mut expected,
    )
    .unwrap();
    assert_eq!(actual.pixels(), expected.pixels());
    let nodes = crepuscularity_core::parse_template("for item in {1 +}\n  span").unwrap();
    assert!(crepuscularity_embedded::render_nodes_to_document(
        &nodes,
        &ctx,
        crepuscularity_embedded::ScreenSize::new(64, 32)
    )
    .is_err());
}
