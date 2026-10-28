use crepuscularity_core::{
    ast::{Node, TextPart},
    parse_template_with_path, TemplateContext,
};
use crepuscularity_lvgl::{render_nodes_to_lvgl_xml, LvglOptions};
use std::path::Path;

#[test]
fn vue_literal_braces_are_not_evaluated_again_by_lvgl() {
    let mut context = TemplateContext::new();
    context.set("secret", "resolved value");
    for text in ["{secret}", "{}", "literal {", "界 {secret} text"] {
        let source = format!("<template><label>{text}</label></template>");
        let nodes = parse_template_with_path(&source, Some(Path::new("view.vue"))).unwrap();
        let xml = render_nodes_to_lvgl_xml(&nodes, &context, &LvglOptions::default()).unwrap();
        assert!(xml.contains(&format!("text=\"{text}\"")), "{xml}");
        assert!(!xml.contains("resolved value"), "{xml}");
    }
}

#[test]
fn expression_parts_evaluate_once_while_literal_parts_stay_literal() {
    let mut context = TemplateContext::new();
    context.set("value", "{other}");
    context.set("other", "wrong");
    let nodes = [Node::Text(vec![
        TextPart::Literal("{other}: ".into()),
        TextPart::Expr("value".into()),
    ])];
    let xml = render_nodes_to_lvgl_xml(&nodes, &context, &LvglOptions::default()).unwrap();
    assert!(xml.contains("text=\"{other}: {other}\""), "{xml}");
}
