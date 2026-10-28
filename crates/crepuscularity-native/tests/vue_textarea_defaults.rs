use crepuscularity_native::{emit_vue_component, ViewIr, IR_VERSION};
use serde_json::json;

fn emit(value: &str, bind: &str, multiline: bool) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":IR_VERSION,"root":[{
        "kind":"input","placeholder":"Hint","bind":bind,"multiline":multiline,"secure":false,
        "style":{"formValue":value,"id":"field","classes":["mark"]}
    }]}))
    .unwrap();
    emit_vue_component(&ir)
}

#[test]
fn initial_newline_textarea_uses_native_directive_server_compensation() {
    let out = emit("\nseed<&{literal}", "", true);
    assert!(out.contains("v-crepus-textarea-default="), "{out}");
    assert!(
        out.contains("beforeMount(node: HTMLTextAreaElement"),
        "{out}"
    );
    assert!(out.contains("node.defaultValue = binding.value;"), "{out}");
    assert!(
        out.contains("getSSRProps(binding: { value: string })"),
        "{out}"
    );
    assert!(
        out.contains("return { value: \"\\n\" + binding.value };"),
        "{out}"
    );
    assert!(!out.contains("{{ \"\\nseed"), "{out}");
    assert!(
        out.contains("id=\"field\"") && out.contains("class=\"mark\""),
        "{out}"
    );
    assert_eq!(out.matches("const vCrepusTextareaDefault =").count(), 1);
}

#[test]
fn ordinary_defaults_inputs_and_bound_textareas_keep_existing_paths() {
    for out in [
        emit("seed", "", true),
        emit("", "", true),
        emit("\rseed", "", true),
        emit("\nseed", "value", true),
        emit("\nseed", "", false),
    ] {
        assert!(!out.contains("vCrepusTextareaDefault"), "{out}");
        assert!(!out.contains("v-crepus-textarea-default="), "{out}");
    }
    assert!(emit("\nseed", "value", true).contains(":value="));
}
