use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::{json, Value};

fn emit(root: Value) -> crepuscularity_native::ImbaComponent {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":root})).unwrap();
    emit_imba_component(&ir).unwrap()
}

#[test]
fn bound_picker_applies_host_value_after_options_exist() {
    let output = emit(json!([{
        "kind":"picker", "bind":"choice", "style":{"classes":["rounded","px-2"]}, "onChange":"prefs.choice",
        "options":[{"value":"first","label":"First"},{"value":"second","label":"Second"}]
    }]));
    assert!(output.source.contains("<select name=(\"choice\") className=(\"rounded px-2\") @change=(bindings.dispatch(handlers,\"prefs.choice\",e)) data-crepus-picker=(\"\") __crepusPickerOwner=(self) __crepusPickerValue=(bindings.e0(scope))>"));
    for (value, label) in [("first", "First"), ("second", "Second")] {
        assert!(
            output.source.lines().any(|line| line
                .contains(&format!("<option value=(\"{value}\") html:selected=("))
                && line.ends_with(&format!("> \"{label}\""))),
            "{}",
            output.source
        );
    }
    assert!(output
        .bindings
        .contains("globalThis.String((scope?.choice ?? \"\"))"));
    assert!(output
        .bindings
        .contains("field.__crepusPickerOwner !== root"));
    assert!(output
        .bindings
        .contains("pickerValues.get(field) !== value"));
}

#[test]
fn unbound_picker_is_editable_and_literals_are_escaped() {
    let output = emit(json!([{
        "kind":"picker", "bind":"", "options":[{"value":"{secret}\"\\","label":"Literal {name}\nNext"}]
    }]));
    assert!(!output.source.contains(" selected="));
    assert!(!output.source.contains(" value=(bindings."));
    assert!(output
        .source
        .contains(r#"value=("\u007bsecret\u007d\"\\")"#));
    assert!(output
        .source
        .contains(r#"> "Literal \u007bname\u007d\nNext""#));
    assert!(!output.bindings.contains("export function e0"));
    assert!(!output.bindings.contains("syncPickers"));
    assert!(!output.source.contains("data-crepus-picker"));
}

#[test]
fn picker_bindings_keep_hygienic_loop_scope() {
    let output = emit(json!([{
        "kind":"forEach", "bind":"rows", "itemName":"scope", "itemBody":[{
            "kind":"picker", "bind":"scope.choice", "options":[{"value":"second","label":"Second"}]
        }]
    }]));
    assert!(output
        .source
        .contains("__crepusPickerValue=(bindings.e1(scope,_crepusItem0))"));
    assert!(output
        .bindings
        .contains("export function e1(scope: any, _crepusItem0: any)"));
    assert!(output
        .bindings
        .contains("globalThis.String((_crepusItem0?.choice ?? \"\"))"));
}
