use crepuscularity_native::{emit_imba_component, ImbaComponent, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> ImbaComponent {
    let ir: ViewIr = serde_json::from_value(json!({"version": 7, "root": root})).unwrap();
    emit_imba_component(&ir).unwrap()
}

#[test]
fn valued_textareas_install_the_owned_server_serialization_hook() {
    let component = emit(json!([
        {"kind":"input", "multiline":true, "placeholder":"", "bind":"body"},
        {"kind":"input", "multiline":true, "placeholder":"", "bind":"", "style":{"formValue":"DEFAULT"}}
    ]));

    assert_eq!(
        component
            .source
            .matches("__crepusTextareaOwner=(self)")
            .count(),
        2
    );
    assert_eq!(
        component
            .source
            .matches("bindings.syncSSRTextareas(self)")
            .count(),
        1
    );
    assert!(component.source.contains("$node$"));
    assert!(component
        .bindings
        .contains("export function syncSSRTextareas("));
    assert!(component.source.contains("value=(bindings."));
    assert!(component.source.contains("defaultValue=(\"DEFAULT\")"));
}

#[test]
fn ordinary_inputs_and_unvalued_textareas_do_not_install_server_textarea_work() {
    let component = emit(json!([
        {"kind":"input", "placeholder":"", "bind":"name"},
        {"kind":"input", "placeholder":"", "bind":"", "style":{"formValue":"INPUT"}},
        {"kind":"input", "multiline":true, "placeholder":"", "bind":""}
    ]));

    assert!(!component.source.contains("__crepusTextareaOwner"));
    assert!(!component.source.contains("syncSSRTextareas"));
    assert!(!component.bindings.contains("syncSSRTextareas"));
}

#[test]
fn hostile_textarea_defaults_remain_property_data_without_raw_children() {
    let value = "\n</textarea><script>alert(1)</script>&amp;\r";
    let component = emit(json!([
        {"kind":"input", "multiline":true, "placeholder":"", "bind":"", "style":{"formValue":value}}
    ]));

    assert!(component.source.contains(&format!(
        "defaultValue=({})",
        serde_json::to_string(value).unwrap()
    )));
    let textarea = component
        .source
        .lines()
        .find(|line| line.contains("<textarea"))
        .unwrap();
    assert!(textarea.ends_with('>'), "{textarea}");
    assert_eq!(component.source.matches("</textarea>").count(), 1);
    assert!(!component.source.contains("> String("));
    assert!(!component.source.contains("> bindings."));
}
