use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

#[test]
fn constant_id_strings_use_the_evaluators_escape_rules_for_both_quote_styles() {
    for (body, expected) in [
        (r"a\rb", r"a\rb"),
        (r"a\u0041b", r"a\u0041b"),
        (r"a\qb", r"a\qb"),
        (r"a\nb", "a\nb"),
        (r"a\tb", "a\tb"),
        (r"a\\b", r"a\b"),
    ] {
        for quote in ['\'', '"'] {
            let source = format!("<div id={{{quote}{body}{quote}}}/>");
            let nodes = parse_template_with_path(&source, Some(Path::new("Entry.jsx"))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert_eq!(element.id.as_deref(), Some(expected), "{source}");
            assert!(element.bindings.is_empty(), "{source}");
        }
    }
    for (expression, expected) in [(r#""a\"b""#, "a\"b"), (r"'a\'b'", "a'b")] {
        let source = format!("<div id={{{expression}}}/>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Entry.jsx"))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!()
        };
        assert_eq!(element.id.as_deref(), Some(expected), "{source}");
    }
}

#[test]
fn static_id_backslashes_and_compound_expression_bindings_remain_distinct() {
    let nodes = parse_template_with_path(
        r#"<div id="a\rb"/><div id={"prefix-" + suffix}/>"#,
        Some(Path::new("Entry.jsx")),
    )
    .unwrap();
    let Node::Element(literal) = &nodes[0] else {
        panic!()
    };
    assert_eq!(literal.id.as_deref(), Some(r"a\rb"));
    let Node::Element(dynamic) = &nodes[1] else {
        panic!()
    };
    assert_eq!(dynamic.id, None);
    assert_eq!(dynamic.bindings[0].prop, "id");
    assert_eq!(dynamic.bindings[0].value, r#""prefix-" + suffix"#);
}
