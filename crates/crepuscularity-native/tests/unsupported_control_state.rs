use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

#[test]
fn unrepresented_control_state_attributes_are_rejected_across_frontends() {
    for attribute in ["disabled", "readonly", "readOnly"] {
        for tag in ["input", "button"] {
            for nested in [false, true] {
                let indent = if nested { "div\n  " } else { "" };
                let markup = if tag == "input" {
                    format!("<input {attribute} />")
                } else {
                    format!("<button {attribute}></button>")
                };
                let markup = if nested {
                    format!("<div>{markup}</div>")
                } else {
                    markup
                };
                for (filename, source) in [
                    ("Entry.crepus", format!("{indent}{tag} {attribute}=true")),
                    ("Entry.jsx", markup.clone()),
                    ("Entry.svelte", markup.clone()),
                    ("Entry.astro", markup.clone()),
                    ("Entry.component.html", markup.clone()),
                    ("Entry.vue", format!("<template>{markup}</template>")),
                ] {
                    let result = render_template_to_ir_with_path(
                        &source,
                        &TemplateContext::new(),
                        Some(Path::new(filename)),
                    );
                    let error =
                        result.expect_err(&format!("{filename} silently accepted {source}"));
                    let diagnostic = error.to_string().to_ascii_lowercase();
                    assert!(
                        diagnostic.contains("unsupported") || diagnostic.contains("not supported"),
                        "{diagnostic}"
                    );
                    assert!(
                        diagnostic.contains(&attribute.to_ascii_lowercase()),
                        "{diagnostic}"
                    );
                }
            }
        }
    }
}

#[test]
fn false_state_bindings_are_not_silently_discarded_either() {
    for attribute in ["disabled", "readonly", "readOnly"] {
        let source = format!("input {attribute}=false");
        let error = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("Entry.crepus")),
        )
        .expect_err("unsupported attribute presence must remain explicit");
        assert!(
            error
                .to_string()
                .to_ascii_lowercase()
                .contains(&attribute.to_ascii_lowercase()),
            "{error}"
        );
    }
}

#[test]
fn supported_required_controls_still_lower_normally() {
    for (filename, source) in [
        ("Entry.crepus", "input required=true"),
        ("Entry.jsx", "<input required />"),
        ("Entry.svelte", "<input required />"),
        ("Entry.astro", "<input required />"),
        ("Entry.component.html", "<input required />"),
        ("Entry.vue", "<template><input required /></template>"),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let json = serde_json::to_value(ir).unwrap();
        assert_eq!(
            json["root"][0]["style"]["required"], true,
            "{filename}: {json}"
        );
    }
}

#[test]
fn metadata_only_children_cannot_hide_control_state() {
    for source in [
        "picker\n  option disabled=true value=one label=One",
        "button\n  span readonly=true label=Label",
        "tabs\n  tab disabled=true label=One",
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Entry.crepus")),
        )
        .expect_err("flattened metadata must preserve the rejection boundary");
        assert!(
            error
                .to_string()
                .contains("unsupported View IR control state"),
            "{error}"
        );
    }
}
