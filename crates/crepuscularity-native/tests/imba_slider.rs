use crepuscularity_native::{emit_imba_component, ViewIr, ViewNode, IR_VERSION};
use serde_json::json;

#[test]
fn imba_slider_uses_live_bound_value_and_exact_change_action() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"slider","label":"Volume {literal}","bind":"prefs.volume","value":5,"min":0,"max":10,"step":0.5,
        "onChange":"prefs.volume.changed","style":{"classes":["w-full"]}
    }]})).unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(output.source.contains("type=(\"range\")"));
    assert!(output
        .source
        .contains("aria-label=(\"Volume \\u007bliteral\\u007d\")"));
    assert!(output
        .source
        .contains("name=(\"prefs.volume\") className=(\"w-full\")"));
    assert!(output
        .source
        .contains("min=(0) max=(10) step=(0.5) value=(bindings.e0(scope))"));
    assert!(output
        .source
        .contains("@change=(bindings.dispatch(handlers,\"prefs.volume.changed\",e))"));
    assert!(output.bindings.contains("scope?.prefs?.volume"));
}

#[test]
fn imba_slider_normalizes_snapshot_and_range_and_keeps_continuous_step() {
    for (value, min, max, step, expected_range, expected_value) in [
        (
            20.0,
            0.0,
            10.0,
            None,
            "min=(0) max=(10) step=(\"any\")",
            "10",
        ),
        (
            5.0,
            10.0,
            0.0,
            Some(-1.0),
            "min=(0) max=(100) step=(\"any\")",
            "0",
        ),
        (
            f32::NAN,
            2.0,
            8.0,
            Some(f32::INFINITY),
            "min=(2) max=(8) step=(\"any\")",
            "2",
        ),
        (
            5.0,
            5.0,
            5.0,
            Some(1.0),
            "min=(5) max=(5) step=(\"any\")",
            "5",
        ),
    ] {
        let output = emit_imba_component(&ViewIr {
            version: IR_VERSION,
            root: vec![ViewNode::Slider {
                label: None,
                bind: None,
                value,
                min,
                max,
                step,
                on_change: None,
                style: None,
            }],
        })
        .unwrap();
        assert!(output.source.contains(expected_range), "{}", output.source);
        assert!(
            output
                .bindings
                .contains(&format!("return {expected_value};")),
            "{}",
            output.bindings
        );
        assert!(!output.source.contains("aria-label="));
    }
}

#[test]
fn imba_slider_binding_preserves_loop_scope() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"forEach","bind":"rows","itemName":"scope","itemBody":[{
            "kind":"slider","bind":"scope.value","value":0,"min":0,"max":1
        }]
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(output
        .source
        .contains("value=(bindings.e1(scope,_crepusItem0))"));
    assert!(output.bindings.contains("_crepusItem0?.value"));
}

#[test]
fn imba_unbound_slider_snapshots_are_form_defaults() {
    for bind in [None, Some(""), Some("  "), Some("volume")] {
        let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
            "kind":"slider","bind":bind,"value":3,"min":0,"max":10,"step":1
        }]}))
        .unwrap();
        let output = emit_imba_component(&ir).unwrap();
        let unbound = bind.is_none_or(|value| value.trim().is_empty());
        assert_eq!(
            output.source.contains(" defaultValue="),
            unbound,
            "{}",
            output.source
        );
        assert_eq!(
            output.source.contains(" value="),
            !unbound,
            "{}",
            output.source
        );
    }
}
