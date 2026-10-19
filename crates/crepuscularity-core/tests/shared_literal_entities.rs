use std::path::Path;

use crepuscularity_core::{
    ast::{Element, Node, TextPart},
    context::TemplateValue,
    eval::eval_expr,
    parse_template_with_path, TemplateContext,
};

fn eval_string(expression: &str, ctx: &TemplateContext) -> String {
    match eval_expr(expression, ctx).unwrap() {
        TemplateValue::Str(value) => value,
        other => panic!("expected string, got {other:?}"),
    }
}

fn parse_element(markup: &str, filename: &str) -> Element {
    let source = if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup.to_owned()
    };
    let mut nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
    let Node::Element(element) = nodes.remove(0) else {
        panic!("expected element");
    };
    element
}

#[test]
fn shared_literal_text_and_attributes_decode_html_entities_once() {
    for filename in ["view.vue", "view.ng.html"] {
        let element = parse_element(
            "<p title=\"A &amp; &#65; &#x1F600; &NotEqualTilde; &amp;lt; &unknown;\">A &amp; &#65; &#x1F600; &NotEqualTilde; &amp;lt; &unknown;</p>",
            filename,
        );
        let expected = "A & A 😀 ≂\u{338} &lt; &unknown;";
        let ctx = TemplateContext::new();
        assert_eq!(
            eval_string(&element.bindings[0].value, &ctx),
            expected,
            "{filename}"
        );
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal(expected.into())]),
            "{filename}: {:?}",
            element.children
        );
    }
}

#[test]
fn decoded_braces_are_literal_and_runtime_values_are_not_decoded() {
    let mut ctx = TemplateContext::new();
    ctx.set("value", "&amp;");
    for filename in ["view.vue", "view.ng.html"] {
        let element = parse_element(
            "<p title=\"&#123;&#123;value&#125;&#125;\">&#123;&#123;value&#125;&#125;|{{value}}</p>",
            filename,
        );
        assert_eq!(eval_string(&element.bindings[0].value, &ctx), "{{value}}");
        let [Node::Text(parts)] = &element.children[..] else {
            panic!("expected text");
        };
        assert_eq!(
            parts,
            &vec![
                TextPart::Literal("{{value}}|".into()),
                TextPart::Expr("value".into())
            ]
        );
        assert_eq!(eval_string("value", &ctx), "&amp;");
        let dynamic = if filename.ends_with(".vue") {
            "<p :title=\"value\"/>"
        } else {
            "<p title=\"{{value}}\"/>"
        };
        let element = parse_element(dynamic, filename);
        assert_eq!(element.bindings[0].value, "value");
        assert_eq!(eval_string(&element.bindings[0].value, &ctx), "&amp;");
    }
}

#[test]
fn literal_decoding_preserves_raw_carriage_returns_and_existing_id_class_policy() {
    for filename in ["view.vue", "view.ng.html"] {
        let element = parse_element("<p id=\"id&amp;key\" class=\"class&amp;key\" title=\"a\r\nb\rc&amp;d\">a\r\nb\rc&amp;d</p>", filename);
        assert_eq!(element.id.as_deref(), Some("id&amp;key"));
        assert_eq!(element.classes, ["class&amp;key"]);
        assert_eq!(
            eval_string(&element.bindings[0].value, &TemplateContext::new()),
            "a\r\nb\rc&d"
        );
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal("a\r\nb\rc&d".into())])
        );
    }
}

#[test]
fn native_vue_parser_retains_raw_literal_entities_for_its_own_decoder() {
    let nodes = crepuscularity_core::parser::parse_vue_native_file(
        "<template><p title=\"&amp;lt;\">&amp;lt;</p></template>",
    )
    .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(
        eval_string(&element.bindings[0].value, &TemplateContext::new()),
        "&amp;lt;"
    );
    assert!(
        matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal("&amp;lt;".into())])
    );
}

#[test]
fn entity_only_whitespace_remains_literal_text() {
    for filename in ["view.vue", "view.ng.html"] {
        for (source, expected) in [("&nbsp;", "\u{a0}"), ("&#32;", " ")] {
            let element = parse_element(&format!("<p>{source}</p>"), filename);
            assert!(
                matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal(expected.into())]),
                "{filename} {source}: {:?}",
                element.children
            );
        }
        // Literal source whitespace retains the existing markup policy.
        let element = parse_element("<p>   </p>", filename);
        assert!(
            matches!(&element.children[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal("   ".into())]),
            "{filename}: {:?}",
            element.children
        );
    }
}
