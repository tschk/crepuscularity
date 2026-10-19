use crepuscularity_core::{ast::Node, parse_template_with_path, TemplateContext};
use crepuscularity_native::{render_nodes_to_ir, render_template_to_ir_with_path};
use serde_json::json;
use std::path::Path;

fn lower(markup: &str, context: &TemplateContext) -> serde_json::Value {
    let source = format!("<template>{markup}</template>");
    serde_json::to_value(
        render_template_to_ir_with_path(&source, context, Some(Path::new("Labels.vue"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn vue_text_button_labels_retain_snapshots_and_live_expression_parts() {
    let mut context = TemplateContext::new();
    context.set("label", "<b>Literal markup</b>");
    for markup in [
        "<button v-text='label'/>",
        "<button><span v-text='label'/></button>",
    ] {
        let ir = lower(markup, &context);
        assert_eq!(ir["root"][0]["label"], "<b>Literal markup</b>");
        assert_eq!(
            ir["root"][0]["labelParts"],
            json!([{"kind":"expr","value":"label"}])
        );
    }
}

#[test]
fn option_text_snapshots_and_explicit_label_precedence_are_preserved() {
    let mut context = TemplateContext::new();
    context.set("label", "<b>Literal markup</b>");
    let picker = lower(
        "<picker><option value='one' v-text='label'/></picker>",
        &context,
    );
    assert_eq!(
        picker["root"][0]["options"][0],
        json!({"value":"one","label":"<b>Literal markup</b>"})
    );
    let button = lower("<button label='Explicit' v-text='label'/>", &context);
    assert_eq!(button["root"][0]["label"], "Explicit");
    assert!(button["root"][0].get("labelParts").is_none());
}

#[test]
fn scalar_text_label_values_follow_core_display_rules() {
    for (expression, expected) in [("false", "false"), ("null", "")] {
        let ir = lower(
            &format!("<button v-text='{expression}'/>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["label"], expected);
        assert_eq!(
            ir["root"][0]["labelParts"],
            json!([{"kind":"expr","value":expression}])
        );
    }
}

#[test]
fn public_raw_text_nodes_preserve_the_same_button_label_contract() {
    let mut nodes = parse_template_with_path("<button/>", Some(Path::new("Labels.jsx"))).unwrap();
    let Node::Element(button) = &mut nodes[0] else {
        panic!("expected button")
    };
    button.children.push(Node::RawText("label".into()));
    let mut context = TemplateContext::new();
    context.set("label", "Raw <label>");
    let ir = serde_json::to_value(render_nodes_to_ir(&nodes, &context).unwrap()).unwrap();
    assert_eq!(ir["root"][0]["label"], "Raw <label>");
    assert_eq!(
        ir["root"][0]["labelParts"],
        json!([{"kind":"expr","value":"label"}])
    );
}

#[test]
fn indentation_raw_text_labels_survive_direct_nested_and_option_sources() {
    let mut context = TemplateContext::new();
    context.set("label", "Source <label>");
    for source in ["button\n  {label}", "button\n  span\n    {label}"] {
        let ir =
            serde_json::to_value(render_template_to_ir_with_path(source, &context, None).unwrap())
                .unwrap();
        assert_eq!(ir["root"][0]["label"], "Source <label>", "{source}");
        assert_eq!(
            ir["root"][0]["labelParts"],
            json!([{"kind":"expr","value":"label"}]),
            "{source}"
        );
    }
    let source = "picker\n  option value=one\n    {label}";
    let ir = serde_json::to_value(render_template_to_ir_with_path(source, &context, None).unwrap())
        .unwrap();
    assert_eq!(
        ir["root"][0]["options"][0],
        json!({"value":"one","label":"Source <label>"})
    );
}
