use std::path::Path;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_template_to_ir_with_path, IR_VERSION};

/// `crepuscularity_wasm::parse_template_json` is a thin wrapper over
/// `render_template_to_ir_with_path`, so exercising this call from a native
/// test proves the new frontends are reachable from JavaScript too.
fn ir_json(source: &str, filename: &str) -> serde_json::Value {
    let ir =
        render_template_to_ir_with_path(source, &TemplateContext::new(), Some(Path::new(filename)))
            .unwrap_or_else(|e| panic!("{filename} should lower to View IR: {e}"));
    assert_eq!(ir.version, IR_VERSION);
    serde_json::to_value(&ir).expect("ir to Value")
}

#[test]
fn astro_files_reach_the_view_ir() {
    let json = ir_json(
        r#"---
const title = "ignored";
---
<section class="p-4"><h1>Hello</h1></section>"#,
        "Page.astro",
    );
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("p-4"), "{text}");
    assert!(text.contains("Hello"), "{text}");
    assert!(
        !text.contains("ignored"),
        "frontmatter must not render: {text}"
    );
}

#[test]
fn angular_component_templates_reach_the_view_ir() {
    let json = ir_json(
        r#"<ul><li *ngFor="let item of items">Item</li></ul>"#,
        "app.component.html",
    );
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("Item"), "{text}");
}
