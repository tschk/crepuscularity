use std::path::Path;

use crepuscularity_core::{
    ast::{Node, TextPart},
    eval::eval_expr,
    parser::{parse_astro_module_template, parse_template_with_path},
    TemplateContext,
};

fn operand(nodes: &[Node], raw: bool) -> &str {
    let Node::Element(element) = &nodes[0] else {
        panic!("element expected")
    };
    match (&element.children[..], raw) {
        ([Node::RawHtml(expression)], true) => expression,
        ([Node::Text(parts)], false) => match &parts[..] {
            [TextPart::Expr(expression)] => expression,
            _ => panic!("single text operand expected"),
        },
        _ => panic!("content directive expected"),
    }
}

#[test]
fn shared_content_literals_preserve_decoded_controls_and_escaping() {
    for (directive, raw) in [("set:text", false), ("set:html", true)] {
        for (literal, expected) in [
            ("A&#13;B", "A\rB"),
            ("A&#1;B", "A\u{1}B"),
            ("A&#10;B", "A\nB"),
            ("A&#9;B", "A\tB"),
            ("&quot;C:\\new\\tools&quot;", "\"C:\\new\\tools\""),
        ] {
            let source = format!("<p {directive}=\"{literal}\"/>");
            let nodes = parse_template_with_path(&source, Some(Path::new("Page.astro"))).unwrap();
            let value = eval_expr(operand(&nodes, raw), &TemplateContext::new()).unwrap();
            assert_eq!(
                crepuscularity_core::context::value_to_str(&value),
                expected,
                "{source}"
            );
        }
    }
}

#[test]
fn authored_content_literals_remain_json_string_expressions() {
    for (directive, raw) in [("set:text", false), ("set:html", true)] {
        let nodes =
            parse_astro_module_template(&format!("<p {directive}=\"A&#13;&#1;&#10;&quot;\\B\"/>"))
                .unwrap();
        let expression = operand(&nodes, raw);
        assert_eq!(
            serde_json::from_str::<String>(expression).unwrap(),
            "A\r\u{1}\n\"\\B"
        );
        assert!(expression.contains("\\r") && expression.contains("\\u0001"));
    }
}
