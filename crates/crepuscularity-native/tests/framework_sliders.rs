use crepuscularity_native::{
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component, ViewIr,
    ViewNode, IR_VERSION,
};

#[test]
fn web_sliders_sanitize_invalid_direct_ir_before_emitting_javascript() {
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Slider {
            label: None,
            bind: Some("level".into()),
            value: f32::NAN,
            min: 10.0,
            max: -10.0,
            step: Some(f32::INFINITY),
            on_change: None,
            style: None,
        }],
    };
    for output in [
        emit_react_component(&ir, "Slider"),
        emit_solid_component(&ir, "Slider"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(!output.contains("NaN"), "{output}");
        assert!(!output.contains("Infinity"), "{output}");
        assert!(output.contains("__crepusSliderValue(scope"), "{output}");
        assert!(output.contains(", 0, 100, 0)"), "{output}");
        assert!(output.contains("step=\"any\""), "{output}");
    }
}

#[test]
fn component_names_cannot_shadow_generated_slider_helpers() {
    let ir = crepuscularity_native::render_template_to_ir(
        "slider bind=level value=5 min=0 max=10",
        &Default::default(),
    )
    .unwrap();
    for output in [
        emit_react_component(&ir, "__crepusSliderValue"),
        emit_solid_component(&ir, "__crepusSliderValue"),
    ] {
        assert!(
            output.contains("export { __CrepusComponent as __crepusSliderValue };"),
            "{output}"
        );
        assert_eq!(
            output.matches("function __crepusSliderValue(").count(),
            1,
            "{output}"
        );
    }
}
