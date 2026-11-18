use std::path::Path;

use crepuscularity_core::{parse_template_with_path, TemplateContext};
use crepuscularity_web::render_nodes_to_html;

#[test]
fn frontend_literal_classes_do_not_read_context_variables() {
    for (filename, source) in [
        (
            "view.vue",
            r#"<template><div class="{name} plain"/></template>"#,
        ),
        ("view.astro", r#"<div class="{name} plain"/>"#),
        ("view.jsx", r#"<div className="{name} plain"/>"#),
        ("view.ng.html", r#"<div class="{name} plain"></div>"#),
        (
            "view.vue",
            r#"<template><div :class="['{name}', 'plain']"/></template>"#,
        ),
        (
            "view.vue",
            r#"<template><div :class="{'{name} plain': true}"/></template>"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        for name in ["changed", "another"] {
            let mut ctx = TemplateContext::new();
            ctx.set("name", name);
            let html = render_nodes_to_html(&nodes, &ctx).unwrap();
            assert!(
                html.contains(r#"class="{name} plain""#),
                "{filename}: {source}: {html}"
            );
        }
    }
}

#[test]
fn authored_interpolation_keeps_evaluating() {
    for (filename, source) in [
        ("view.svelte", r#"<div class="{name} plain"/>"#),
        ("view.ng.html", r#"<div class="{{name}}"></div>"#),
        ("view.jsx", r#"<div className={name}/>"#),
        ("view.vue", r#"<template><div :class="name"/></template>"#),
        ("view.crepus", "div {name}"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let mut ctx = TemplateContext::new();
        ctx.set("name", "changed");
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert!(html.contains("changed"), "{filename}: {source}: {html}");
        assert!(!html.contains("{name}"), "{filename}: {source}: {html}");
    }
}

#[test]
fn svelte_quoted_class_interpolation_keeps_expression_whitespace() {
    let source = r#"<div class="before-{1 + 2}-after flex">OK</div>"#;
    let nodes = parse_template_with_path(source, Some(Path::new("view.svelte"))).unwrap();
    let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
    assert!(html.contains(r#"class="before-3-after flex""#), "{html}");
    assert!(html.contains(">OK</div>"), "{html}");
}
