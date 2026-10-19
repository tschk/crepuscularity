use crepuscularity_native::{
    emit_moonshine_dom_component, emit_moonshine_ssr_component, render_template_to_ir, ViewIr,
    ViewNode, IR_VERSION,
};

fn outputs(ir: &ViewIr) -> [String; 2] {
    [
        emit_moonshine_dom_component(ir, "Controls").unwrap(),
        emit_moonshine_ssr_component(ir, "Controls").unwrap(),
    ]
}

fn document(node: ViewNode) -> ViewIr {
    ViewIr {
        version: IR_VERSION,
        root: vec![node],
    }
}

#[test]
fn picker_keeps_options_live_selection_and_exact_change_handler() {
    let ir = render_template_to_ir(
        "picker bind=prefs.choice @change=prefs.choose\n  option value=\"one\" \"One\"\n  option value=\"two\" \"Two & {literal}\"",
        &Default::default(),
    ).unwrap();
    for output in outputs(&ir) {
        assert!(output.contains("h(\"select\""), "{output}");
        assert_eq!(output.matches("h(\"option\"").count(), 2, "{output}");
        assert!(
            output.contains("value: () => (scope()?.prefs?.choice ?? \"\")"),
            "{output}"
        );
        assert!(output.contains("value: \"one\""), "{output}");
        assert!(output.contains("value: \"two\""), "{output}");
        assert!(
            output.contains("__crepusOwnHandler(handlers(), \"prefs.choose\")"),
            "{output}"
        );
        assert!(output.contains("?.[\"prefs.choose\"]?.(event)"), "{output}");
        assert!(output.contains("onChange"), "{output}");
    }
}

#[test]
fn sliders_use_safe_live_values_labels_and_uncontrolled_snapshots() {
    let bound = document(ViewNode::Slider {
        label: Some("Level {literal} & \"quoted\"".into()),
        bind: Some("prefs.level".into()),
        value: f32::NAN,
        min: 10.0,
        max: -10.0,
        step: Some(f32::INFINITY),
        on_change: Some("prefs.slide".into()),
        style: None,
    });
    for output in outputs(&bound) {
        assert!(
            output.contains("__crepusSliderValue(scope()?.prefs?.level, 0, 100, 0)"),
            "{output}"
        );
        assert!(
            output.contains("\"aria-label\": \"Level {literal} & \\\"quoted\\\"\""),
            "{output}"
        );
        assert!(output.contains("step: \"any\""), "{output}");
        assert!(
            !output.contains("NaN") && !output.contains("Infinity"),
            "{output}"
        );
        assert!(
            output.contains("__crepusOwnHandler(handlers(), \"prefs.slide\")"),
            "{output}"
        );
    }
    let unbound = document(ViewNode::Slider {
        label: None,
        bind: None,
        value: 5.0,
        min: 0.0,
        max: 10.0,
        step: Some(0.5),
        on_change: None,
        style: None,
    });
    for output in outputs(&unbound) {
        assert!(output.contains("defaultValue: 5"), "{output}");
        assert!(output.contains("step: 0.5"), "{output}");
        assert!(!output.contains("value: () =>"), "{output}");
    }
}

#[test]
fn indicators_normalize_invalid_values_but_keep_valid_meter_scale() {
    for node in [
        ViewNode::Progress {
            label: Some("Progress {literal}".into()),
            value: f32::NAN,
            max: f32::INFINITY,
            style: None,
        },
        ViewNode::Meter {
            label: Some("Meter {literal}".into()),
            value: f32::NAN,
            min: 5.0,
            max: -5.0,
            style: None,
        },
    ] {
        for output in outputs(&document(node)) {
            assert!(
                !output.contains("NaN") && !output.contains("Infinity"),
                "{output}"
            );
            assert!(output.contains("max: 1"), "{output}");
            assert!(output.contains("value: 0"), "{output}");
            assert!(output.contains("\"aria-label\":"), "{output}");
            assert!(output.contains("{literal}"), "{output}");
        }
    }
    let meter = document(ViewNode::Meter {
        label: None,
        value: -5.0,
        min: -10.0,
        max: 10.0,
        style: None,
    });
    for output in outputs(&meter) {
        for expected in ["h(\"meter\"", "min: -10", "max: 10", "value: -5"] {
            assert!(output.contains(expected), "{output}");
        }
        assert!(
            output.find("max: 10").unwrap() < output.find("value: -5").unwrap(),
            "{output}"
        );
    }
}

#[test]
fn loop_picker_and_slider_bindings_keep_shadowed_scope_items_local() {
    let ir = render_template_to_ir(
        "for scope in rows\n  picker bind=scope.choice @change=choose\n    option value=\"one\" \"One\"\n  slider bind=scope.level value=5 min=0 max=10 @change=slide",
        &Default::default(),
    ).unwrap();
    for output in outputs(&ir) {
        assert!(output.contains("each: () => toArray("), "{output}");
        assert!(
            output.contains("globalThis.Array.isArray(value)"),
            "{output}"
        );
        assert!(
            output.contains("value: () => (_crepusItem0?.choice ?? \"\")"),
            "{output}"
        );
        assert!(
            output.contains("__crepusSliderValue(_crepusItem0?.level, 0, 10, 5)"),
            "{output}"
        );
        assert!(output.contains("function __crepusSliderValue("), "{output}");
        for forbidden in [
            "scope()?.scope",
            "scope().scope",
            "scope()?.choice",
            "scope()?.level",
        ] {
            assert!(!output.contains(forbidden), "{forbidden}: {output}");
        }
    }
}
