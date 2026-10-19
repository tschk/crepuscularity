use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, tag: &str, property: &str, value: Option<&str>) -> String {
    let attribute = match value {
        None if filename.ends_with(".crepus") => format!("{property}=true"),
        None => property.into(),
        Some(value) if filename.ends_with(".vue") => format!(":{property}=\"{value}\""),
        Some(value) if filename.ends_with(".ng") => format!("[{property}]=\"{value}\""),
        Some(value) => format!("{property}={{{value}}}"),
    };
    if filename.ends_with(".crepus") {
        return format!("picker\n  {tag} value=one label=One {attribute}");
    }
    let markup = format!("<picker><{tag} value='one' {attribute}>One</{tag}></picker>");
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup
    }
}

#[test]
fn picker_metadata_cannot_silently_drop_supplied_control_state() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for tag in ["option", "span", "button"] {
            for property in ["disabled", "readonly"] {
                for value in [None, Some("false"), Some("null")] {
                    let source = source(filename, tag, property, value);
                    let error = render_template_to_ir_with_path(
                        &source,
                        &TemplateContext::new(),
                        Some(Path::new(filename)),
                    )
                    .expect_err(&format!("metadata state was dropped: {filename} {source}"));
                    assert!(
                        error
                            .to_string()
                            .contains("unsupported View IR control state"),
                        "{filename}: {error}"
                    );
                    assert!(error.to_string().contains(property), "{filename}: {error}");
                }
            }
        }
    }
}

#[test]
fn ordinary_option_metadata_remains_supported() {
    for (filename, source) in [
        ("Entry.crepus", "picker\n  option value=one label=One"),
        (
            "Entry.jsx",
            "<picker><option value='one'>One</option></picker>",
        ),
        (
            "Entry.svelte",
            "<picker><option value='one'>One</option></picker>",
        ),
        (
            "Entry.vue",
            "<template><picker><option value='one'>One</option></picker></template>",
        ),
        (
            "Entry.astro",
            "<picker><option value='one'>One</option></picker>",
        ),
        (
            "Entry.ng",
            "<picker><option value='one'>One</option></picker>",
        ),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["options"][0],
            serde_json::json!({"value":"one","label":"One"}),
            "{filename}"
        );
    }
}
