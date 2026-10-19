use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_native::{
    emit_react_component, render_template_to_ir, render_template_to_ir_with_path,
};
use std::path::Path;

#[test]
fn default_aliases_preserve_static_values_in_all_frontends() {
    for alias in ["defaultValue", "default-value"] {
        for (filename, source) in [
            ("Entry.crepus", format!("input {alias}=\"seed & exact\"")),
            ("Entry.jsx", format!("<input {alias}=\"seed & exact\"/>")),
            ("Entry.svelte", format!("<input {alias}=\"seed & exact\"/>")),
            ("Entry.astro", format!("<input {alias}=\"seed & exact\"/>")),
            (
                "Entry.vue",
                format!("<template><input {alias}=\"seed & exact\"/></template>"),
            ),
            ("Entry.ng", format!("<input {alias}=\"seed & exact\"/>")),
        ] {
            let ir = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            let canonical = render_template_to_ir_with_path(
                &source.replace(alias, "value"),
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            assert_eq!(ir, canonical, "{filename}: {source}");
            let json = serde_json::to_value(ir).unwrap();
            assert_eq!(
                json["root"][0]["style"]["formValue"], "seed & exact",
                "{filename}: {source}"
            );
        }
    }
}

#[test]
fn frontend_property_defaults_evaluate_context_snapshots() {
    let mut ctx = TemplateContext::new();
    ctx.set("seed", "dynamic & exact");
    for (filename, source) in [
        ("Entry.crepus", "input defaultValue={seed}"),
        ("Entry.jsx", "<input defaultValue={seed}/>"),
        ("Entry.svelte", "<input defaultValue={seed}/>"),
        ("Entry.astro", "<input defaultValue={seed}/>"),
        (
            "Entry.vue",
            "<template><input :defaultValue=\"seed\"/></template>",
        ),
        ("Entry.ng", "<input [defaultValue]=\"seed\"/>"),
    ] {
        let ir = render_template_to_ir_with_path(source, &ctx, Some(Path::new(filename))).unwrap();
        let canonical = render_template_to_ir_with_path(
            &source.replace("defaultValue", "value"),
            &ctx,
            Some(Path::new(filename)),
        )
        .unwrap();
        assert_eq!(ir, canonical, "{filename}");
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["style"]["formValue"],
            "dynamic & exact",
            "{filename}"
        );
    }
}

#[test]
fn aliases_evaluate_snapshots_for_text_and_checkbox_submission_values() {
    let mut ctx = TemplateContext::new();
    for tag in [
        "input",
        "textfield",
        "textinput",
        "textarea",
        "checkbox",
        "input type=checkbox",
    ] {
        for alias in ["value", "defaultValue", "default-value"] {
            for (value, expected) in [
                (TemplateValue::Str("".into()), ""),
                (TemplateValue::Null, ""),
                (TemplateValue::Int(42), "42"),
                (
                    TemplateValue::Str(" quoted & exact ".into()),
                    " quoted & exact ",
                ),
            ] {
                ctx.set("seed", value);
                let ir = render_template_to_ir(&format!("{tag} {alias}={{seed}}"), &ctx).unwrap();
                let json = serde_json::to_value(ir).unwrap();
                assert_eq!(
                    json["root"][0]["style"]["formValue"], expected,
                    "{tag} {alias}"
                );
                if tag.contains("checkbox") {
                    assert_eq!(json["root"][0]["checked"], false);
                }
            }
            assert!(render_template_to_ir(&format!("{tag} {alias}={{1 +}}"), &ctx).is_err());
        }
    }
    let ir = render_template_to_ir("input bind=live defaultValue=\"seed\"", &ctx).unwrap();
    let json = serde_json::to_value(&ir).unwrap();
    assert_eq!(json["root"][0]["bind"], "live");
    assert_eq!(json["root"][0]["style"]["formValue"], "seed");
    let react = emit_react_component(&ir, "Panel");
    assert!(
        react.contains("scope?.live") && !react.contains("\"seed\""),
        "{react}"
    );
}

#[test]
fn conflicting_defaults_duplicates_file_values_and_textarea_bodies_error() {
    for tag in ["input", "checkbox", "textarea"] {
        for attrs in [
            "value=\"a\" defaultValue=\"b\"",
            "value=\"a\" default-value=\"b\"",
            "defaultValue=\"a\" default-value=\"b\"",
        ] {
            let error = render_template_to_ir(&format!("{tag} {attrs}"), &TemplateContext::new())
                .unwrap_err()
                .to_string();
            assert!(error.contains("cannot combine"), "{error}");
        }
        for alias in ["value", "defaultValue", "default-value"] {
            let error = render_template_to_ir(
                &format!("{tag} {alias}=\"a\" {alias}=\"b\""),
                &TemplateContext::new(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(&format!("duplicate {alias} binding")),
                "{error}"
            );
        }
    }
    for alias in ["value", "defaultValue", "default-value"] {
        assert!(render_template_to_ir(
            &format!("input type=file {alias}=\"seed\""),
            &TemplateContext::new()
        )
        .is_err());
        assert!(render_template_to_ir(
            &format!("textarea {alias}=\"seed\"\n  \"body\""),
            &TemplateContext::new()
        )
        .is_err());
    }
}
