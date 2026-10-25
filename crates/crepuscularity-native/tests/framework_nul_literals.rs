use crepuscularity_native::{emit_svelte_component, emit_vue_component, ViewIr};
use serde_json::json;

fn fixture(root: serde_json::Value) -> ViewIr {
    serde_json::from_value(json!({"version": 7, "root": root})).unwrap()
}

#[test]
fn nul_text_and_button_labels_use_runtime_safe_expressions() {
    let ir = fixture(json!([
        {"kind": "text", "content": "text\0tail"},
        {"kind": "button", "label": "label\0tail"}
    ]));
    let svelte = emit_svelte_component(&ir);
    let vue = emit_vue_component(&ir);
    for source in [&svelte, &vue] {
        assert!(!source.contains('\0'), "raw NUL in generated source");
        assert!(source.contains(r"text\u0000tail"), "{source}");
        assert!(source.contains(r"label\u0000tail"), "{source}");
    }
    assert!(svelte.contains(r#"{"text\u0000tail"}"#), "{svelte}");
    // A constant Vue interpolation may be folded back into HTML by its compiler.
    assert!(vue.contains("__crepusText("), "{vue}");
    assert!(vue.contains("{{ __crepusText("), "{vue}");
}

#[test]
fn nul_attributes_are_dynamic_and_keep_their_json_escape() {
    let ir = fixture(json!([
        {"kind": "input", "bind": "", "placeholder": "hint\0tail",
         "style": {"id": "id\0tail", "ariaLabel": "aria\0tail"}}
    ]));
    let svelte = emit_svelte_component(&ir);
    let vue = emit_vue_component(&ir);
    for source in [&svelte, &vue] {
        assert!(!source.contains('\0'), "raw NUL in generated source");
        for literal in [r"hint\u0000tail", r"id\u0000tail", r"aria\u0000tail"] {
            assert!(source.contains(literal), "missing {literal}: {source}");
        }
    }
    for attribute in ["placeholder", "id", "aria-label"] {
        assert!(svelte.contains(&format!(" {attribute}={{")), "{svelte}");
        assert!(
            vue.contains(&format!(" :{attribute}=\"__crepusText(")),
            "{vue}"
        );
    }
}

#[test]
fn ordinary_static_literals_do_not_add_display_runtime() {
    let ir = fixture(json!([
        {"kind": "text", "content": "ordinary"},
        {"kind": "button", "label": "label"},
        {"kind": "input", "bind": "", "placeholder": "hint",
         "style": {"id": "identity", "ariaLabel": "description"}}
    ]));
    for source in [emit_svelte_component(&ir), emit_vue_component(&ir)] {
        assert!(!source.contains("__crepusText"), "{source}");
        assert!(source.contains("ordinary"));
        assert!(source.contains("placeholder=\"hint\""));
        assert!(source.contains("id=\"identity\""));
    }
}
