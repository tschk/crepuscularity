use crepuscularity_native::{emit_imba_component, ImbaComponent, ViewIr};
use serde_json::{json, Value};

fn emit(node: Value) -> ImbaComponent {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
    emit_imba_component(&ir).unwrap()
}

#[test]
fn file_picker_preserves_labels_accept_lists_and_exact_raw_event_handlers() {
    let output = emit(json!({
        "kind":"filePicker",
        "label":"Choose {file}\"\\\nNext",
        "accept":["image/png", ".txt", "quoted\"\\{kind}"],
        "multiple":true,
        "onPick":"files.pick\"\\{key}"
    }));
    let input = output
        .source
        .lines()
        .find(|line| line.contains("<input"))
        .unwrap();
    assert!(input.contains("type=(\"file\")"), "{input}");
    assert!(
        input.contains(r#"accept=("image/png,.txt,quoted\"\\\u007bkind\u007d")"#),
        "{input}"
    );
    assert!(input.contains("multiple=(true)"), "{input}");
    assert!(
        input
            .contains(r#"@change=(bindings.dispatch(handlers,"files.pick\"\\\u007bkey\u007d",e))"#),
        "{input}"
    );
    assert!(
        output
            .source
            .contains(r#""Choose \u007bfile\u007d\"\\\nNext""#),
        "{}",
        output.source
    );
    assert!(!input.contains(" value="), "{input}");
    assert!(!input.contains(" defaultValue="), "{input}");
    assert!(output.bindings.contains("handler.apply(handlers, args)"));
}

#[test]
fn composite_file_picker_places_identity_on_input_and_classes_on_wrapper() {
    let output = emit(json!({
        "kind":"filePicker", "label":"Upload", "accept":[], "multiple":false,
        "style":{
            "id":"upload", "formName":"attachment", "ariaLabel":"Upload file",
            "required":true, "classes":["rounded","p-2"], "formValue":"must not reach file input"
        }
    }));
    let wrapper = output
        .source
        .lines()
        .find(|line| line.contains("<label"))
        .unwrap();
    let input = output
        .source
        .lines()
        .find(|line| line.contains("<input"))
        .unwrap();
    assert!(wrapper.contains("className=(\"rounded p-2\")"), "{wrapper}");
    for attribute in ["id=", "name=", "aria-label=", "required="] {
        assert!(!wrapper.contains(attribute), "{wrapper}");
    }
    for attribute in [
        "id=(\"upload\")",
        "name=(\"attachment\")",
        "aria-label=(\"Upload file\")",
        "required=(true)",
    ] {
        assert!(input.contains(attribute), "{input}");
    }
    assert!(!input.contains("className="), "{input}");
    assert!(!input.contains("multiple="), "{input}");
    assert!(!input.contains("accept="), "{input}");
    assert!(!input.contains(" value="), "{input}");
    assert!(!input.contains(" defaultValue="), "{input}");
    assert!(!output.source.contains("must not reach file input"));
    assert_eq!(output.source.matches("id=(\"upload\")").count(), 1);
}

#[test]
fn old_file_picker_defaults_omit_optional_accept_multiple_and_callback() {
    let output = emit(json!({"kind":"filePicker","label":"Pick"}));
    let input = output
        .source
        .lines()
        .find(|line| line.contains("<input"))
        .unwrap();
    assert!(input.contains("type=(\"file\")"), "{input}");
    for attribute in [
        "accept=",
        "multiple=",
        "@change=",
        " value=",
        " defaultValue=",
    ] {
        assert!(!input.contains(attribute), "{input}");
    }
}
