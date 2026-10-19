use crepuscularity_core::{ast::Node, parse_template_with_path, TemplateContext};
use std::path::Path;
fn resolved(source: &str, filename: &str, ctx: &TemplateContext) -> Vec<String> {
    let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element")
    };
    element
        .classes
        .iter()
        .map(|class| ctx.interpolate(class).unwrap())
        .chain(
            element
                .conditional_classes
                .iter()
                .filter(|class| ctx.eval_condition(&class.condition).unwrap())
                .map(|class| ctx.interpolate(&class.class).unwrap()),
        )
        .collect()
}
#[test]
fn static_frontend_class_tokens_keep_literal_braces_without_context_reads() {
    for value in ["{name}", "prefix-{name}", "unclosed{", r#"{quote\"slash}"#] {
        // Attribute source uses single quotes; the value contains no single quote.
        for (file, markup) in [
            (
                "Entry.vue",
                format!("<template><div class='{value}'/></template>"),
            ),
            ("Entry.jsx", format!("<div class='{value}'/>")),
            ("Entry.astro", format!("<div class='{value}'/>")),
            ("Entry.ng", format!("<div class='{value}'/>")),
        ] {
            for present in [false, true] {
                let mut ctx = TemplateContext::new();
                if present {
                    ctx.set("name", "CHANGED");
                }
                assert_eq!(resolved(&markup, file, &ctx), [value], "{file}: {markup}");
            }
        }
    }
}
#[test]
fn class_list_literal_entries_and_keys_are_not_reinterpolated() {
    for expression in [
        r"'{name}'",
        r"['{name}']",
        r"{ '{name}': true }",
        r"[['prefix-{name}']]",
    ] {
        let expected = if expression.contains("prefix-") {
            "prefix-{name}"
        } else {
            "{name}"
        };
        for (file, source) in [
            (
                "Entry.vue",
                format!(r#"<template><div :class="{expression}"/></template>"#),
            ),
            ("Entry.ng", format!(r#"<div [ngClass]="{expression}"/>"#)),
            ("Entry.astro", format!("<div class:list={{{expression}}}/>")),
        ] {
            let mut ctx = TemplateContext::new();
            ctx.set("name", "CHANGED");
            assert_eq!(
                resolved(&source, file, &ctx),
                [expected],
                "{file}: {expression}"
            );
        }
    }
}
#[test]
fn authored_interpolation_and_plain_class_tokens_keep_their_contracts() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "active");
    for (file, source) in [
        ("Entry.svelte", r#"<div class="{name}"/>"#),
        ("Entry.ng", r#"<div class="{{ name }}"/>"#),
        ("Entry.jsx", r#"<div class={name}/>"#),
        ("Entry.vue", r#"<template><div :class="name"/></template>"#),
        ("Entry.crepus", r#"div class="{name}""#),
    ] {
        assert_eq!(resolved(source, file, &ctx), ["active"], "{file}");
    }
    for (file, source) in [
        ("Entry.jsx", "<div class='flex items-center'/>"),
        ("Entry.svelte", "<div class='flex items-center'/>"),
        ("Entry.astro", "<div class='flex items-center'/>"),
        (
            "Entry.vue",
            "<template><div class='flex items-center'/></template>",
        ),
        ("Entry.ng", "<div class='flex items-center'/>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(file))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!()
        };
        assert_eq!(element.classes, ["flex", "items-center"]);
    }
}
