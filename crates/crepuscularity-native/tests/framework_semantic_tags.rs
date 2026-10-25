use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    generate_native_source, ir::ViewStyle, render_template_to_ir_with_path, NativeCodegenTarget,
    ViewIr, ViewNode, IR_VERSION,
};
use std::path::Path;

fn outputs(ir: &ViewIr) -> Vec<String> {
    vec![
        emit_react_component(ir, "Semantic"),
        emit_solid_component(ir, "Semantic"),
        emit_svelte_component(ir),
        emit_vue_component(ir),
        emit_imba_component(ir).unwrap().source,
        emit_moonshine_dom_component(ir, "Semantic").unwrap(),
        emit_moonshine_ssr_component(ir, "Semantic").unwrap(),
    ]
}

#[test]
fn source_headings_paragraphs_and_nested_emphasis_survive_every_web_target() {
    for (filename, source) in [
        ("page.crepus", "h1 \"Title {count}\"\np\n  \"Hello \"\n  strong \"world {count}\"\nh2"),
        ("page.jsx", "<h1>Title {count}</h1><p>Hello <strong>world {count}</strong></p><h2></h2>"),
        ("page.svelte", "<h1>Title {count}</h1><p>Hello <strong>world {count}</strong></p><h2></h2>"),
        ("page.vue", "<template><h1>Title {{count}}</h1><p>Hello <strong>world {{count}}</strong></p><h2></h2></template>"),
        ("page.astro", "<h1>Title {count}</h1><p>Hello <strong>world {count}</strong></p><h2></h2>"),
        ("page.ng.html", "<h1>Title {{count}}</h1><p>Hello <strong>world {{count}}</strong></p><h2></h2>"),
    ] {
        let ir = render_template_to_ir_with_path(source, &Default::default(), Some(Path::new(filename))).unwrap();
        let json = serde_json::to_value(&ir).unwrap();
        assert_eq!(json["root"][0]["style"]["htmlTag"], "h1", "{filename}: {json}");
        assert_eq!(json["root"][1]["style"]["htmlTag"], "p", "{filename}: {json}");
        assert_eq!(json["root"][1]["children"][1]["style"]["htmlTag"], "strong", "{filename}: {json}");
        for output in outputs(&ir) {
            for tag in ["h1", "p", "strong", "h2"] {
                assert!(output.contains(&format!("<{tag}")) || output.contains(&format!("h(\"{tag}\"")), "{filename}/{tag}: {output}");
            }
        }
    }
}

#[test]
fn old_ir_keeps_default_tags_and_supplied_tag_hints_cannot_inject_markup() {
    let old: ViewIr =
        serde_json::from_str(r#"{"version":7,"root":[{"kind":"text","content":"Old"}]}"#).unwrap();
    for output in outputs(&old) {
        assert!(
            output.contains("<span") || output.contains("h(\"span\""),
            "{output}"
        );
    }
    for tag in ["script", "h1 onclick=alert(1)", "p><script>", "iframe"] {
        let ir = ViewIr {
            version: IR_VERSION,
            root: vec![ViewNode::Text {
                content: "safe".into(),
                bind: None,
                style: Some(ViewStyle {
                    html_tag: Some(tag.into()),
                    ..Default::default()
                }),
            }],
        };
        for output in outputs(&ir) {
            assert!(
                !output.contains(&format!("<{tag}>safe"))
                    && !output.contains(&format!("h(\"{tag}\"")),
                "{output}"
            );
            assert!(
                output.contains("<span") || output.contains("h(\"span\""),
                "{output}"
            );
        }
    }
}

#[test]
fn native_codegen_ignores_web_tag_hints() {
    let mut ir = render_template_to_ir_with_path(
        "<h1>Title</h1><p>Hello <strong>world</strong></p>",
        &Default::default(),
        Some(Path::new("page.jsx")),
    )
    .unwrap();
    let expected: Vec<_> = [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose]
        .into_iter()
        .map(|target| generate_native_source(&ir, target, "Page"))
        .collect();
    fn clear(node: &mut ViewNode) {
        if let Some(style) = node.style_mut() {
            style.html_tag = None;
        }
        if let ViewNode::Stack { children, .. } = node {
            for child in children {
                clear(child);
            }
        }
    }
    for node in &mut ir.root {
        clear(node);
    }
    for (target, expected) in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose]
        .into_iter()
        .zip(expected)
    {
        assert_eq!(generate_native_source(&ir, target, "Page"), expected);
    }
}
