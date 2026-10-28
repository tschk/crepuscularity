use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, kind: Option<&str>) -> String {
    if filename.ends_with(".crepus") {
        return kind
            .map(|kind| format!("button type={kind} label=Action"))
            .unwrap_or_else(|| "button label=Action".into());
    }
    let attribute = kind
        .map(|kind| format!(" type=\"{kind}\""))
        .unwrap_or_default();
    let markup = format!("<button{attribute}>Action</button>");
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup
    }
}

#[test]
fn unsupported_submit_and_reset_semantics_are_not_silently_changed() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for kind in ["submit", "reset"] {
            let source = source(filename, Some(kind));
            let error = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("unsupported View IR button type"),
                "{filename}: {error}"
            );
        }
    }
}

#[test]
fn ordinary_and_explicit_button_types_keep_existing_action_nodes() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for kind in [None, Some("button")] {
            let source = source(filename, kind);
            let ir = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            let ir = serde_json::to_value(ir).unwrap();
            assert_eq!(ir["root"][0]["kind"], "button", "{filename}: {ir}");
            assert_eq!(ir["root"][0]["label"], "Action", "{filename}: {ir}");
        }
    }
}

#[test]
fn duplicate_button_type_attributes_are_rejected() {
    let error = render_template_to_ir_with_path(
        "<button type='button' type='submit'>Action</button>",
        &TemplateContext::new(),
        Some(Path::new("Entry.jsx")),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("duplicate type binding"),
        "{error}"
    );
}
