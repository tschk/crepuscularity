use crepuscularity_core::{ast::Node, parse_template_with_path, TemplateContext};
use std::path::Path;

#[test]
fn svelte_quoted_class_expressions_keep_whitespace_and_quoted_delimiters() {
    for (attribute, expected) in [
        (r#"class="w-{1 + 2} flex""#, vec!["w-3", "flex"]),
        (
            r#"class="before-{'a } b'}-after flex""#,
            vec!["before-a } b-after", "flex"],
        ),
        (
            "class='flex\u{2003}w-{1 + 2}\u{00a0}items-center'",
            vec!["flex", "w-3", "items-center"],
        ),
    ] {
        let source = format!("<div {attribute}/>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Entry.svelte"))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        let actual = element
            .classes
            .iter()
            .map(|class| TemplateContext::new().interpolate(class).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{source}");
    }
}

#[test]
fn svelte_class_expressions_remain_expressions_and_plain_classes_stay_identical() {
    for source in [r#"<div class={name}/>"#, r#"<div class="{name}"/>"#] {
        let nodes = parse_template_with_path(source, Some(Path::new("Entry.svelte"))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!()
        };
        let mut ctx = TemplateContext::new();
        ctx.set("name", "active");
        assert_eq!(ctx.interpolate(&element.classes[0]).unwrap(), "active");
    }
    let nodes = parse_template_with_path(
        "<div class='flex items-center'/>",
        Some(Path::new("Entry.svelte")),
    )
    .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!()
    };
    assert_eq!(element.classes, ["flex", "items-center"]);
    let nodes =
        parse_template_with_path(r#"<div class="w-{1 + 2}"/>"#, Some(Path::new("Entry.jsx")))
            .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!()
    };
    let resolved = element
        .classes
        .iter()
        .map(|class| TemplateContext::new().interpolate(class).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(resolved, ["w-{1", "+", "2}"]);
}
