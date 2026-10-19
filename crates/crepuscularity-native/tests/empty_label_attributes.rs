use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use serde_json::json;
use std::path::Path;

fn source(filename: &str, expression: &str) -> String {
    match filename {
        "Entry.crepus" => format!("button label={{{expression}}}\n  {{caption}}"),
        "Entry.vue" => format!("<template><button :label=\"{expression}\"><span v-text='caption'/></button></template>"),
        _ => format!("<button label={{{expression}}}>{{caption}}</button>"),
    }
}

#[test]
fn empty_label_attributes_allow_reactive_child_fallback_parts() {
    let mut context = TemplateContext::new();
    context.set("caption", "Child caption");
    for filename in ["Entry.crepus", "Entry.jsx", "Entry.vue"] {
        for expression in ["''", "'   '", "null", "missing"] {
            let source = source(filename, expression);
            let ir = render_template_to_ir_with_path(&source, &context, Some(Path::new(filename)))
                .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["label"], "Child caption", "{source}: {ir}");
            assert_eq!(
                ir["root"][0]["labelParts"],
                json!([{"kind":"expr","value":"caption"}]),
                "{source}: {ir}"
            );
        }
    }
}

#[test]
fn nonempty_label_attribute_values_keep_snapshot_precedence() {
    let mut context = TemplateContext::new();
    context.set("caption", "Child caption");
    for filename in ["Entry.crepus", "Entry.jsx", "Entry.vue"] {
        for (expression, expected) in [("'Explicit'", "Explicit"), ("false", "false"), ("0", "0")] {
            let source = source(filename, expression);
            let ir = render_template_to_ir_with_path(&source, &context, Some(Path::new(filename)))
                .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["label"], expected, "{source}: {ir}");
            assert!(ir["root"][0].get("labelParts").is_none(), "{source}: {ir}");
        }
    }
}

#[test]
fn duplicate_empty_label_attributes_are_still_explicit_errors() {
    for (filename, source) in [
        ("Entry.crepus", "button label=\"\" label=\"\"\n  {caption}"),
        ("Entry.jsx", "<button label='' label=''>{caption}</button>"),
        (
            "Entry.vue",
            "<template><button label='' :label=\"''\"><span v-text='caption'/></button></template>",
        ),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("duplicate label binding"),
            "{filename}: {error}"
        );
    }
}
