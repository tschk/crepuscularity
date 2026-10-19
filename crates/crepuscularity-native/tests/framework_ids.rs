use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir_with_path, ViewIr, ViewNode, ViewStyle, IR_VERSION,
};
use std::path::Path;

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    vec![
        ("react", emit_react_component(ir, "Identity")),
        ("solid", emit_solid_component(ir, "Identity")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().source),
        ("dom", emit_moonshine_dom_component(ir, "Identity").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Identity").unwrap()),
    ]
}
#[test]
fn every_frontend_preserves_ids_without_requiring_classes() {
    let markup = "<div id='container'><p id='text'>Text</p><input id='field'/><button id='action'>Go</button><span id='styled' class='font-bold'>Styled</span></div>";
    for (filename, source) in [
        ("view.crepus", "div #container\n  p #text \"Text\"\n  input #field\n  button #action \"Go\"\n  span #styled font-bold \"Styled\"".to_string()),
        ("view.jsx", markup.into()), ("view.svelte",markup.into()), ("view.astro",markup.into()), ("view.ng",markup.into()), ("view.vue",format!("<template>{markup}</template>"))
    ] {
        let ir = render_template_to_ir_with_path(&source,&TemplateContext::new(),Some(Path::new(filename))).unwrap();
        for (target,output) in outputs(&ir) {
            for id in ["container","text","field","action","styled"] {
                let expected = match target { "imba"=>format!("id=(\"{id}\")"), "dom"|"ssr"=>format!("id: \"{id}\""), _=>format!("id=\"{id}\"") };
                assert!(output.contains(&expected),"{filename}/{target}: missing {expected}: {output}");
            }
            assert!(output.contains("font-bold"),"{filename}/{target}: class lost");
        }
    }
}
#[test]
fn special_ids_are_literal_data_and_absent_ids_add_no_attribute() {
    let id = "&amp;\"{value}\\path";
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Text {
            content: "Text".into(),
            bind: None,
            style: Some(ViewStyle {
                id: Some(id.into()),
                ..Default::default()
            }),
        }],
    };
    let json = serde_json::to_string(id).unwrap();
    for (target, output) in outputs(&ir) {
        let expected = match target {
            "react" | "solid" => format!("id={{{json}}}"),
            "svelte" => "id=\"&amp;amp;&quot;&#123;value&#125;\\path\"".into(),
            "vue" => "id=\"&amp;amp;&quot;{value}\\path\"".into(),
            "imba" => format!(
                "id=({})",
                json.replace('{', "\\u007b").replace('}', "\\u007d")
            ),
            _ => format!("id: {json}"),
        };
        assert!(
            output.contains(&expected),
            "{target}: missing {expected:?}: {output}"
        );
    }
    let entity_ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Text {
            content: "Text".into(),
            bind: None,
            style: Some(ViewStyle {
                id: Some("&amp;".into()),
                ..Default::default()
            }),
        }],
    };
    for (target, output) in outputs(&entity_ir) {
        if matches!(target, "react" | "solid") {
            assert!(output.contains(r#"id={"&amp;"}"#), "{target}: {output}");
        }
    }
    for style in [None, Some(ViewStyle::default())] {
        let ir = ViewIr {
            version: IR_VERSION,
            root: vec![ViewNode::Text {
                content: "Text".into(),
                bind: None,
                style,
            }],
        };
        for (target, output) in outputs(&ir) {
            assert!(!output.contains(" id="), "{target}: {output}");
            assert!(!output.contains("id: \"\""), "{target}: {output}");
        }
    }
}
