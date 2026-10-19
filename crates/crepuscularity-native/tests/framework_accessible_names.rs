use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir_with_path, NativeCodegenTarget, ViewIr,
    IR_VERSION,
};
use serde_json::{json, Value};
use std::path::Path;

fn framework_outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Names")),
        ("solid", emit_solid_component(ir, "Names")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
    ]
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    let mut output = framework_outputs(ir);
    output.push(("dom", emit_moonshine_dom_component(ir, "Names").unwrap()));
    output.push(("ssr", emit_moonshine_ssr_component(ir, "Names").unwrap()));
    output
}

fn ir(node: Value) -> ViewIr {
    serde_json::from_value(json!({"version":IR_VERSION,"root":[node]})).unwrap()
}

fn named_attr(target: &str, name: &str) -> String {
    let quoted = serde_json::to_string(name).unwrap();
    match target {
        "imba" => format!("aria-label=({quoted})"),
        "dom" | "ssr" => format!("\"aria-label\": {quoted}"),
        _ => format!("aria-label=\"{name}\""),
    }
}

#[test]
fn all_frontends_keep_input_and_button_names_and_bound_snapshots() {
    let markup = "<div><input aria-label='Entry'/><button aria-label='Action'>Go</button></div>";
    let indent = "div\n  input aria-label=\"Entry\"\n  button aria-label=\"Action\" \"Go\"";
    for (filename, source) in [
        ("view.crepus", indent.to_string()),
        ("view.jsx", markup.into()),
        ("view.svelte", markup.into()),
        ("view.astro", markup.into()),
        ("view.ng", markup.into()),
        ("view.vue", format!("<template>{markup}</template>")),
    ] {
        let result = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        for (target, output) in outputs(&result) {
            for name in ["Entry", "Action"] {
                assert!(
                    output.contains(&named_attr(target, name)),
                    "{filename}/{target}: {output}"
                );
            }
        }
    }
    let mut context = TemplateContext::new();
    context.set("name", "Snapshot");
    for (filename, source) in [
        ("view.crepus", "input aria-label={name}"),
        ("view.jsx", "<input aria-label={name}/>"),
        ("view.astro", "<input aria-label={name}/>"),
        ("view.ng", "<input [attr.aria-label]=\"name\"/>"),
        ("view.svelte", "<input aria-label={name}/>"),
        (
            "view.vue",
            "<template><input :aria-label=\"name\"/></template>",
        ),
    ] {
        let result =
            render_template_to_ir_with_path(source, &context, Some(Path::new(filename))).unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["root"][0]["style"]["ariaLabel"],
            "Snapshot"
        );
    }
    for filename in [
        "view.jsx",
        "view.svelte",
        "view.astro",
        "view.ng",
        "view.vue",
    ] {
        let source = "<input aria-label='First' aria-label='Second'/>";
        let source = if filename.ends_with("vue") {
            format!("<template>{source}</template>")
        } else {
            source.into()
        };
        assert!(
            render_template_to_ir_with_path(&source, &context, Some(Path::new(filename))).is_err(),
            "accepted {filename}"
        );
    }
}

#[test]
fn explicit_names_override_indicator_fallbacks_including_empty_names() {
    for kind in ["slider", "progress", "meter"] {
        for name in ["Explicit", ""] {
            let result = ir(
                json!({"kind":kind,"value":0.5,"min":0.0,"max":1.0,"label":"Fallback","style":{"ariaLabel":name}}),
            );
            for (target, output) in outputs(&result) {
                assert!(
                    output.contains(&named_attr(target, name)),
                    "{kind}/{target}: {output}"
                );
                assert_eq!(
                    output.matches("aria-label=").count()
                        + output.matches("\"aria-label\":").count(),
                    1,
                    "{kind}/{target}: {output}"
                );
                assert!(!output.contains("Fallback"), "{kind}/{target}: {output}");
            }
        }
        let result = ir(json!({"kind":kind,"value":0.5,"min":0.0,"max":1.0,"label":"Fallback"}));
        for (target, output) in outputs(&result) {
            assert!(
                output.contains(&named_attr(target, "Fallback")),
                "{kind}/{target}: {output}"
            );
        }
    }
}

