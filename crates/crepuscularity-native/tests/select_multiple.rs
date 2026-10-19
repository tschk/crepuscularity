use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, attribute: Option<(&str, bool)>) -> String {
    let attribute = match attribute {
        None => String::new(),
        Some(("bare", _)) => "multiple".into(),
        Some((value, true)) => format!("multiple=\"{value}\""),
        Some((value, false)) if filename.ends_with(".crepus") => format!("multiple={{{value}}}"),
        Some((value, false)) if filename.ends_with(".vue") => format!(":multiple=\"{value}\""),
        Some((value, false)) if filename.ends_with(".ng") => format!("[multiple]=\"{value}\""),
        Some((value, false)) => format!("multiple={{{value}}}"),
    };
    if filename.ends_with(".crepus") {
        let attribute = if attribute == "multiple" {
            "multiple=true"
        } else {
            &attribute
        };
        return format!("select {attribute}\n  option value=one label=One");
    }
    let markup = format!("<select {attribute}><option value='one'>One</option></select>");
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup
    }
}

#[test]
fn standard_select_multiple_presence_cannot_silently_become_single_selection() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for attribute in [Some(("bare", true)), Some(("false", true))] {
            let source = source(filename, attribute);
            let error = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("View IR pickers do not support multiple selection"),
                "{filename}: {error}"
            );
        }
        for attribute in [None, Some(("false", false)), Some(("null", false))] {
            let source = source(filename, attribute);
            let ir = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["kind"], "picker", "{filename}: {ir}");
            assert_eq!(ir["root"][0]["options"][0]["value"], "one");
        }
    }
}

#[test]
fn custom_picker_boolean_coercion_and_duplicate_validation_remain_explicit() {
    let context = TemplateContext::new();
    let error =
        render_template_to_ir_with_path("picker multiple=true", &context, None).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("View IR pickers do not support multiple selection"),
        "{error}"
    );
    let ir = render_template_to_ir_with_path("picker multiple=\"false\"", &context, None).unwrap();
    assert_eq!(
        serde_json::to_value(ir).unwrap()["root"][0]["kind"],
        "picker"
    );
    let error = render_template_to_ir_with_path(
        "<select multiple={false} multiple={null}/>",
        &context,
        Some(Path::new("Entry.jsx")),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("duplicate multiple binding"),
        "{error}"
    );
}
