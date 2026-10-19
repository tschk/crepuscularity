use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn markup_source(filename: &str, markup: &str) -> String {
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup.to_string()
    }
}

#[test]
fn duplicate_evaluated_and_raw_control_bindings_are_rejected_in_every_frontend() {
    for (tag, property, first, second) in [
        ("checkbox", "checked", "true", "false"),
        ("file-picker", "multiple", "true", "false"),
        ("input", "placeholder", "'First'", "'Second'"),
        ("input", "bind", "user.first", "user.second"),
    ] {
        for filename in [
            "Entry.crepus",
            "Entry.jsx",
            "Entry.svelte",
            "Entry.vue",
            "Entry.astro",
            "Entry.component.html",
        ] {
            let source = if filename.ends_with(".crepus") {
                format!("{tag} {property}={first} {property}={second}")
            } else {
                let attributes = if filename.ends_with(".vue") {
                    format!("{property}=\"{first}\" :{property}=\"{second}\"")
                } else if filename.ends_with(".component.html") {
                    format!("{property}=\"{first}\" [{property}]=\"{second}\"")
                } else {
                    format!("{property}=\"{first}\" {property}=\"{second}\"")
                };
                markup_source(filename, &format!("<{tag} {attributes} />"))
            };
            let error = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .expect_err(&format!("{filename} accepted {source}"));
            let diagnostic = error.to_string();
            assert!(
                diagnostic.contains(&format!("duplicate {property} binding")),
                "{filename}: {diagnostic}"
            );
        }
    }
}

#[test]
fn duplicate_option_values_and_image_sources_are_not_flattened_away() {
    for (source, property) in [
        ("picker\n  option value=one value=two label=Choice", "value"),
        ("img src=first.png src=second.png", "src"),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Entry.crepus")),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("duplicate {property} binding")),
            "{error}"
        );
    }
}

#[test]
fn single_false_empty_and_dotted_raw_bindings_keep_their_existing_values() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.component.html",
    ] {
        let source = if filename.ends_with(".crepus") {
            "checkbox checked=false\nfile-picker multiple=false\ninput bind=user.name placeholder=\"\"".to_string()
        } else {
            let attributes = if filename.ends_with(".vue") {
                ":checked=\"false\""
            } else if filename.ends_with(".component.html") {
                "[checked]=\"false\""
            } else {
                "checked={false}"
            };
            let multiple = if filename.ends_with(".vue") {
                ":multiple=\"false\""
            } else if filename.ends_with(".component.html") {
                "[multiple]=\"false\""
            } else {
                "multiple={false}"
            };
            markup_source(filename, &format!("<checkbox {attributes}/><file-picker {multiple}/><input bind=\"user.name\" placeholder=\"\"/>"))
        };
        let ir = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let value = serde_json::to_value(ir).unwrap();
        assert_eq!(value["root"][0]["checked"], false, "{filename}: {value}");
        assert_eq!(value["root"][1]["multiple"], false, "{filename}: {value}");
        assert_eq!(value["root"][2]["bind"], "user.name", "{filename}: {value}");
        assert_eq!(value["root"][2]["placeholder"], "", "{filename}: {value}");
    }
}
