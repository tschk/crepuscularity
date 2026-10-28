use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

#[test]
fn loop_bindings_preserve_scalar_types_and_object_scopes() {
    for (value, expression, expected) in [
        (Some(TemplateValue::Int(2)), "item + 1", "3"),
        (Some(TemplateValue::Bool(false)), "item == false", "true"),
        (
            Some(TemplateValue::Str(String::new())),
            "item == ''",
            "true",
        ),
        (Some(TemplateValue::Null), "item == null", "true"),
        (None, "item.name", "Object"),
    ] {
        let mut item = TemplateContext::new();
        item.set("name", "Object");
        if let Some(value) = value {
            item.set("value", value);
        }
        let source = format!("for item in items\n  span \"Result:{{{expression}}}:End\"");
        let mut template = Template::from_source(source);
        template.set("item", "parent must not leak");
        template.set("items", TemplateValue::List(vec![item]));
        let mut terminal = Terminal::new(TestBackend::new(60, 3)).unwrap();
        terminal
            .draw(|frame| template.draw_full(frame).unwrap())
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            output.contains(&format!("Result:{expected}:End")),
            "{expression}: {output}"
        );
    }
}

#[test]
fn loop_scope_restores_parent_fields_between_different_item_shapes() {
    let mut items = Vec::new();
    for (value, name) in [(0, Some("First")), (1, None), (2, Some("Third"))] {
        let mut item = TemplateContext::new();
        item.set("value", value);
        if let Some(name) = name {
            item.set("name", name);
        }
        items.push(item);
    }
    let mut template = Template::from_source(
        "div flex-col\n  for item in items\n    span \"[{name}:{item + 1}]\"",
    );
    template.set("name", "Parent");
    template.set("items", TemplateValue::List(items));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    for expected in ["[First:1]", "[Parent:2]", "[Third:3]"] {
        assert!(output.contains(expected), "missing {expected}: {output}");
    }
}

#[test]
fn dotted_iterator_lists_render_and_discover_focus() {
    let mut item = TemplateContext::new();
    item.set("value", "entry");
    let mut data = TemplateContext::new();
    data.set("items", TemplateValue::List(vec![item]));
    let mut template = Template::from_source(
        "for item in data.items\n  button id={item} focusable\n    \"Visible {item}\"",
    );
    template.set("data", TemplateValue::Scope(data));
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["entry"]);
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Visible entry"), "{output}");
}
