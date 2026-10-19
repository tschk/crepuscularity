use crepuscularity_native::{emit_solid_component, ViewIr, IR_VERSION};
use serde_json::json;

fn emit(value: &str, bind: &str, multiline: bool, name: &str) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":IR_VERSION,"root":[{
        "kind":"input","placeholder":"Hint","bind":bind,"multiline":multiline,"secure":false,
        "style":{"formValue":value,"id":"field","classes":["mark"]}
    }]}))
    .unwrap();
    emit_solid_component(&ir, name)
}

#[test]
fn unbound_textarea_keeps_dom_default_and_escaped_server_body() {
    let out = emit("<&</textarea>&amp;\r\n", "", true, "Panel");
    assert!(
        out.contains("prop:defaultValue={\"<&</textarea>&amp;\\r\\n\"}"),
        "{out}"
    );
    assert!(out.contains("__crepusTextareaServer ? (__crepusTextareaSsr(\"&lt;&amp;&lt;/textarea&gt;&amp;amp;&#13;\\n\") as unknown as import(\"solid-js\").JSX.Element) : undefined"), "{out}");
    assert!(
        out.contains("id=\"field\"") && out.contains("class=\"mark\""),
        "{out}"
    );
    assert_eq!(out.matches("ssr as __crepusTextareaSsr").count(), 1);
    for value in ["", "seed", "\nseed", "\rseed"] {
        let out = emit(value, " ", true, "Panel");
        assert!(out.contains("__crepusTextareaServer ?"), "{out}");
    }
    assert!(emit("\nseed", "", true, "Panel").contains("__crepusTextareaSsr(\"\\n\\nseed\")"));
    assert!(emit("\rseed", "", true, "Panel").contains("__crepusTextareaSsr(\"&#13;seed\")"));
}

#[test]
fn bound_textarea_has_server_text_and_keeps_client_value_live() {
    let bound = emit("ignored default", "value", true, "Panel");
    assert!(
        bound.contains("value={__crepusTextareaServer ? undefined :"),
        "{bound}"
    );
    assert!(
        bound.contains("__crepusTextareaSsr(__crepusTextareaHtml("),
        "{bound}"
    );
    assert!(
        bound.contains("const text = `${value ?? \"\"}`;"),
        "{bound}"
    );
    assert!(!bound.contains("prop:defaultValue="), "{bound}");
    assert!(!bound.contains("ignored default"), "{bound}");
    assert_eq!(bound.matches("function __crepusTextareaHtml(").count(), 1);
    let input = emit("seed", "value", false, "Panel");
    assert!(!input.contains("__crepusTextarea"), "{input}");
    assert!(input.contains(" value={"), "{input}");
}

#[test]
fn component_names_cannot_shadow_server_textarea_imports() {
    for name in [
        "__crepusTextareaServer",
        "__crepusTextareaSsr",
        "__crepusTextareaHtml",
    ] {
        let out = emit("seed", "", true, name);
        assert!(out.contains("function __CrepusComponent("), "{out}");
        assert!(!out.contains(&format!("function {name}(")), "{out}");
    }
}

#[test]
fn bound_textarea_inside_loop_preserves_local_binding_rewrite() {
    let ir: ViewIr = serde_json::from_value(json!({"version":IR_VERSION,"root":[{
        "kind":"forEach","bind":"rows","itemName":"row","itemBody":[{
            "kind":"input","placeholder":"","bind":"row.text","multiline":true,"secure":false
        }]
    }]}))
    .unwrap();
    let out = emit_solid_component(&ir, "Panel");
    assert!(
        out.contains("__crepusTextareaSsr(__crepusTextareaHtml("),
        "{out}"
    );
    assert!(
        out.contains(r#"__crepusTextareaHtml((_crepusItem0?.text ?? ""))"#),
        "{out}"
    );
    assert!(out.contains("_crepusItem0"), "{out}");
    assert_eq!(out.matches("function __crepusTextareaHtml(").count(), 1);
}
