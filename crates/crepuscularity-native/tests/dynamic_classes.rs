use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

#[test]
fn dynamic_class_lists_match_literal_layout_and_style() {
    let mut ctx = TemplateContext::new();
    ctx.set("theme", "flex flex-row gap-4 p-2 bg-red-500");
    for (filename, dynamic, literal) in [
        (
            "view.crepus",
            "div class={theme}",
            "div class=\"flex flex-row gap-4 p-2 bg-red-500\"",
        ),
        (
            "view.jsx",
            "<div class={theme} />",
            "<div class=\"flex flex-row gap-4 p-2 bg-red-500\" />",
        ),
    ] {
        let dynamic =
            render_template_to_ir_with_path(dynamic, &ctx, Some(Path::new(filename))).unwrap();
        let literal =
            render_template_to_ir_with_path(literal, &ctx, Some(Path::new(filename))).unwrap();
        let dynamic = serde_json::to_value(dynamic).unwrap();
        assert_eq!(dynamic, serde_json::to_value(literal).unwrap());
        assert_eq!(dynamic["root"][0]["axis"], "row");
        assert_eq!(dynamic["root"][0]["spacing"], 16.0);
    }
}

#[test]
fn invalid_dynamic_class_expression_returns_error() {
    assert!(render_template_to_ir_with_path(
        "div class={1 +}",
        &TemplateContext::new(),
        Some(Path::new("view.crepus")),
    )
    .is_err());
}
