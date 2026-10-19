use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, tag: &str, prop: &str, value: &str) -> String {
    if filename.ends_with(".crepus") {
        return format!("{tag} {prop}={{{value}}}");
    }
    let attr = if filename.ends_with(".vue") {
        format!(":{prop}=\"{value}\"")
    } else if filename.ends_with(".ng") {
        format!("[{prop}]=\"{value}\"")
    } else {
        format!("{prop}={{{value}}}")
    };
    let markup = format!("<{tag} {attr}/>");
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup
    }
}

#[test]
fn null_missing_and_typed_false_do_not_enable_control_booleans_in_six_frontends() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for (tag, property) in [
            ("checkbox", "checked"),
            ("toggle", "checked"),
            ("file-picker", "multiple"),
        ] {
            for value in ["null", "missing.enabled", "false"] {
                let source = source(filename, tag, property, value);
                let ir = render_template_to_ir_with_path(
                    &source,
                    &TemplateContext::new(),
                    Some(Path::new(filename)),
                )
                .unwrap();
                let ir = serde_json::to_value(ir).unwrap();
                assert_eq!(ir["root"][0][property], false, "{filename} {source}: {ir}");
            }
        }
    }
}

#[test]
fn bare_and_empty_control_booleans_remain_enabled() {
    for (tag, property) in [
        ("checkbox", "checked"),
        ("toggle", "checked"),
        ("file-picker", "multiple"),
    ] {
        for attr in [property.to_string(), format!("{property}=\"\"")] {
            let source = format!("<{tag} {attr}/>");
            let ir = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new("Entry.jsx")),
            )
            .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0][property], true, "{source}: {ir}");
        }
    }
}

#[test]
fn explicit_null_multiple_overrides_custom_class_fallback() {
    for (source, expected) in [
        ("file-picker multiple={null} class=multiple", false),
        ("file-picker class=multiple", true),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Entry.crepus")),
        )
        .unwrap();
        let ir = serde_json::to_value(ir).unwrap();
        assert_eq!(ir["root"][0]["multiple"], expected, "{source}: {ir}");
    }
}
