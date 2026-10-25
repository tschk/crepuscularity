use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir_with_path, NativeCodegenTarget, ViewIr,
    IR_VERSION,
};
use serde_json::{json, Value};
use std::path::Path;

fn lower(
    source: &str,
    filename: &str,
    context: &TemplateContext,
) -> Result<ViewIr, crepuscularity_core::CrepusError> {
    render_template_to_ir_with_path(source, context, Some(Path::new(filename)))
}

fn ir(node: Value) -> ViewIr {
    serde_json::from_value(json!({"version":IR_VERSION,"root":[node]})).unwrap()
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Fields")),
        ("solid", emit_solid_component(ir, "Fields")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "Fields").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Fields").unwrap()),
    ]
}

fn attribute(target: &str, name: &str, value: &str) -> String {
    match target {
        "imba" => format!("{name}=(\"{value}\")"),
        "dom" | "ssr" => format!("{name}: \"{value}\""),
        _ => format!("{name}=\"{value}\""),
    }
}

#[test]
fn frontend_fields_keep_type_name_and_boolean_presence() {
    let markup = "<input type='email' name='email' required/>";
    for (filename, source) in [
        (
            "view.crepus",
            "input type=\"email\" name=\"email\" required=true".to_string(),
        ),
        ("view.jsx", markup.into()),
        ("view.svelte", markup.into()),
        ("view.astro", markup.into()),
        ("view.ng", markup.into()),
        ("view.vue", format!("<template>{markup}</template>")),
    ] {
        let result = lower(&source, filename, &TemplateContext::new()).unwrap();
        let serialized = serde_json::to_value(&result).unwrap();
        let style = &serialized["root"][0]["style"];
        assert_eq!(style["formName"], "email", "{filename}");
        assert_eq!(style["inputType"], "email", "{filename}");
        assert_eq!(style["required"], true, "{filename}");
        for (target, output) in outputs(&result) {
            assert!(
                output.contains(&attribute(target, "type", "email")),
                "{filename}/{target}: {output}"
            );
            assert!(
                output.contains(&attribute(target, "name", "email")),
                "{filename}/{target}: {output}"
            );
            assert!(output.contains("required"), "{filename}/{target}: {output}");
        }
    }
}

#[test]
fn expressions_are_snapshots_and_duplicate_or_structural_types_fail() {
    let mut context = TemplateContext::new();
    context.set("field", "contact");
    context.set("kind", "email");
    context.set("needed", true);
    let result = lower(
        "<input type={kind} name={field} required={needed}/>",
        "view.jsx",
        &context,
    )
    .unwrap();
    let serialized = serde_json::to_value(result).unwrap();
    assert_eq!(serialized["root"][0]["style"]["formName"], "contact");
    assert_eq!(serialized["root"][0]["style"]["inputType"], "email");
    assert_eq!(serialized["root"][0]["style"]["required"], true);
    for key in ["type", "name", "required"] {
        let source = format!("<input {key}='email' {key}='email'/>");
        assert!(
            lower(&source, "view.jsx", &context).is_err(),
            "accepted {source}"
        );
    }
    for kind in [
        "radio", "range", "button", "submit", "reset", "image", "unknown",
    ] {
        let source = format!("<input type='{kind}'/>");
        assert!(
            lower(&source, "view.jsx", &context).is_err(),
            "accepted {source}"
        );
    }
    assert!(lower("<input type='checkbox'/>", "view.jsx", &context).is_ok());
    assert!(lower("<textfield type='checkbox'/>", "view.jsx", &context).is_ok());
}

#[test]
fn explicit_names_override_bind_fallback_and_required_uses_presence_semantics() {
    for (source, required) in [
        ("<input bind='host.path' name='' required={false}/>", false),
        ("<input bind='host.path' name='' required={null}/>", false),
        ("<input bind='host.path' name='' required='false'/>", true),
    ] {
        let result = lower(source, "view.jsx", &TemplateContext::new()).unwrap();
        for (target, output) in outputs(&result) {
            assert!(
                output.contains(&attribute(target, "name", "")),
                "{target}: {output}"
            );
            assert!(
                !output.contains(&attribute(target, "name", "host.path")),
                "{target}: {output}"
            );
            assert_eq!(output.contains("required"), required, "{target}: {output}");
        }
    }
    let old = lower(
        "<input bind='host.path'/>",
        "view.jsx",
        &TemplateContext::new(),
    )
    .unwrap();
    for (target, output) in outputs(&old) {
        assert!(
            output.contains(&attribute(target, "name", "host.path")),
            "{target}: {output}"
        );
    }
}

