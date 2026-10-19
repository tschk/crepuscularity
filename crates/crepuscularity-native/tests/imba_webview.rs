use crepuscularity_core::TemplateContext;
use crepuscularity_native::{emit_imba_component, render_template_to_ir_with_path, ViewIr};
use serde_json::json;
use std::path::Path;

#[test]
fn imba_webviews_preserve_literal_urls_and_identity_metadata() {
    let src = "about:blank?quote=\"&brace={literal}\\path";
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"webView", "src":src,
        "style":{"id":"frame-id","classes":["rounded","w-full"],"ariaLabel":"Preview frame"}
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    let frame = output
        .source
        .lines()
        .find(|line| line.contains("<iframe"))
        .unwrap();
    for attribute in [
        "id=(\"frame-id\")",
        "className=(\"rounded w-full\")",
        "aria-label=(\"Preview frame\")",
    ] {
        assert!(frame.contains(attribute), "{frame}");
    }
    let encoded = serde_json::to_string(src)
        .unwrap()
        .replace('{', "\\u007b")
        .replace('}', "\\u007d");
    if frame.contains("src=(bindings.") {
        assert!(output.bindings.contains(&encoded), "{}", output.bindings);
    } else {
        assert!(frame.contains(&format!("src=({encoded})")), "{frame}");
    }
}

#[test]
fn ordinary_iframes_and_webview_adapters_reach_the_supported_imba_node() {
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
        let output = emit_imba_component(&ir).unwrap();
        assert!(output.source.contains("<iframe"), "{}", output.source);
        assert!(output.source.contains("about:blank") || output.bindings.contains("about:blank"));
        if source.starts_with("<iframe") {
            assert!(output.source.contains("id=(\"preview\")"));
            assert!(output.source.contains("aria-label=(\"Preview\")"));
        }
    }
}
