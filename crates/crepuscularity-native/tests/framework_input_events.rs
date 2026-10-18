use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir_with_path, ViewIr, IR_VERSION,
};
use serde_json::{json, Value};
use std::path::Path;

fn lower(source: &str, filename: &str) -> Result<ViewIr, crepuscularity_core::CrepusError> {
    render_template_to_ir_with_path(source, &TemplateContext::new(), Some(Path::new(filename)))
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    let imba = emit_imba_component(ir).unwrap();
    vec![
        ("react", emit_react_component(ir, "Events")),
        ("solid", emit_solid_component(ir, "Events")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", format!("{}\n{}", imba.source, imba.bindings)),
        ("dom", emit_moonshine_dom_component(ir, "Events").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Events").unwrap()),
    ]
}

#[test]
fn all_frontends_preserve_both_input_and_change_callbacks() {
    for (filename, source) in [
        ("view.crepus", "input @input=typing @change=commit"),
        ("view.jsx", "<input onInput={typing} onChange={commit}/>"),
        (
            "view.svelte",
            "<input on:input={typing} on:change={commit}/>",
        ),
        ("view.astro", "<input onInput={typing} onChange={commit}/>"),
        (
            "view.vue",
            "<template><input @input=\"typing\" @change=\"commit\"/></template>",
        ),
        ("view.ng", "<input (input)=\"typing\" (change)=\"commit\"/>"),
    ] {
        let result = lower(source, filename).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["root"][0]["onInput"], "typing", "{filename}");
        assert_eq!(json["root"][0]["onChange"], "commit", "{filename}");
    }
}

#[test]
fn emitters_keep_native_input_and_change_routes_for_input_and_textarea() {
    for tag in ["input", "textarea"] {
        let source =
            format!("<{tag} bind='value' onInput={{device.typing}} onChange={{device.commit}}/>");
        let result = lower(&source, "view.jsx").unwrap();
        for (target, output) in outputs(&result) {
            let decoded = output.replace("&quot;", "\"");
            assert!(decoded.contains("device.typing"), "{target}: {output}");
            assert!(decoded.contains("device.commit"), "{target}: {output}");
            match target {
                "react" => {
                    assert!(output.contains("onInput="), "{output}");
                    assert!(output.contains("event.nativeEvent"), "{output}");
                    assert!(output.contains("__crepusInputRef"), "{output}");
                    assert!(output.contains("defaultValue="), "{output}");
                    assert!(!output.contains("onChange="), "{output}");
                }
                "solid" => {
                    assert!(output.contains("onInput="), "{output}");
                    assert!(output.contains("onChange="), "{output}");
                }
                "svelte" => {
                    assert!(output.contains("oninput="), "{output}");
                    assert!(output.contains("onchange="), "{output}");
                }
                "vue" => {
                    assert!(output.contains("@input="), "{output}");
                    assert!(output.contains("@change="), "{output}");
                    assert!(output.contains("$event"), "{output}");
                }
                "imba" => {
                    assert!(output.contains("@input="), "{output}");
                    assert!(output.contains("@change="), "{output}");
                    assert!(
                        output.contains("Object.prototype.hasOwnProperty.call"),
                        "{output}"
                    );
                }
                _ => {
                    assert!(output.contains("\"onInput\":"), "{output}");
                    assert!(output.contains("\"onChange\":"), "{output}");
                }
            }
            if target != "imba" {
                assert!(output.contains("__crepusOwnHandler"), "{target}: {output}");
                assert!(
                    decoded.contains("?.[\"device.typing\"]?.("),
                    "{target}: {output}"
                );
                if target == "react" {
                    assert!(
                        decoded.contains("handlers, \"device.commit\")}"),
                        "{output}"
                    );
                    assert!(
                        output.contains("__crepusOwnHandler(handlers, action)?.[action]?.(event)"),
                        "{output}"
                    );
                } else {
                    assert!(
                        decoded.contains("?.[\"device.commit\"]?.("),
                        "{target}: {output}"
                    );
                }
            }
        }
    }
}

#[test]
fn old_ir_omits_input_handler_and_explicit_json_preserves_it() {
    let node = json!({"kind":"input","placeholder":"","bind":""});
    let decode = |node: Value| -> ViewIr {
        serde_json::from_value(json!({"version":IR_VERSION,"root":[node]})).unwrap()
    };
    let old = decode(node.clone());
    assert!(serde_json::to_value(&old).unwrap()["root"][0]
        .get("onInput")
        .is_none());
    let mut named = node;
    named["onInput"] = json!("typing");
    let named = decode(named);
    assert_eq!(
        serde_json::to_value(&named).unwrap()["root"][0]["onInput"],
        "typing"
    );
    for (target, output) in outputs(&old) {
        assert!(!output.contains("typing"), "{target}: {output}");
    }
}

#[test]
fn duplicate_input_handlers_and_input_events_on_other_nodes_are_errors() {
    for (filename, source) in [
        ("view.crepus", "input @input=first @input=second"),
        ("view.jsx", "<input onInput={first} onInput={second}/>"),
        ("view.svelte", "<input on:input={first} on:input={second}/>"),
        ("view.astro", "<input onInput={first} onInput={second}/>"),
        (
            "view.vue",
            "<template><input @input=\"first\" @input=\"second\"/></template>",
        ),
        ("view.ng", "<input (input)=\"first\" (input)=\"second\"/>"),
    ] {
        assert!(
            lower(source, filename).is_err(),
            "accepted {filename}: {source}"
        );
    }
    for tag in ["button", "div", "checkbox", "toggle", "picker"] {
        let source = format!("<{tag} onInput={{typing}}/>");
        let error = lower(&source, "view.jsx").unwrap_err().to_string();
        assert!(error.contains("input"), "{tag}: {error}");
    }
}
