use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_native::render_template_to_ir_with_path;
use serde_json::json;
use std::path::Path;

#[test]
fn initially_empty_nested_expressions_keep_their_live_label_parts() {
    for value in [
        None,
        Some(TemplateValue::Null),
        Some(TemplateValue::Str(String::new())),
    ] {
        let mut context = TemplateContext::new();
        if let Some(value) = value {
            context.set("caption", value);
        }
        for (filename, source) in [
            ("Entry.crepus", "button\n  span\n    {caption}"),
            ("Entry.crepus", "button\n  span\n    {caption}\n  span \"Fallback\""),
            ("Entry.vue", "<template><button v-text='caption'/></template>"),
            ("Entry.vue", "<template><button><span v-text='caption'/></button></template>"),
            ("Entry.vue", "<template><button><span v-text='caption'/><span>Fallback</span></button></template>"),
        ] {
            let ir = render_template_to_ir_with_path(source, &context, Some(Path::new(filename))).unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["label"], "", "{source}: {ir}");
            assert_eq!(ir["root"][0]["labelParts"], json!([{"kind":"expr","value":"caption"}]), "{source}: {ir}");
        }
    }
}

#[test]
fn static_empty_nested_content_keeps_existing_fallback_selection() {
    for (filename, source) in [
        (
            "Entry.crepus",
            "button\n  span\n    \"\"\n  span \"Fallback\"",
        ),
        (
            "Entry.vue",
            "<template><button><span></span><span>Fallback</span></button></template>",
        ),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let ir = serde_json::to_value(ir).unwrap();
        assert_eq!(ir["root"][0]["label"], "Fallback", "{source}: {ir}");
        assert!(ir["root"][0].get("labelParts").is_none(), "{ir}");
    }
}

#[test]
fn explicit_labels_still_override_initially_empty_nested_expressions() {
    for (filename, source) in [
        (
            "Entry.crepus",
            "button label=Explicit\n  span\n    {caption}",
        ),
        (
            "Entry.vue",
            "<template><button label='Explicit'><span v-text='caption'/></button></template>",
        ),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let ir = serde_json::to_value(ir).unwrap();
        assert_eq!(ir["root"][0]["label"], "Explicit", "{source}: {ir}");
        assert!(ir["root"][0].get("labelParts").is_none(), "{ir}");
    }
}
