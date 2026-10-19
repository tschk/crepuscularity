use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_component, emit_moonshine_dom_component,
    emit_moonshine_ssr_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir, NativeCodegenTarget, ViewIr,
};

fn scroll_ir() -> ViewIr {
    render_template_to_ir(
        "div id=layout class=\"flex flex-row gap-3 items-center justify-between overflow-x-auto\"\n  span \"First\"\n  span \"Second\"",
        &TemplateContext::new(),
    ).unwrap()
}

#[test]
fn synthetic_scroll_layout_wrapper_is_transparent_in_every_web_emitter() {
    let ir = scroll_ir();
    let value = serde_json::to_value(&ir).unwrap();
    assert_eq!(value["root"][0]["kind"], "scroll");
    assert!(value["root"][0]["style"]
        .get("htmlDisplayContents")
        .is_none());
    assert_eq!(
        value["root"][0]["children"][0]["style"]["htmlDisplayContents"],
        true
    );
    for (name, source, display) in [
        (
            "react",
            emit_moonshine_component(&ir, "Scroll"),
            "style={{display:\"contents\"}}",
        ),
        (
            "solid",
            emit_solid_component(&ir, "Scroll"),
            "style={{display:\"contents\"}}",
        ),
        (
            "svelte",
            emit_svelte_component(&ir),
            "style=\"display: contents;\"",
        ),
        (
            "vue",
            emit_vue_component(&ir),
            "style=\"display: contents;\"",
        ),
        (
            "imba",
            emit_imba_component(&ir).unwrap().source,
            "style=(\"display: contents;\")",
        ),
        (
            "dom",
            emit_moonshine_dom_component(&ir, "Scroll").unwrap(),
            "style: {display: \"contents\"}",
        ),
        (
            "ssr",
            emit_moonshine_ssr_component(&ir, "Scroll").unwrap(),
            "style: {display: \"contents\"}",
        ),
    ] {
        assert!(source.contains(display), "{name}: {source}");
        assert_eq!(
            source.matches("overflow-x-auto").count(),
            1,
            "{name}: {source}"
        );
        assert_eq!(source.matches("layout").count(), 1, "{name}: {source}");
    }
}

#[test]
fn web_wrapper_metadata_leaves_native_layout_source_unchanged() {
    let ir = scroll_ir();
    let mut previous = serde_json::to_value(&ir).unwrap();
    previous["root"][0]["children"][0]
        .as_object_mut()
        .unwrap()
        .remove("style");
    let previous: ViewIr = serde_json::from_value(previous).unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&ir, target, "Scroll"),
            generate_native_source(&previous, target, "Scroll")
        );
    }
    let ordinary = render_template_to_ir(
        "div class=\"flex flex-row gap-3\"\n span \"First\"",
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(!serde_json::to_string(&ordinary)
        .unwrap()
        .contains("htmlDisplayContents"));
}

#[test]
fn scroll_viewport_axis_does_not_change_the_content_stack_axis() {
    for (classes, viewport, content) in [
        ("flex flex-col overflow-x-auto", "row", "column"),
        ("flex flex-row overflow-y-auto", "column", "row"),
        ("flex flex-row overflow-auto", "row", "row"),
        ("flex flex-col overflow-auto", "column", "column"),
        (
            "flex flex-row overflow-x-auto overflow-y-auto",
            "row",
            "row",
        ),
    ] {
        let source = format!("div class=\"{classes}\"\n span \"First\"\n span \"Second\"");
        let ir = render_template_to_ir(&source, &TemplateContext::new()).unwrap();
        let value = serde_json::to_value(&ir).unwrap();
        assert_eq!(value["root"][0]["kind"], "scroll", "{classes}");
        assert_eq!(value["root"][0]["axis"], viewport, "{classes}");
        assert_eq!(
            value["root"][0]["children"][0]["axis"], content,
            "{classes}"
        );
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "Scroll");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "Scroll");
        let (swift_axis, compose_axis) = if viewport == "row" {
            ("ScrollView(.horizontal)", "horizontalScroll")
        } else {
            ("ScrollView(.vertical)", "verticalScroll")
        };
        assert!(swift.contains(swift_axis), "{swift}");
        assert!(compose.contains(compose_axis), "{compose}");
    }
}

#[test]
fn scroll_long_press_survives_lowering_emission_and_action_updates() {
    use crepuscularity_native::{apply_mutations, diff_ir, IrMutation};
    let make = |action: &str| {
        render_template_to_ir(
            &format!("div class=\"overflow-auto\" {action}\n  span \"Content\""),
            &TemplateContext::new(),
        )
        .unwrap()
    };
    let ir = make("@longpress=hold");
    let json = serde_json::to_value(&ir).unwrap();
    assert_eq!(json["root"][0]["kind"], "scroll");
    assert_eq!(json["root"][0]["onLongPress"], "hold");
    assert!(json["root"][0]["children"][0].get("onLongPress").is_none());
    for source in [
        emit_moonshine_component(&ir, "Scroll"),
        emit_solid_component(&ir, "Scroll"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
        emit_imba_component(&ir).unwrap().source,
        emit_moonshine_dom_component(&ir, "Scroll").unwrap(),
        emit_moonshine_ssr_component(&ir, "Scroll").unwrap(),
    ] {
        assert_eq!(
            source.matches("data-crepus-on-long-press").count(),
            1,
            "{source}"
        );
        assert!(source.contains("hold"), "{source}");
    }
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "Scroll");
        assert!(source.contains("hold"), "{source}");
    }
    for next in [make("@longpress=other"), make("")] {
        let mutations = diff_ir(&ir, &next);
        assert!(matches!(
            mutations.as_slice(),
            [IrMutation::ReplaceNode { .. }]
        ));
        let mut updated = ir.clone();
        apply_mutations(&mut updated, &mutations).unwrap();
        assert_eq!(updated, next);
    }
    let mut legacy = serde_json::to_value(make("")).unwrap();
    legacy["root"][0]
        .as_object_mut()
        .unwrap()
        .remove("onLongPress");
    let legacy: ViewIr = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy, make(""));
}
