use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use serde_json::{json, Value};
use std::path::Path;

const FRONTENDS: [&str; 6] = [
    "Entry.crepus",
    "Entry.jsx",
    "Entry.svelte",
    "Entry.vue",
    "Entry.astro",
    "Entry.ng",
];

fn render(filename: &str, tag: &str, attributes: &str, text: &str) -> Value {
    let mut context = TemplateContext::new();
    context.set("text", text);
    let expression = if filename.ends_with(".vue") || filename.ends_with(".ng") {
        "{{text}}"
    } else {
        "{text}"
    };
    let source = if filename.ends_with(".crepus") {
        format!("{tag}\n  option {attributes}\n    {{text}}")
    } else {
        let markup = format!("<{tag}><option {attributes}>{expression}</option></{tag}>");
        if filename.ends_with(".vue") {
            format!("<template>{markup}</template>")
        } else {
            markup
        }
    };
    let ir = render_template_to_ir_with_path(&source, &context, Some(Path::new(filename)))
        .unwrap_or_else(|error| panic!("{filename}: {source}: {error}"));
    serde_json::to_value(ir).unwrap()["root"][0]["options"][0].clone()
}

#[test]
fn standard_options_default_to_text_instead_of_a_slug_or_display_label() {
    for filename in FRONTENDS {
        for (text, expected) in [
            ("Hello World!", "Hello World!"),
            (" \t Hello\n  World! \r ", "Hello World!"),
            ("\u{a0} Hello \u{a0}", "\u{a0} Hello \u{a0}"),
            ("", ""),
        ] {
            assert_eq!(
                render(filename, "select", "", text)["value"],
                expected,
                "{filename}: {text:?}"
            );
        }
        let whitespace_label = render(filename, "select", "label=\"   \"", "Actual Text");
        assert_eq!(
            whitespace_label,
            json!({"value":"Actual Text", "label":"   "}),
            "{filename}"
        );
        let option = render(filename, "select", "label=\"Shown\"", "Actual Text");
        assert_eq!(
            option,
            json!({"value":"Actual Text", "label":"Shown"}),
            "{filename}"
        );
    }
}

#[test]
fn explicit_values_including_empty_win_and_custom_pickers_keep_slug_defaults() {
    for filename in FRONTENDS {
        for value in ["", "Exact Value!"] {
            assert_eq!(
                render(
                    filename,
                    "select",
                    &format!("value=\"{value}\""),
                    "Hello World!"
                )["value"],
                value,
                "{filename}"
            );
        }
        assert_eq!(
            render(filename, "picker", "", "Hello World!")["value"],
            "hello_world",
            "{filename}"
        );
    }
}

#[test]
fn standard_options_concatenate_all_descendant_text() {
    let source = "<select><option>one<span>two</span>three</option></select>";
    let ir = render_template_to_ir_with_path(
        source,
        &TemplateContext::new(),
        Some(Path::new("Entry.jsx")),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(ir).unwrap()["root"][0]["options"][0],
        json!({"value":"onetwothree", "label":"onetwothree"})
    );
}

#[test]
fn explicit_null_option_values_keep_the_scalar_empty_string_contract() {
    for filename in FRONTENDS {
        for expression in ["null", "missing"] {
            let attribute = if filename.ends_with(".vue") {
                format!(":value=\"{expression}\"")
            } else if filename.ends_with(".ng") {
                format!("[value]=\"{expression}\"")
            } else {
                format!("value={{{expression}}}")
            };
            assert_eq!(
                render(filename, "select", &attribute, "Actual Text"),
                json!({"value":"", "label":"Actual Text"}),
                "{filename}: {expression}"
            );
        }
    }
}

#[test]
fn standard_option_text_excludes_script_and_template_subtrees() {
    for (children, expected) in [
        ("one<script>bad()</script>two", "onetwo"),
        ("one<SCRIPT>bad()</SCRIPT>two", "onetwo"),
        ("one<template>ignored</template>two", "onetwo"),
        (
            "one<span>inside<script>bad()</script></span>two",
            "oneinsidetwo",
        ),
        ("one<style>included</style>two", "oneincludedtwo"),
        ("one<span hidden>included</span>two", "oneincludedtwo"),
    ] {
        let ir = render_template_to_ir_with_path(
            &format!("<select><option>{children}</option></select>"),
            &TemplateContext::new(),
            Some(Path::new("Entry.jsx")),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["options"][0],
            json!({"value":expected, "label":expected}),
            "{children}"
        );
    }
}