#[test]
fn composite_field_attributes_belong_to_inputs_and_names_are_escaped() {
    for kind in ["checkbox", "filePicker"] {
        let node = json!({"kind":kind,"label":"Visible","checked":false,"accept":[],"multiple":false,
            "style":{"formName":"field","required":true,"classes":["font-bold"]}});
        for (target, output) in outputs(&ir(node)) {
            let (label_marker, input_marker) = if matches!(target, "dom" | "ssr") {
                ("h(\"label\", ", "h(\"input\", ")
            } else {
                ("<label", "<input")
            };
            let label = output.split_once(label_marker).unwrap().1;
            let (wrapper, input) = label.split_once(input_marker).unwrap();
            assert!(wrapper.contains("font-bold"), "{target}: {output}");
            assert!(
                !wrapper.contains("required") && !wrapper.contains("name="),
                "{target}: {output}"
            );
            let name = attribute(target, "name", "field");
            assert!(
                input.contains(&name) && input.contains("required"),
                "{target}: {output}"
            );
            assert_eq!(output.matches(&name).count(), 1, "{target}: {output}");
        }
    }
    let result = ir(
        json!({"kind":"input","placeholder":"","bind":"","style":{"formName":"&amp;\"{x}\\path"}}),
    );
    for (target, output) in outputs(&result) {
        let expected = match target {
            "react" | "solid" => r#"name={"&amp;\"{x}\\path"}"#,
            "svelte" => "name=\"&amp;amp;&quot;&#123;x&#125;\\path\"",
            "vue" => "name=\"&amp;amp;&quot;{x}\\path\"",
            "imba" => r#"name=("&amp;\"\u007bx\u007d\\path")"#,
            _ => r#"name: "&amp;\"{x}\\path""#,
        };
        assert!(output.contains(expected), "{target}: {output}");
    }
}

#[test]
fn supplied_type_hints_respect_secure_and_native_codegen_ignores_web_metadata() {
    for (hint, secure, expected) in [
        ("email", false, "email"),
        ("invalid", false, "text"),
        ("email", true, "password"),
        ("checkbox", false, "text"),
    ] {
        let result = ir(
            json!({"kind":"input","placeholder":"","bind":"","secure":secure,"style":{"inputType":hint}}),
        );
        for (target, output) in outputs(&result) {
            assert!(
                output.contains(&attribute(target, "type", expected)),
                "{target}: {output}"
            );
        }
    }
    let old = ir(json!({"kind":"input","placeholder":"","bind":""}));
    let named = ir(
        json!({"kind":"input","placeholder":"","bind":"","style":{"formName":"email","inputType":"email","required":true}}),
    );
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&old, target, "Fields"),
            generate_native_source(&named, target, "Fields")
        );
    }
}

#[test]
fn picker_and_slider_keep_explicit_names_without_duplicate_bind_names() {
    for node in [
        json!({"kind":"picker","bind":"host.choice","options":[],"style":{"formName":"choice","required":true}}),
        json!({"kind":"slider","bind":"host.level","value":0.5,"min":0.0,"max":1.0,"style":{"formName":"level"}}),
    ] {
        let is_picker = node["kind"] == "picker";
        let name = if is_picker { "choice" } else { "level" };
        for (target, output) in outputs(&ir(node.clone())) {
            let expected = attribute(target, "name", name);
            assert!(output.contains(&expected), "{target}: {output}");
            assert_eq!(output.matches(&expected).count(), 1, "{target}: {output}");
            assert!(
                !output.contains(&attribute(target, "name", &format!("host.{name}"))),
                "{target}: {output}"
            );
            assert_eq!(output.contains("required"), is_picker, "{target}: {output}");
        }
    }
}

#[test]
fn dom_pickers_preserve_empty_option_values_and_unbound_first_selection() {
    for bind in ["", " \t", "choice"] {
        let result = ir(json!({
            "kind":"picker", "bind":bind,
            "options":[
                {"value":"","label":"Choose an option"},
                {"value":"second","label":"Second"}
            ]
        }));
        for (target, output) in [
            (
                "dom",
                emit_moonshine_dom_component(&result, "Picker").unwrap(),
            ),
            (
                "ssr",
                emit_moonshine_ssr_component(&result, "Picker").unwrap(),
            ),
        ] {
            let first = output.split_once("h(\"option\", ").unwrap().1;
            let (first, second) = first.split_once("h(\"option\", ").unwrap();
            assert!(first.contains("value: \"\""), "{target}: {output}");
            assert!(
                first.contains("ref: (option: any) => option.setAttribute(\"value\", \"\")"),
                "{target}: {output}"
            );
            assert_eq!(
                first.contains("selected: true"),
                bind.trim().is_empty(),
                "{target}: {output}"
            );
            assert!(!second.contains("selected: true"), "{target}: {output}");
            assert!(second.contains("value: \"second\""), "{target}: {output}");
            assert_eq!(
                output.contains("value: () =>"),
                !bind.trim().is_empty(),
                "{target}: {output}"
            );
        }
    }
}
