use crepuscularity_core::parser::parse_template_with_path;
use std::path::Path;

#[test]
fn computed_class_object_keys_fail_in_every_shared_frontend() {
    for key in ["[name]", "['active']", "[prefix + suffix]"] {
        for (filename, source) in [
            (
                "Comp.vue",
                format!(r#"<template><div :class="{{ {key}: enabled }}"/></template>"#),
            ),
            (
                "app.component.html",
                format!(r#"<div [ngClass]="{{ {key}: enabled }}"></div>"#),
            ),
            (
                "Page.astro",
                format!(r#"<div class:list={{{{ {key}: enabled }}}}></div>"#),
            ),
        ] {
            let error = parse_template_with_path(&source, Some(Path::new(filename)))
                .expect_err(&format!("{filename} silently accepted computed key {key}"));
            assert!(
                error.to_string().contains("computed class"),
                "{filename}: {error}"
            );
        }
    }
}

#[test]
fn quoted_bracket_class_keys_remain_static() {
    for (filename, source) in [
        (
            "Comp.vue",
            r#"<template><div :class="{ '[name]': enabled }"/></template>"#,
        ),
        (
            "app.component.html",
            r#"<div [class]="{ '[name]': enabled }"></div>"#,
        ),
        (
            "Page.astro",
            r#"<div class:list={{ '[name]': enabled }}></div>"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let crepuscularity_core::ast::Node::Element(element) = &nodes[0] else {
            panic!("{filename}: expected element");
        };
        assert_eq!(element.conditional_classes.len(), 1, "{filename}");
        assert_eq!(element.conditional_classes[0].class, "[name]", "{filename}");
        assert_eq!(
            element.conditional_classes[0].condition, "enabled",
            "{filename}"
        );
    }
}
