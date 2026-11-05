#![cfg(feature = "std")]
use crepuscularity_core::{parse_template_with_path, TemplateContext};
use crepuscularity_embedded::{render_nodes_to_document, ScreenSize};
use std::path::Path;

#[test]
fn dynamic_ids_index_containers_text_and_buttons() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry");
    for tag in ["div", "span", "button"] {
        let source = format!("<{tag} id={{name}}>Content</{tag}>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Ids.jsx"))).unwrap();
        let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(64, 32)).unwrap();
        assert_eq!(document.node_by_id("entry").unwrap().tag, tag);
    }
}

#[test]
fn dynamic_ids_index_nodes_across_markup_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry");
    for (filename, source) in [
        ("Ids.jsx", "<div id={name} />"),
        ("Ids.svelte", "<div id={name} />"),
        ("Ids.astro", "<div id={name} />"),
        ("Ids.vue", r#"<template><div :id="name" /></template>"#),
        ("Ids.ng.html", r#"<div [id]="name" />"#),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(64, 32)).unwrap();
        assert!(document.node_by_id("entry").is_some(), "{filename}");
    }
}

#[test]
fn invalid_dynamic_id_expressions_return_errors() {
    let nodes =
        parse_template_with_path("<div id={1 +} />", Some(Path::new("Invalid.jsx"))).unwrap();
    let error = render_nodes_to_document(&nodes, &TemplateContext::new(), ScreenSize::new(64, 32))
        .unwrap_err();
    assert!(error.to_string().contains("expression"), "{error}");
}