#[test]
fn composite_controls_name_the_input_and_tabs_name_the_tablist() {
    for kind in ["checkbox", "toggle", "filePicker"] {
        let result = ir(
            json!({"kind":kind,"label":"Visible","checked":false,"accept":[],"multiple":false,"style":{"ariaLabel":"Control","classes":["font-bold"]}}),
        );
        for (target, output) in outputs(&result) {
            let marker = named_attr(target, "Control");
            if kind != "toggle" || matches!(target, "dom" | "ssr") {
                let (label_marker, input_marker) = if matches!(target, "dom" | "ssr") {
                    ("h(\"label\", ", "h(\"input\", ")
                } else {
                    ("<label", "<input")
                };
                let label = output.split_once(label_marker).unwrap().1;
                let (wrapper, input) = label.split_once(input_marker).unwrap();
                assert!(wrapper.contains("font-bold"), "{target}: {output}");
                assert!(!wrapper.contains("aria-label"), "{target}: {output}");
                assert!(input.contains(&marker), "{target}: {output}");
            } else {
                assert!(
                    output.split_once("<button").unwrap().1.contains(&marker),
                    "{target}: {output}"
                );
            }
            assert_eq!(
                output.matches("aria-label=").count() + output.matches("\"aria-label\":").count(),
                1,
                "{target}: {output}"
            );
        }
    }
    let tabs = ir(
        json!({"kind":"tabs","bind":"active","tabs":[{"value":"one","label":"One","children":[]}],"style":{"ariaLabel":"Sections"}}),
    );
    for (target, output) in framework_outputs(&tabs) {
        let tablist = output
            .split_once("role=\"tablist\"")
            .or_else(|| output.split_once("role=(\"tablist\")"))
            .unwrap()
            .1;
        assert!(
            tablist.contains(&named_attr(target, "Sections")),
            "{target}: {output}"
        );
        assert_eq!(
            output.matches("aria-label=").count() + output.matches("\"aria-label\":").count(),
            1,
            "{target}: {output}"
        );
    }
    for output in [
        emit_moonshine_dom_component(&tabs, "Names").unwrap(),
        emit_moonshine_ssr_component(&tabs, "Names").unwrap(),
    ] {
        assert!(
            output.contains("role: \"tablist\", \"aria-label\": \"Sections\""),
            "{output}"
        );
        assert_eq!(output.matches("\"aria-label\":").count(), 1, "{output}");
    }
}

#[test]
fn metadata_is_escaped_while_old_ir_and_native_codegen_stay_compatible() {
    let name = "&amp;\"{value}\\path";
    let result = ir(json!({"kind":"input","placeholder":"","bind":"","style":{"ariaLabel":name}}));
    let quoted = serde_json::to_string(name).unwrap();
    for (target, output) in outputs(&result) {
        let expected = match target {
            "react" | "solid" => format!("aria-label={{{quoted}}}"),
            "svelte" => "aria-label=\"&amp;amp;&quot;&#123;value&#125;\\path\"".into(),
            "vue" => "aria-label=\"&amp;amp;&quot;{value}\\path\"".into(),
            "imba" => format!(
                "aria-label=({})",
                quoted.replace('{', "\\u007b").replace('}', "\\u007d")
            ),
            _ => format!("\"aria-label\": {quoted}"),
        };
        assert!(output.contains(&expected), "{target}: {output}");
    }
    let old = ir(json!({"kind":"input","placeholder":"","bind":""}));
    for (target, output) in outputs(&old) {
        assert!(!output.contains("aria-label"), "{target}: {output}");
    }
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&result, target, "Names"),
            generate_native_source(&old, target, "Names")
        );
    }
}
