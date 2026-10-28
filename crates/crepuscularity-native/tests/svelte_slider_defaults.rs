use crepuscularity_native::{emit_svelte_component, ViewIr};
use serde_json::json;

#[test]
fn unbound_svelte_sliders_keep_the_normalized_initial_value_as_reset_default() {
    for bind in [None, Some(""), Some("  ")] {
        for (value, min, max) in [(23, 10, 90), (150, 100, 200)] {
            let mut node = json!({"kind":"slider","value":value,"min":min,"max":max,"step":1});
            if let Some(bind) = bind {
                node["bind"] = bind.into();
            }
            let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
            let output = emit_svelte_component(&ir);
            assert!(output.contains(&format!("value={{{value}}}")), "{output}");
            assert!(
                output.contains(&format!("defaultValue={{{value}}}")),
                "{output}"
            );
            assert!(output.contains(&format!("min={{{min}}}")), "{output}");
            assert!(output.contains(&format!("max={{{max}}}")), "{output}");
            let bounds = output.find(&format!("max={{{max}}}")).unwrap();
            let value_at = output.find(&format!(" value={{{value}}}")).unwrap();
            assert!(
                bounds < value_at,
                "bounds must precede value initialization: {output}"
            );
        }
    }
}

#[test]
fn bound_svelte_sliders_keep_live_values_without_an_unbound_reset_default() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"slider","bind":"prefs.level","value":23,"min":10,"max":90,"step":1
    }]}))
    .unwrap();
    let output = emit_svelte_component(&ir);
    assert!(output.contains("prefs?.level"), "{output}");
    assert!(output.contains(" value={"), "{output}");
    assert!(!output.contains("defaultValue="), "{output}");
}
