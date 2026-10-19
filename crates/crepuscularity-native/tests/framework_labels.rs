use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_template_to_ir_with_path, NativeCodegenTarget, ViewIr,
    IR_VERSION,
};
use std::path::Path;

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Labels")),
        ("solid", emit_solid_component(ir, "Labels")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "Labels").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Labels").unwrap()),
    ]
}
fn lower(
    source: &str,
    path: &str,
    context: &TemplateContext,
) -> Result<ViewIr, crepuscularity_core::CrepusError> {
    render_template_to_ir_with_path(source, context, Some(Path::new(path)))
}
#[test]
fn six_frontends_preserve_plain_and_nested_labels_and_target_ids() {
    let markup = "<div><label for='plain'>Name</label><input id='plain'/><label for='nested'>Full <strong>name</strong></label><input id='nested'/></div>";
    let indent = "div\n  label for=\"plain\" \"Name\"\n  input #plain\n  label for=\"nested\"\n    strong \"Full name\"\n  input #nested";
    for (path, source) in [
        ("view.crepus", indent.to_string()),
        ("view.jsx", markup.into()),
        ("view.svelte", markup.into()),
        ("view.astro", markup.into()),
        ("view.ng", markup.into()),
        ("view.vue", format!("<template>{markup}</template>")),
    ] {
        let ir = lower(&source, path, &TemplateContext::new()).unwrap();
        for (target, output) in outputs(&ir) {
            for id in ["plain", "nested"] {
                let expected = match target {
                    "react" => format!("htmlFor=\"{id}\""),
                    "imba" => format!("for=(\"{id}\")"),
                    "dom" | "ssr" => format!("for: \"{id}\""),
                    _ => format!("for=\"{id}\""),
                };
                assert!(
                    output.contains(&expected),
                    "{path}/{target}: {expected}: {output}"
                );
                let expected_id = match target {
                    "imba" => format!("id=(\"{id}\")"),
                    "dom" | "ssr" => format!("id: \"{id}\""),
                    _ => format!("id=\"{id}\""),
                };
                assert!(output.contains(&expected_id), "{path}/{target}: {output}");
            }
            assert!(
                output.contains("<label") || output.contains("h(\"label\""),
                "{path}/{target}: {output}"
            );
        }
    }
}

#[test]
fn aliases_and_bound_targets_are_snapshots_and_duplicates_fail() {
    let mut context = TemplateContext::new();
    context.set("target", "field");
    for (path, source) in [
        ("view.jsx", "<label htmlFor={target}>Name</label>"),
        ("view.svelte", "<label for={target}>Name</label>"),
        (
            "view.vue",
            "<template><label :for=\"target\">Name</label></template>",
        ),
    ] {
        let ir = lower(source, path, &context).unwrap();
        assert_eq!(
            serde_json::to_value(&ir).unwrap()["root"][0]["style"]["htmlFor"],
            "field"
        );
    }
    for attrs in [
        "for='a' for='b'",
        "for='a' htmlFor='b'",
        "htmlFor='a' htmlFor='b'",
    ] {
        for path in [
            "view.jsx",
            "view.svelte",
            "view.vue",
            "view.astro",
            "view.ng",
        ] {
            let source = format!("<label {attrs}>Name</label>");
            let source = if path.ends_with("vue") {
                format!("<template>{source}</template>")
            } else {
                source
            };
            assert!(
                lower(&source, path, &context).is_err(),
                "accepted {path}: {source}"
            );
        }
    }
}
#[test]
fn label_metadata_escapes_literals_and_is_ignored_by_native_codegen() {
    let value = "&amp;\"{target}\\path";
    let make_ir = |metadata: serde_json::Value| -> ViewIr {
        serde_json::from_value(serde_json::json!({"version":IR_VERSION,"root":[{"kind":"text","content":"Name","style":metadata}]})).unwrap()
    };
    let ir = make_ir(serde_json::json!({"htmlTag":"label","htmlFor":value}));
    let json = serde_json::to_string(value).unwrap();
    for (target, output) in outputs(&ir) {
        let expected = match target {
            "react" => format!("htmlFor={{{json}}}"),
            "solid" => format!("for={{{json}}}"),
            "svelte" => "for=\"&amp;amp;&quot;&#123;target&#125;\\path\"".into(),
            "vue" => "for=\"&amp;amp;&quot;{target}\\path\"".into(),
            "imba" => format!(
                "for=({})",
                json.replace('{', "\\u007b").replace('}', "\\u007d")
            ),
            _ => format!("for: {json}"),
        };
        assert!(
            output.contains(&expected),
            "{target}: {expected:?}: {output}"
        );
    }
    let non_label = make_ir(serde_json::json!({"htmlTag":"p","htmlFor":"unused-target"}));
    for (target, output) in outputs(&non_label) {
        assert!(
            !output.contains("unused-target"),
            "{target}: non-label target leaked: {output}"
        );
    }
    let old = make_ir(serde_json::json!({"htmlTag":"label"}));
    for (target, output) in outputs(&old) {
        assert!(
            !output.contains("htmlFor=") && !output.contains(" for=") && !output.contains("for: "),
            "{target}: {output}"
        );
    }
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&ir, target, "Labels"),
            generate_native_source(&old, target, "Labels")
        );
    }
}
