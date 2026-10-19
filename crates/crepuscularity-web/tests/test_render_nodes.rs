use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_core::parser::parse_template;
use crepuscularity_web::render_nodes_to_html;

#[test]
fn test_render_nodes_to_html_basic() {
    let source = r#"<div class="hello {world}">Hello, {world}!</div>"#;
    let template_nodes = parse_template(source).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("world".to_string(), TemplateValue::Str("Earth".to_string()));

    let result = render_nodes_to_html(&template_nodes, &ctx).unwrap();
    assert_eq!(result, r#"<div class="hello Earth">Hello, Earth!</div>"#);
}

#[test]
fn test_render_nodes_to_html_with_let() {
    let source = "$: let x = 42;\n<div>{x}</div>";
    let template_nodes = parse_template(source).unwrap();
    let ctx = TemplateContext::new();

    let result = render_nodes_to_html(&template_nodes, &ctx).unwrap();
    assert_eq!(result, "<div>42</div>");
}

#[test]
fn test_render_nodes_to_html_empty() {
    let source = "";
    let template_nodes = parse_template(source).unwrap();
    let ctx = TemplateContext::new();
    let result = render_nodes_to_html(&template_nodes, &ctx).unwrap();
    assert_eq!(result, "");
}
