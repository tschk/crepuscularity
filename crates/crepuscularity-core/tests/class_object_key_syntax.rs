use crepuscularity_core::{ast::Node, parser::parse_template_with_path, TemplateContext};
use std::path::Path;

fn sources(expression: &str) -> [(&str, String); 3] {
    [
        (
            "Comp.vue",
            format!("<template><div :class=\"{expression}\"/></template>"),
        ),
        ("Comp.ng", format!("<div [ngClass]=\"{expression}\"/>")),
        ("Comp.astro", format!("<div class:list={{{expression}}}/>")),
    ]
}

#[test]
fn class_object_keys_reject_expressions_and_adjacent_tokens() {
    for key in ["'a' + 'b'", "'a' 'b'", "a+b", "a b", "a.b", "a/b"] {
        for (filename, source) in sources(&format!("{{ {key}: true }}")) {
            assert!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_err(),
                "accepted {filename}: {source}"
            );
        }
    }
}

#[test]
fn static_class_object_key_values_and_duplicate_order_are_preserved() {
    for (expression, expected) in [
        ("{ 'a b': true, a: false }", vec!["a", "b"]),
        (r"{ 'say\'hi': true }", vec!["say'hi"]),
        (r"{ 'C:\\new': true }", vec![r"C:\new"]),
        (
            "{ active: true, active: false, other: true }",
            vec!["other"],
        ),
        ("{ active: true, 42: true }", vec!["active", "42"]),
        ("{ 1.5: true }", vec!["1.5"]),
        // Preserve the existing shared parser's bare utility-key extension.
        ("{ font-bold: true }", vec!["font-bold"]),
    ] {
        for (filename, source) in sources(expression) {
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element expected")
            };
            let context = TemplateContext::new();
            let actual: Vec<_> = element
                .conditional_classes
                .iter()
                .filter(|class| context.eval_condition(&class.condition).unwrap())
                .map(|class| context.interpolate(&class.class).unwrap())
                .collect();
            assert_eq!(actual, expected, "{filename}: {expression}");
        }
    }
}
