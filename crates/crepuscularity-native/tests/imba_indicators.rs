use crepuscularity_native::{emit_imba_component, ViewIr, ViewNode, ViewStyle, IR_VERSION};

fn emit(node: ViewNode) -> String {
    emit_imba_component(&ViewIr {
        version: IR_VERSION,
        root: vec![node],
    })
    .unwrap()
    .source
}

#[test]
fn progress_normalizes_values_and_sets_max_before_value() {
    for (value, max, attrs) in [
        (0.4, 1.0, "max=(1) value=(0.4)"),
        (5.0, 2.0, "max=(2) value=(2)"),
        (-1.0, 2.0, "max=(2) value=(0)"),
        (f32::NAN, 2.0, "max=(2) value=(0)"),
        (0.5, f32::INFINITY, "max=(1) value=(0)"),
        (0.5, 0.0, "max=(1) value=(0)"),
    ] {
        let source = emit(ViewNode::Progress {
            label: Some("Load {literal}".into()),
            value,
            max,
            style: None,
        });
        assert!(
            source.contains("<progress aria-label=(\"Load \\u007bliteral\\u007d\")"),
            "{source}"
        );
        assert!(source.contains(attrs), "{source}");
    }
}

#[test]
fn meter_normalizes_values_and_sets_range_before_value() {
    for (value, min, max, attrs) in [
        (0.4, 0.0, 1.0, "min=(0) max=(1) value=(0.4)"),
        (12.0, 2.0, 8.0, "min=(2) max=(8) value=(8)"),
        (-1.0, 2.0, 8.0, "min=(2) max=(8) value=(2)"),
        (f32::NAN, 2.0, 8.0, "min=(2) max=(8) value=(2)"),
        (1.0, 8.0, 2.0, "min=(0) max=(1) value=(0)"),
        (1.0, f32::NEG_INFINITY, 2.0, "min=(0) max=(1) value=(0)"),
        (4.0, 5.0, 5.0, "min=(0) max=(1) value=(0)"),
        (5.0, 5.0, 5.0, "min=(0) max=(1) value=(1)"),
    ] {
        let source = emit(ViewNode::Meter {
            label: Some("Capacity".into()),
            value,
            min,
            max,
            style: None,
        });
        assert!(
            source.contains("<meter aria-label=(\"Capacity\")"),
            "{source}"
        );
        assert!(source.contains(attrs), "{source}");
    }
}

#[test]
fn indicators_preserve_classes_and_optional_labels() {
    let style = Some(ViewStyle {
        classes: vec!["w-full".into(), "rounded".into()],
        ..Default::default()
    });
    for node in [
        ViewNode::Progress {
            label: None,
            value: 0.5,
            max: 1.0,
            style: style.clone(),
        },
        ViewNode::Meter {
            label: None,
            value: 0.5,
            min: 0.0,
            max: 1.0,
            style,
        },
    ] {
        let source = emit(node);
        assert!(source.contains("className=(\"w-full rounded\")"));
        assert!(!source.contains("aria-label="));
    }
}
