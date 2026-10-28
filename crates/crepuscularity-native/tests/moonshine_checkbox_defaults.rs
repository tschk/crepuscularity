use crepuscularity_native::{emit_moonshine_dom_component, emit_moonshine_ssr_component, ViewIr};
use serde_json::{json, Value};

fn outputs(kind: &str, bind: Option<&str>, checked: bool) -> Vec<String> {
    let mut node = json!({"kind":kind,"label":"State","checked":checked});
    if let Some(bind) = bind {
        node["bind"] = Value::String(bind.into());
    }
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
    vec![
        emit_moonshine_dom_component(&ir, "States").unwrap(),
        emit_moonshine_ssr_component(&ir, "States").unwrap(),
    ]
}

#[test]
fn unbound_checkboxes_use_literal_reset_defaults_in_both_hosts() {
    for bind in [None, Some(""), Some("  ")] {
        for checked in [true, false] {
            for output in outputs("checkbox", bind, checked) {
                assert!(
                    output.contains(&format!("defaultChecked: {checked}")),
                    "{output}"
                );
                assert!(!output.contains("defaultChecked: () =>"), "{output}");
                assert!(!output.contains("checked: () =>"), "{output}");
                assert_eq!(output.matches("defaultChecked:").count(), 1, "{output}");
            }
        }
    }
}

#[test]
fn bound_checkboxes_keep_their_live_checked_accessor() {
    for checked in [true, false] {
        for output in outputs("checkbox", Some("prefs.enabled"), checked) {
            assert!(output.contains("checked: () =>"), "{output}");
            assert!(output.contains("scope()?.prefs?.enabled"), "{output}");
            assert!(!output.contains("defaultChecked:"), "{output}");
        }
    }
}

#[test]
fn toggle_state_contract_is_unchanged_for_bound_and_unbound_nodes() {
    for bind in [None, Some(""), Some("  "), Some("prefs.enabled")] {
        for checked in [true, false] {
            for output in outputs("toggle", bind, checked) {
                assert!(output.contains("checked: () =>"), "{output}");
                assert!(output.contains("role: \"switch\""), "{output}");
                assert!(!output.contains("defaultChecked:"), "{output}");
            }
        }
    }
}
