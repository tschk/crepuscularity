use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

#[test]
fn unsupported_island_adapters_cannot_be_dropped_from_view_ir() {
    for adapter in [None, Some("module"), Some("react"), Some("unknown")] {
        let attribute = adapter
            .map(|value| format!(" adapter=\"{value}\""))
            .unwrap_or_default();
        let island = format!("<island src=\"./widget.tsx\"{attribute}/>");
        for source in [
            island.clone(),
            format!("<div>{island}</div>"),
            format!("<button>{island}</button>"),
            format!("<button label='Explicit'>{island}</button>"),
            format!("<picker><option value='one'>{island}</option></picker>"),
        ] {
            let error = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new("Entry.jsx")),
            )
            .expect_err(&format!("embedded island was dropped: {source}"));
            assert!(
                error
                    .to_string()
                    .contains("View IR does not support embedded islands"),
                "{error}"
            );
        }
    }
}

#[test]
fn webview_adapter_remains_supported_as_an_ordinary_node() {
    let ir = render_template_to_ir_with_path(
        "<island src='https://example.invalid/view' adapter='webview'/>",
        &TemplateContext::new(),
        Some(Path::new("Entry.jsx")),
    )
    .unwrap();
    let ir = serde_json::to_value(ir).unwrap();
    assert_eq!(ir["root"][0]["kind"], "webView");
    assert_eq!(ir["root"][0]["src"], "https://example.invalid/view");
}

#[test]
fn supported_webviews_cannot_be_hidden_inside_flattened_label_metadata() {
    let island = "<island src='https://example.invalid/view' adapter='webview'/>";
    for source in [
        format!("<button>{island}</button>"),
        format!("<picker><option value='one'>{island}</option></picker>"),
    ] {
        let error = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("Entry.jsx")),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("View IR does not support embedded islands"),
            "{error}"
        );
    }
}
