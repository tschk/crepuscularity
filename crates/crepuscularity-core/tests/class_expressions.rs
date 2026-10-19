use crepuscularity_core::{ast::Node, parser::parse_template_with_path, TemplateContext};
use std::path::Path;

#[test]
fn quoted_operands_remain_class_expressions_in_all_shared_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("suffix", "blue");
    for (expression, expected) in [
        ("'a' + 'b'", "ab"),
        ("['a' + 'b']", "ab"),
        ("'prefix-' + suffix", "prefix-blue"),
        ("[['prefix-' + suffix]]", "prefix-blue"),
        (r"'snow\',雪 one\t two'", "snow',雪 one two"),
        (r"'one\rtwo'", "one two"),
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
            let actual = element
                .classes
                .iter()
                .map(|class| ctx.interpolate(class).unwrap())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(actual, expected, "{filename}: {expression}");
        }
    }
}
