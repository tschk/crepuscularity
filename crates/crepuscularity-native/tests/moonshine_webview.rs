use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_dom_component, emit_moonshine_ssr_component, render_template_to_ir_with_path,
    ViewIr,
};
use serde_json::json;
use std::path::Path;

fn outputs(ir: &ViewIr) -> [String; 2] {
    [
        emit_moonshine_dom_component(ir, "Preview").unwrap(),
        emit_moonshine_ssr_component(ir, "Preview").unwrap(),
    ]
}

#[test]
fn direct_host_webviews_preserve_literal_urls_and_identity() {
    let src = "about:blank?quote=\"&brace={raw}<tag>\\path\nnext";
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"webView", "src":src,
        "style":{"id":"preview","classes":["rounded","w-full"],"ariaLabel":"Preview frame"}
    }]}))
    .unwrap();
    for output in outputs(&ir) {
        for retained in [
            "h(\"iframe\"",
            "id: \"preview\"",
            "class: \"rounded w-full\"",
            "\"aria-label\": \"Preview frame\"",
        ] {
            assert!(output.contains(retained), "missing {retained}: {output}");
        }
        assert!(
            output.contains(&format!("src: {}", serde_json::to_string(src).unwrap())),
            "{output}"
        );
        assert!(!output.contains("src: () =>"), "{output}");
    }
}

#[test]
fn source_iframe_and_webview_adapter_emit_the_same_supported_element() {
    for source in [
        "<iframe src='about:blank' id='preview' class='rounded' aria-label='Preview'/>",
        "<island src='about:blank' adapter='webview'/>",
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Preview.jsx")),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&ir).unwrap()["root"][0]["kind"],
            "webView"
        );
        for output in outputs(&ir) {
            assert!(output.contains("h(\"iframe\""), "{output}");
            assert!(output.contains("src: \"about:blank\""), "{output}");
            if source.starts_with("<iframe") {
                assert!(output.contains("id: \"preview\""), "{output}");
                assert!(output.contains("\"aria-label\": \"Preview\""), "{output}");
            }
        }
    }
}
