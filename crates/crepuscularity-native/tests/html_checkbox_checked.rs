use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, value: Option<&str>, literal: bool) -> String {
    let attribute = match value {
        None => String::new(),
        Some(value) if literal => format!("checked=\"{value}\""),
        Some(value) if filename.ends_with(".crepus") => format!("checked={{{value}}}"),
        Some(value) if filename.ends_with(".vue") => format!(":checked=\"{value}\""),
        Some(value) if filename.ends_with(".ng") => format!("[checked]=\"{value}\""),
        Some(value) => format!("checked={{{value}}}"),
    };
    if filename.ends_with(".crepus") {
        return format!("input type=checkbox {attribute}");
    }
    let markup = format!("<input type='checkbox' {attribute}/>");
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup
    }
}

#[test]
fn standard_checkbox_checked_uses_html_presence_in_all_frontends() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for (value, literal, expected) in [
            (Some("false"), true, true),
            (Some("checked"), true, true),
            (Some("false"), false, false),
            (Some("null"), false, false),
            (None, false, false),
        ] {
            let source = source(filename, value, literal);
            let ir = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["kind"], "checkbox", "{filename}: {ir}");
            assert_eq!(
                ir["root"][0]["checked"], expected,
                "{filename} {source}: {ir}"
            );
        }
    }
}

#[test]
fn custom_checkbox_false_string_keeps_existing_boolean_coercion() {
    for (filename, source) in [
        ("Entry.crepus", "checkbox checked=\"false\""),
        ("Entry.jsx", "<checkbox checked='false'/>"),
        ("Entry.svelte", "<checkbox checked='false'/>"),
        (
            "Entry.vue",
            "<template><checkbox checked='false'/></template>",
        ),
        ("Entry.astro", "<checkbox checked='false'/>"),
        ("Entry.ng", "<checkbox checked='false'/>"),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["checked"],
            false,
            "{filename}"
        );
    }
}
