use crepuscularity_core::{ast::Node, TemplateContext};
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, render_nodes_to_ir, render_template_to_ir_with_path,
    NativeCodegenTarget, ViewIr,
};
use std::path::Path;

fn lower(source: &str, filename: &str) -> Result<ViewIr, crepuscularity_core::CrepusError> {
    let mut context = TemplateContext::new();
    context.set("name", "Ada");
    render_template_to_ir_with_path(source, &context, Some(Path::new(filename)))
}

fn value(ir: &ViewIr) -> String {
    serde_json::to_value(ir).unwrap()["root"][0]["style"]["formValue"]
        .as_str()
        .unwrap()
        .into()
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "TextArea")),
        ("solid", emit_solid_component(ir, "TextArea")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "TextArea").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "TextArea").unwrap()),
    ]
}

#[test]
fn six_frontends_keep_plain_and_mixed_textarea_bodies() {
    for (filename, source, expected) in [
        (
            "view.crepus",
            "textarea\n  \"  Hello {name}!  \"",
            "  Hello Ada!  ",
        ),
        (
            "view.jsx",
            "<textarea>  Hello {name}!  </textarea>",
            "  Hello Ada!  ",
        ),
        (
            "view.svelte",
            "<textarea>  Hello {name}!  </textarea>",
            "  Hello Ada!  ",
        ),
        (
            "view.astro",
            "<textarea>  Hello {name}!  </textarea>",
            "  Hello Ada!  ",
        ),
        (
            "view.ng",
            "<textarea>  Hello {{ name }}!  </textarea>",
            "  Hello Ada!  ",
        ),
        (
            "view.vue",
            "<template><textarea>  Hello {{ name }}!  </textarea></template>",
            "  Hello Ada!  ",
        ),
    ] {
        let result = lower(source, filename).unwrap();
        assert_eq!(value(&result), expected, "{filename}");
    }
    for filename in ["view.jsx", "view.svelte", "view.astro", "view.ng"] {
        assert_eq!(
            value(&lower("<textarea>Plain</textarea>", filename).unwrap()),
            "Plain"
        );
    }
    let alias = lower("input multiline\n  \"Hello {name}\"", "view.crepus").unwrap();
    assert_eq!(value(&alias), "Hello Ada");
}

#[test]
fn ambiguous_values_and_nontext_children_fail_instead_of_being_dropped() {
    for source in [
        "<textarea value='Explicit'>Body</textarea>",
        "<textarea><span>Nested</span></textarea>",
        "<textarea><if condition={true}>Body</if></textarea>",
        "<textarea><include src='missing.crepus'/></textarea>",
        "<textarea>{=html}</textarea>",
    ] {
        assert!(lower(source, "view.jsx").is_err(), "accepted {source}");
    }
    assert_eq!(
        value(&lower("<textarea value='Explicit'></textarea>", "view.jsx").unwrap()),
        "Explicit"
    );
    assert_eq!(
        value(&lower("<textarea value='Explicit'>{''}</textarea>", "view.jsx").unwrap()),
        "Explicit"
    );
}

#[test]
fn all_web_outputs_initialize_from_body_but_bound_host_value_wins() {
    let initial = lower("<textarea>Hello {name}!</textarea>", "view.jsx").unwrap();
    for (target, output) in outputs(&initial) {
        let expected = match target {
            "react" | "solid" => "defaultValue={\"Hello Ada!\"}",
            "imba" => "defaultValue=(\"Hello Ada!\")",
            "dom" | "ssr" => "defaultValue: \"Hello Ada!\"",
            _ => ">Hello Ada!</textarea>",
        };
        assert!(output.contains(expected), "{target}: {output}");
    }
    let bound = lower(
        "<textarea bind='host.value'>BODY_SNAPSHOT</textarea>",
        "view.jsx",
    )
    .unwrap();
    assert_eq!(value(&bound), "BODY_SNAPSHOT");
    for (target, output) in outputs(&bound) {
        assert!(!output.contains("BODY_SNAPSHOT"), "{target}: {output}");
        assert!(output.contains("host"), "{target}: {output}");
    }
    let mut old = initial.clone();
    for node in &mut old.root {
        node.style_mut().as_mut().unwrap().form_value = None;
    }
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(&initial, target, "Area"),
            generate_native_source(&old, target, "Area")
        );
    }
}

#[test]
fn raw_text_children_are_evaluated_and_concatenated_with_text_parts() {
    let mut nodes = crepuscularity_core::parse_template_with_path(
        "<textarea>Prefix </textarea>",
        Some(Path::new("view.jsx")),
    )
    .unwrap();
    let Node::Element(element) = &mut nodes[0] else {
        panic!("textarea")
    };
    element.children.push(Node::RawText("name".into()));
    let mut context = TemplateContext::new();
    context.set("name", "Ada");
    let result = render_nodes_to_ir(&nodes, &context).unwrap();
    assert_eq!(value(&result), "Prefix Ada");
}
