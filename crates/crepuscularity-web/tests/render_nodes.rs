use crepuscularity_core::{
    ast::{Element, Node, TextPart},
    context::TemplateContext,
};
use crepuscularity_web::render_nodes_to_html;

#[test]
fn test_render_nodes_to_html() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Alice");

    let nodes = vec![Node::Element(Element {
        tag: "div".to_string(),
        id: Some("greeting".to_string()),
        classes: vec!["card".to_string()],
        conditional_classes: vec![],
        event_handlers: vec![],
        bindings: vec![],
        animations: vec![],
        children: vec![Node::Text(vec![
            TextPart::Literal("Hello, ".to_string()),
            TextPart::Expr("name".to_string()),
            TextPart::Literal("!".to_string()),
        ])],
    })];
    let html = render_nodes_to_html(&nodes, &ctx).unwrap();
    assert_eq!(
        html,
        "<div id=\"greeting\" class=\"card\">Hello, Alice!</div>"
    );
}

#[test]
fn test_render_nodes_to_html_let_decl() {
    use crepuscularity_core::ast::LetDecl;
    let ctx = TemplateContext::new();

    let nodes = vec![
        Node::LetDecl(LetDecl {
            name: "count".to_string(),
            expr: "42".to_string(),
            is_default: false,
        }),
        Node::Element(Element {
            tag: "span".to_string(),
            id: None,
            classes: vec![],
            conditional_classes: vec![],
            event_handlers: vec![],
            bindings: vec![],
            animations: vec![],
            children: vec![Node::Text(vec![TextPart::Expr("count".to_string())])],
        }),
    ];
    let html = render_nodes_to_html(&nodes, &ctx).unwrap();
    assert_eq!(html, "<span>42</span>");
}
