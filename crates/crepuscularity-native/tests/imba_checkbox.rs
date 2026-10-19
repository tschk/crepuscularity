use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::json;

#[test]
fn imba_unbound_checkbox_snapshots_are_defaults_and_bound_values_stay_live() {
    for bind in [None, Some(""), Some("  "), Some("prefs.enabled")] {
        for checked in [false, true] {
            let ir:ViewIr=serde_json::from_value(json!({"version":7,"root":[{"kind":"checkbox","label":"Keep choice","bind":bind,"checked":checked,"onChange":"prefs.change"}]})).unwrap();
            let output = emit_imba_component(&ir).unwrap();
            let input = output
                .source
                .lines()
                .find(|line| line.contains("<input"))
                .unwrap();
            let unbound = bind.is_none_or(|value| value.trim().is_empty());
            assert_eq!(input.contains(" defaultChecked="), unbound, "{input}");
            assert_eq!(input.contains(" checked="), !unbound, "{input}");
            assert!(
                input.contains("bindings.dispatch(handlers,\"prefs.change\",e)"),
                "{input}"
            );
        }
    }
}
