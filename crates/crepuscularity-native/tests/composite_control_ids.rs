use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir_with_path, ViewIr, IR_VERSION,
};
use std::path::Path;

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Controls")),
        ("solid", emit_solid_component(ir, "Controls")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "Controls").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Controls").unwrap()),
    ]
}

fn assert_wrapped_input(target: &str, output: &str, encoded_id: &str) {
    let (wrapper, input) = if matches!(target, "dom" | "ssr") {
        let label = output.split_once("h(\"label\", ").unwrap().1;
        let (wrapper, input) = label.split_once("h(\"input\", ").unwrap();
        (wrapper, input)
    } else {
        let label = output.split_once("<label").unwrap().1;
        let (wrapper, input) = label.split_once("<input").unwrap();
        (wrapper, input)
    };
    assert!(
        wrapper.contains("font-bold"),
        "{target}: wrapper classes lost: {output}"
    );
    assert!(
        !wrapper.contains(encoded_id),
        "{target}: wrapper owns ID: {output}"
    );
    assert!(
        input.contains(encoded_id),
        "{target}: input ID absent: {output}"
    );
    assert_eq!(
        output.matches(encoded_id).count(),
        1,
        "{target}: duplicated ID: {output}"
    );
}

#[test]
fn checkbox_ids_belong_to_inputs_and_toggle_ids_to_controls() {
    for kind in ["checkbox", "toggle"] {
        let source = format!("<{kind} id='control' class='font-bold' label='Control'/>");
        let ir = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("view.jsx")),
        )
        .unwrap();
        for (target, output) in outputs(&ir) {
            let encoded_id = match target {
                "imba" => "id=(\"control\")",
                "dom" | "ssr" => "id: \"control\"",
                _ => "id=\"control\"",
            };
            if kind == "checkbox" || matches!(target, "dom" | "ssr") {
                assert_wrapped_input(target, &output, encoded_id);
            } else {
                let button = output.split_once("<button").unwrap().1;
                assert!(button.contains(encoded_id), "{target}: {output}");
                assert!(button.contains("font-bold"), "{target}: {output}");
                assert_eq!(output.matches(encoded_id).count(), 1, "{target}: {output}");
            }
        }
    }
}

#[test]
fn special_checkbox_ids_are_encoded_on_the_input() {
    let id = "&amp;\"{value}\\path";
    let ir: ViewIr = serde_json::from_value(serde_json::json!({
        "version": IR_VERSION,
        "root": [{"kind":"checkbox", "label":"Control", "checked":false,
            "style":{"id":id,"classes":["font-bold"]}}]
    }))
    .unwrap();
    let json = serde_json::to_string(id).unwrap();
    for (target, output) in outputs(&ir) {
        let encoded_id = match target {
            "react" | "solid" => format!("id={{{json}}}"),
            "svelte" => "id=\"&amp;amp;&quot;&#123;value&#125;\\path\"".into(),
            "vue" => "id=\"&amp;amp;&quot;{value}\\path\"".into(),
            "imba" => format!(
                "id=({})",
                json.replace('{', "\\u007b").replace('}', "\\u007d")
            ),
            _ => format!("id: {json}"),
        };
        assert_wrapped_input(target, &output, &encoded_id);
    }
}

#[test]
fn supported_file_pickers_keep_wrapper_classes_and_input_identity() {
    let ir: ViewIr = serde_json::from_value(serde_json::json!({
        "version":IR_VERSION,
        "root":[{"kind":"filePicker","label":"Upload","accept":[],"multiple":false,
            "style":{"id":"upload","classes":["font-bold"]}}]
    }))
    .unwrap();
    for (target, output) in [
        ("react", emit_react_component(&ir, "Upload")),
        ("solid", emit_solid_component(&ir, "Upload")),
        ("svelte", emit_svelte_component(&ir)),
        ("vue", emit_vue_component(&ir)),
    ] {
        assert_wrapped_input(target, &output, "id=\"upload\"");
    }
    assert_wrapped_input(
        "imba",
        &emit_imba_component(&ir).unwrap().source,
        "id=(\"upload\")",
    );
    for (target, output) in [
        ("dom", emit_moonshine_dom_component(&ir, "Upload").unwrap()),
        ("ssr", emit_moonshine_ssr_component(&ir, "Upload").unwrap()),
    ] {
        assert_wrapped_input(target, &output, "id: \"upload\"");
    }
}
