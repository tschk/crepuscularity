use crepuscularity_core::{ast::Node, context::TemplateContext, eval::eval_expr, parse_template};

#[test]
fn landing_page_code_samples_preserve_literal_bindings() {
    let nodes = parse_template(include_str!("../../../docs-site/index.crepus")).unwrap();
    let mut pending: Vec<_> = nodes.iter().collect();
    let mut samples = Vec::new();
    while let Some(node) = pending.pop() {
        match node {
            Node::Element(element) => pending.extend(&element.children),
            Node::RawText(expression) => {
                samples.push(eval_expr(expression, &TemplateContext::new()).unwrap());
            }
            _ => {}
        }
    }
    let text = samples
        .iter()
        .map(crepuscularity_core::context::value_to_str)
        .collect::<Vec<_>>();
    assert!(text.iter().any(|sample| sample
        .contains("input type=\"text\" value={input_value} placeholder=\"Type here\"")));
    assert!(text.iter().any(|sample| sample
        .contains("<input type=\"text\" value={input_value} placeholder=\"Type here\" />")));
    assert!(text.iter().any(|sample| sample.contains(
        "for conv in {conversations}\n        item @click=\"select:{conv.id}\"\n          span \"{conv.title}\""
    )));
}
