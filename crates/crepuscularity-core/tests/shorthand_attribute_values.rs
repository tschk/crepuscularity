use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

#[test]
fn shorthand_attributes_cannot_discard_an_assigned_value() {
    for filename in ["view.svelte", "view.astro"] {
        for value in ["{other}", "'other'", "false", "true", "''"] {
            let source = format!("<div {{title}}={value}></div>");
            assert!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_err(),
                "accepted {filename}: {source}"
            );
        }
    }
}

#[test]
fn bare_shorthand_keeps_its_named_expression() {
    for filename in ["view.svelte", "view.astro"] {
        let nodes =
            parse_template_with_path("<div {title}></div>", Some(Path::new(filename))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        assert_eq!(element.bindings.len(), 1);
        assert_eq!(element.bindings[0].prop, "title");
        assert_eq!(element.bindings[0].value, "title");
    }
}

#[test]
fn shorthand_names_allow_surrounding_whitespace() {
    for filename in ["view.svelte", "view.astro"] {
        let expected =
            parse_template_with_path("<div {title}></div>", Some(Path::new(filename))).unwrap();
        for shorthand in ["{ title }", "{\ttitle\t}", "{\ntitle\n}"] {
            let source = format!("<div {shorthand}></div>");
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            assert_eq!(nodes, expected, "{filename}: {source}");
        }
        for source in ["<div { title ></div>", "<div { title }={other}></div>"] {
            assert!(
                parse_template_with_path(source, Some(Path::new(filename))).is_err(),
                "{filename}: {source}"
            );
        }
    }
}

#[test]
fn shorthand_requires_one_supported_variable_name() {
    for filename in ["view.svelte", "view.astro"] {
        for name in [
            "a+b",
            "user.name",
            "name()",
            "123",
            "true",
            "null",
            "a-b",
            "a b",
            "界",
            "$name",
            "'title'",
        ] {
            let source = format!("<div {{{name}}}></div>");
            assert!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_err(),
                "accepted {filename}: {source}"
            );
        }
        for name in ["title", "_value", "value2"] {
            let source = format!("<div {{{name}}}></div>");
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            let mut context = crepuscularity_core::TemplateContext::new();
            context.set(name, 42_i64);
            assert!(matches!(
                crepuscularity_core::eval::eval_expr(&element.bindings[0].value, &context).unwrap(),
                crepuscularity_core::TemplateValue::Int(42)
            ));
        }
    }
}
