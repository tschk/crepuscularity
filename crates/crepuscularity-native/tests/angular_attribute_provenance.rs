use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use serde_json::{json, Value};
use std::path::Path;

fn render(source: &str, ctx: &TemplateContext) -> Value {
    serde_json::to_value(
        render_template_to_ir_with_path(source, ctx, Some(Path::new("view.ng"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn angular_attributes_coerce_scalar_values_and_preserve_presence() {
    let ir = render("<input type=\"checkbox\" [attr.checked]=\"false\" [attr.required]=\"false\" [attr.name]=\"false\"/>", &TemplateContext::new());
    assert_eq!(ir["root"][0]["checked"], true);
    assert_eq!(ir["root"][0]["style"]["required"], true);
    assert_eq!(ir["root"][0]["style"]["formName"], "false");
    let ir = render("<input type=\"checkbox\" [attr.checked]=\"null\" [attr.required]=\"null\" [attr.name]=\"null\"/>", &TemplateContext::new());
    assert_eq!(ir["root"][0]["checked"], false);
    assert!(ir["root"][0]["style"].get("required").is_none());
    assert!(ir["root"][0]["style"].get("formName").is_none());
}

#[test]
fn angular_attribute_urls_are_snapshots_and_null_ids_are_absent() {
    let mut ctx = TemplateContext::new();
    ctx.set("destination", false);
    let ir = render(
        "<a [attr.href]=\"destination\" [attr.id]=\"null\" [attr.aria-label]=\"null\">link</a>",
        &ctx,
    );
    assert_eq!(ir["root"][0]["href"], "false");
    assert!(ir["root"][0].get("hrefBind").is_none());
    assert!(ir["root"][0]["style"].get("id").is_none());
    assert!(ir["root"][0]["style"].get("ariaLabel").is_none());
}

#[test]
fn angular_attribute_paths_and_textarea_values_do_not_become_live_control_state() {
    let ir = render(
        "<textarea [attr.value]=\"'ignored'\" [attr.bind]=\"'model.title'\">body</textarea>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["bind"], "");
    assert_eq!(ir["root"][0]["style"]["formValue"], "body");
    let ir = render(
        "<input [attr.bind]=\"'model.title'\" [attr.value]=\"false\"/>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["bind"], "");
    assert_eq!(ir["root"][0]["style"]["formValue"], "false");
    let ir = render("<select [attr.value]=\"'ignored'\" [attr.bind]=\"'model.choice'\"><option value=\"one\">One</option></select>", &TemplateContext::new());
    assert_eq!(ir["root"][0]["bind"], "");
    let ir = render(
        "<input type=\"file\" [attr.bind]=\"'model.files'\" [attr.value]=\"'ignored'\"/>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["kind"], "filePicker");
}

#[test]
fn angular_attribute_classes_are_literal_strings() {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "{literal} flex");
    ctx.set("literal", "injected");
    let ir = render("<div [attr.class]=\"classes\"></div>", &ctx);
    assert_eq!(
        ir["root"][0]["style"]["classes"],
        json!(["{literal}", "flex"])
    );
}

#[test]
fn angular_attribute_interval_does_not_select_rotation_behavior() {
    let ir = render(
        "<slot-rotate [attr.interval]=\"99\">one<!--split-->two</slot-rotate>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["intervalMs"], 3200);
    assert_eq!(ir["root"][0]["phrases"], json!(["one", "two"]));
}

#[test]
fn angular_attribute_hidden_uses_presence_and_null_keeps_existing_style() {
    let ir = render(
        "<div [attr.hidden]=\"false\"></div>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["style"]["hidden"], true);
    let ir = render(
        "<div [attr.hidden]=\"null\"></div>",
        &TemplateContext::new(),
    );
    assert!(ir["root"][0]["style"].get("hidden").is_none());
    let ir = render(
        "<div class=\"hidden\" [attr.hidden]=\"null\"></div>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["style"]["hidden"], true);
}

#[test]
fn angular_null_unsupported_state_attributes_are_omitted_even_in_metadata() {
    for name in ["disabled", "readonly"] {
        for source in [
            format!("<input [attr.{name}]=\"null\"/>"),
            format!("<button><span [attr.{name}]=\"null\">Label</span></button>"),
        ] {
            render(&source, &TemplateContext::new());
        }
        for source in [
            format!("<input [attr.{name}]=\"false\"/>"),
            format!("<button><span [attr.{name}]=\"false\">Label</span></button>"),
        ] {
            let error = render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new("view.ng")),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("unsupported View IR control state"),
                "{error}"
            );
        }
    }
}

#[test]
fn angular_attribute_default_aliases_do_not_initialize_native_controls() {
    for alias in ["defaultValue", "default-value"] {
        for tag in ["input", "textarea"] {
            let closing = if tag == "input" { "/>" } else { "></textarea>" };
            let ir = render(
                &format!("<{tag} [attr.{alias}]=\"'ignored'\"{closing}"),
                &TemplateContext::new(),
            );
            assert!(ir["root"][0]["style"].get("formValue").is_none(), "{ir}");
            let ir = render(
                &format!("<{tag} [{alias}]=\"'ordinary'\"{closing}"),
                &TemplateContext::new(),
            );
            assert_eq!(ir["root"][0]["style"]["formValue"], "ordinary", "{ir}");
        }
        let ir = render(
            &format!("<input [attr.{alias}]=\"'ignored'\" [attr.value]=\"'actual'\"/>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["style"]["formValue"], "actual");
        let ir = render(
            &format!("<textarea [attr.{alias}]=\"'ignored'\" [value]=\"'actual'\"></textarea>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["style"]["formValue"], "actual");
    }
    for alias in ["defaultChecked", "default-checked"] {
        let ir = render(
            &format!("<input type=\"checkbox\" [attr.{alias}]=\"true\"/>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["checked"], false);
        let ir = render(
            &format!("<input type=\"checkbox\" [{alias}]=\"true\"/>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["checked"], true);
        let ir = render(
            &format!("<input type=\"checkbox\" [attr.{alias}]=\"true\" [checked]=\"false\"/>"),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["checked"], false);
        let ir = render(
            &format!(
                "<input type=\"checkbox\" [attr.{alias}]=\"false\" [attr.checked]=\"false\"/>"
            ),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["checked"], true);
    }
}
