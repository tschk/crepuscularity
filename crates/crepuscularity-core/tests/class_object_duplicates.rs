use crepuscularity_core::{ast::Node, parser::parse_template_with_path, TemplateContext};
use std::path::Path;

#[test]
fn duplicate_class_object_keys_use_final_value_and_original_key_order() {
    for (expression, expected) in [
        ("{ active: true, active: false }", vec![]),
        ("{ active: false, active: true }", vec!["active"]),
        ("{ active: true, 'active': false }", vec![]),
        ("{ 'a b': true, a: false }", vec!["a", "b"]),
        ("{ a: true, b: true, a: true }", vec!["a", "b"]),
        (
            "[{ active: true, active: false }, { active: true }]",
            vec!["active"],
        ),
    ] {
        for (filename, source) in [
            (
                "Comp.vue",
                format!(r#"<template><div :class="{expression}"/></template>"#),
            ),
            (
                "app.component.html",
                format!(r#"<div [ngClass]="{expression}"></div>"#),
            ),
            (
                "Page.astro",
                format!("<div class:list={{{expression}}}></div>"),
            ),
        ] {
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            let ctx = TemplateContext::new();
            let actual: Vec<_> = element
                .conditional_classes
                .iter()
                .filter(|class| ctx.eval_condition(&class.condition).unwrap())
                .map(|class| class.class.as_str())
                .collect();
            assert_eq!(actual, expected, "{filename}: {expression}");
        }
    }
}
