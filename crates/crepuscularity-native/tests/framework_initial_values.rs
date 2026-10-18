use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir_with_path, NativeCodegenTarget, ViewIr,
    IR_VERSION,
};
use serde_json::{json, Value};
use std::path::Path;

fn ir(node: Value) -> ViewIr {
    serde_json::from_value(json!({"version":IR_VERSION,"root":[node]})).unwrap()
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Values")),
        ("solid", emit_solid_component(ir, "Values")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "Values").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Values").unwrap()),
    ]
}

#[test]
fn six_frontends_snapshot_initial_values_and_preserve_explicit_empty_values() {
    for (filename, source) in [
        ("view.crepus", "input value=\"Initial\"".to_string()),
        ("view.jsx", "<input value='Initial'/>".into()),
        ("view.svelte", "<input value='Initial'/>".into()),
        ("view.astro", "<input value='Initial'/>".into()),
        ("view.ng", "<input value='Initial'/>".into()),
        (
            "view.vue",
            "<template><input value='Initial'/></template>".into(),
        ),
    ] {
        let result = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["root"][0]["style"]["formValue"],
            "Initial",
            "{filename}"
        );
    }
    let mut context = TemplateContext::new();
    context.set("initial", "Snapshot");
    for value in ["{initial}", "''"] {
        let source = format!("<input value={value}/>");
        let result =
            render_template_to_ir_with_path(&source, &context, Some(Path::new("view.jsx")))
                .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["root"][0]["style"]["formValue"],
            if value == "''" { "" } else { "Snapshot" }
        );
    }
    for tag in ["input", "checkbox"] {
        let source = format!("<{tag} value='one' value='two'/>");
        assert!(
            render_template_to_ir_with_path(&source, &context, Some(Path::new("view.jsx")))
                .is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn initial_input_and_textarea_values_use_uncontrolled_output_and_host_bind_wins() {
    for multiline in [false, true] {
        for bind in ["", " \t", "host.value"] {
            let result = ir(
                json!({"kind":"input","placeholder":"","bind":bind,"multiline":multiline,"style":{"formValue":"INITIAL_ONLY"}}),
            );
            for (target, output) in outputs(&result) {
                if !bind.trim().is_empty() {
                    assert!(!output.contains("INITIAL_ONLY"), "{target}: {output}");
                    assert!(output.contains("host"), "{target}: {output}");
                    continue;
                }
                let expected = match target {
                    "react" | "solid" => "defaultValue={\"INITIAL_ONLY\"}",
                    "imba" => "defaultValue=(\"INITIAL_ONLY\")",
                    "dom" | "ssr" => "defaultValue: \"INITIAL_ONLY\"",
                    _ if multiline => ">INITIAL_ONLY</textarea>",
                    _ => "value=\"INITIAL_ONLY\"",
                };
                assert!(
                    output.contains(expected),
                    "{target}: missing {expected}: {output}"
                );
            }
        }
    }
}

#[test]
fn checkbox_submission_values_are_on_the_input_independent_of_checked_binding() {
    for bind in ["", "checked"] {
        let result = ir(
            json!({"kind":"checkbox","label":"Visible","checked":false,"bind":bind,"style":{"formValue":"submitted","classes":["font-bold"]}}),
        );
        for (target, output) in outputs(&result) {
            let (label_marker, input_marker) = if matches!(target, "dom" | "ssr") {
                ("h(\"label\", ", "h(\"input\", ")
            } else {
                ("<label", "<input")
            };
            let label = output.split_once(label_marker).unwrap().1;
            let (wrapper, input) = label.split_once(input_marker).unwrap();
            assert!(!wrapper.contains("submitted"), "{target}: {output}");
            assert!(wrapper.contains("font-bold"), "{target}: {output}");
            let expected = match target {
                "imba" => "value=(\"submitted\")",
                "dom" | "ssr" => "value: \"submitted\"",
                _ => "value=\"submitted\"",
            };
            assert!(input.contains(expected), "{target}: {output}");
            assert_eq!(output.matches("submitted").count(), 1, "{target}: {output}");
        }
    }
}

#[test]
fn literal_values_are_encoded_and_old_ir_and_native_codegen_remain_compatible() {
    let value = "&amp;\"{literal}\\path";
    let result = ir(json!({"kind":"input","placeholder":"","bind":"","style":{"formValue":value}}));
    let quoted = serde_json::to_string(value).unwrap();
    for (target, output) in outputs(&result) {
        let expected = match target {
            "react" | "solid" => format!("defaultValue={{{quoted}}}"),
            "svelte" => "value=\"&amp;amp;&quot;&#123;literal&#125;\\path\"".into(),
            "vue" => "value=\"&amp;amp;&quot;{literal}\\path\"".into(),
            "imba" => format!(
                "defaultValue=({})",
                quoted.replace('{', "\\u007b").replace('}', "\\u007d")
            ),
            _ => format!("defaultValue: {quoted}"),
        };
        assert!(output.contains(&expected), "{target}: {output}");
    }
    let old = ir(json!({"kind":"input","placeholder":"","bind":""}));
    for (target, output) in outputs(&old) {
        let assignment = if matches!(target, "dom" | "ssr") {
            "defaultValue:"
        } else {
            "defaultValue="
        };
        assert!(!output.contains(assignment), "{target}: {output}");
    }
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&old, target, "Values"),
            generate_native_source(&result, target, "Values")
        );
    }
}
