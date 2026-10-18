use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_web::render_template_to_html;

fn context() -> TemplateContext {
    let mut item = TemplateContext::new();
    item.set("value", 7_i64);
    let mut data = TemplateContext::new();
    data.set("items", TemplateValue::List(vec![item]));
    let mut ctx = TemplateContext::new();
    ctx.set("data", TemplateValue::Scope(data));
    ctx
}

#[test]
fn plain_render_evaluates_iterator_expressions_and_keeps_scalar_types() {
    let html = render_template_to_html(
        "for item in data.items\n  span \"Value {item + 1}\"",
        &context(),
    )
    .unwrap();
    assert_eq!(html, "<span>Value 8</span>");
    assert!(render_template_to_html("for item in {1 +}\n  span", &context()).is_err());
}

#[cfg(feature = "hydration")]
#[test]
fn hydration_evaluates_the_same_iterator_expression() {
    let html = crepuscularity_web::render_template_to_html_with_hydration(
        "for item in data.items\n  span \"Value {item + 1}\"",
        &context(),
    )
    .unwrap();
    assert!(html.contains("Value 8"), "{html}");
    assert_eq!(html.matches("<span").count(), 1);
    assert!(crepuscularity_web::render_template_to_html_with_hydration(
        "for item in {1 +}\n  span",
        &context()
    )
    .is_err());
}
