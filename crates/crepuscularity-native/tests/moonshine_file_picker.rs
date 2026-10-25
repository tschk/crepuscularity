#![cfg(feature = "web-modules")]

use crepuscularity_native::{emit_moonshine_dom_component, emit_moonshine_ssr_component, ViewIr};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, ObjectPropertyKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::json;

#[derive(Default)]
struct Elements {
    nodes: Vec<(String, Vec<String>)>,
}
impl<'a> Visit<'a> for Elements {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let [Argument::StringLiteral(tag), Argument::ObjectExpression(props), ..] =
            &call.arguments[..]
        {
            let keys = props
                .properties
                .iter()
                .filter_map(|property| {
                    if let ObjectPropertyKind::ObjectProperty(property) = property {
                        property.key.static_name().map(|name| name.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            self.nodes.push((tag.value.to_string(), keys));
        }
        walk::walk_call_expression(self, call);
    }
}

#[test]
fn direct_hosts_preserve_file_picker_metadata_events_and_literal_data() {
    let label = "Choose {file}\"\\\nNext";
    let action = "files.pick\"\\{key}";
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"filePicker", "label":label, "accept":["image/png", ".txt"], "multiple":true, "onPick":action,
        "style":{"id":"upload","formName":"attachment","ariaLabel":"Pick file","required":true,"classes":["rounded","p-2"],"formValue":"never initialize files"}
    }]})).unwrap();
    for output in [
        emit_moonshine_dom_component(&ir, "Files").unwrap(),
        emit_moonshine_ssr_component(&ir, "Files").unwrap(),
    ] {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut elements = Elements::default();
        elements.visit_program(&parsed.program);
        let input = &elements
            .nodes
            .iter()
            .find(|(tag, _)| tag == "input")
            .unwrap()
            .1;
        let wrapper = &elements
            .nodes
            .iter()
            .find(|(tag, _)| tag == "label")
            .unwrap()
            .1;
        for key in [
            "type",
            "accept",
            "multiple",
            "id",
            "name",
            "aria-label",
            "required",
            "onChange",
        ] {
            assert_eq!(
                input.iter().filter(|actual| actual.as_str() == key).count(),
                1,
                "{key}: {output}"
            );
        }
        assert_eq!(wrapper, &["class".to_string()]);
        for key in ["class", "value", "defaultValue"] {
            assert!(!input.iter().any(|actual| actual == key), "{output}");
        }
        assert!(output.contains("image/png,.txt"), "{output}");
        assert!(
            output.contains(&serde_json::to_string(label).unwrap()),
            "{output}"
        );
        assert!(
            output.contains(&format!(
                "__crepusOwnHandler(handlers(), {})",
                serde_json::to_string(action).unwrap()
            )),
            "{output}"
        );
        assert!(output.contains("(event: Event)"), "{output}");
        assert!(output.contains("?.(event)"), "{output}");
        assert!(!output.contains("never initialize files"));
    }
}

#[test]
fn optional_file_picker_attributes_are_omitted_for_old_ir_defaults() {
    let ir: ViewIr =
        serde_json::from_value(json!({"version":7,"root":[{"kind":"filePicker","label":"Pick"}]}))
            .unwrap();
    for output in [
        emit_moonshine_dom_component(&ir, "Files").unwrap(),
        emit_moonshine_ssr_component(&ir, "Files").unwrap(),
    ] {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty());
        let mut elements = Elements::default();
        elements.visit_program(&parsed.program);
        let input = &elements
            .nodes
            .iter()
            .find(|(tag, _)| tag == "input")
            .unwrap()
            .1;
        assert_eq!(input, &["type".to_string()], "{output}");
    }
}
