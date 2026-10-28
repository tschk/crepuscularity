use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn wrap(filename: &str, markup: &str) -> String {
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup.into()
    }
}

#[test]
fn optgroups_are_rejected_instead_of_dropping_their_options() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        let source = if filename.ends_with(".crepus") {
            "select\n  optgroup label=Group\n    option value=one label=One".into()
        } else {
            wrap(filename, "<select><optgroup label='Group'><option value='one'>One</option></optgroup></select>")
        };
        let error = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported View IR picker child"),
            "{filename}: {error}"
        );
    }
}

#[test]
fn selected_presence_is_rejected_but_false_null_and_absence_keep_options() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for (value, expression, rejected) in [
            (Some("true"), true, true),
            (Some("false"), false, true),
            (Some("false"), true, false),
            (Some("null"), true, false),
            (None, false, false),
        ] {
            let attribute = match value {
                None => String::new(),
                Some("true") if filename.ends_with(".crepus") => "selected=true".into(),
                Some("true") => "selected".into(),
                Some(value) if !expression => format!("selected=\"{value}\""),
                Some(value) if filename.ends_with(".vue") => format!(":selected=\"{value}\""),
                Some(value) if filename.ends_with(".ng") => format!("[selected]=\"{value}\""),
                Some(value) => format!("selected={{{value}}}"),
            };
            let source = if filename.ends_with(".crepus") {
                format!("select\n  option value=one label=One {attribute}")
            } else {
                wrap(
                    filename,
                    &format!("<select><option value='one' {attribute}>One</option></select>"),
                )
            };
            let result = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            );
            if rejected {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("selected defaults"),
                    "{filename}: {source}"
                );
            } else {
                let ir = serde_json::to_value(result.unwrap()).unwrap();
                assert_eq!(
                    ir["root"][0]["options"][0],
                    serde_json::json!({"value":"one","label":"One"}),
                    "{filename}: {ir}"
                );
            }
        }
    }
}

#[test]
fn custom_picker_child_aliases_remain_valid_and_unknown_children_are_explicit_errors() {
    for tag in ["span", "button", "option"] {
        let source = format!("<picker><{tag} value='one'>One</{tag}></picker>");
        let ir = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("Entry.jsx")),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["options"][0]["value"],
            "one"
        );
    }
    for (source, expected) in [
        (
            "<picker><div>Lost</div></picker>",
            "unsupported View IR picker child",
        ),
        (
            "<select><option selected={false} selected={false}>One</option></select>",
            "duplicate selected binding",
        ),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Entry.jsx")),
        )
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}
