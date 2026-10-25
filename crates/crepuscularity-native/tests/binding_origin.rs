use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use serde_json::{json, Value};
use std::path::Path;

fn render(source: &str, context: &TemplateContext) -> Value {
    serde_json::to_value(
        render_template_to_ir_with_path(source, context, Some(Path::new("view.ng"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn attribute_presence_differs_from_expression_booleans() {
    for (value, present) in [("false", true), ("0", true), ("''", true), ("null", false)] {
        let ir = render(
            &format!(
                "<checkbox [attr.checked]=\"{value}\"/><file-picker [attr.multiple]=\"{value}\"/>"
            ),
            &TemplateContext::new(),
        );
        assert_eq!(ir["root"][0]["checked"], present, "{ir}");
        assert_eq!(ir["root"][1]["multiple"], present, "{ir}");
    }
    let ir = render(
        "<checkbox [checked]=\"false\"/><file-picker [multiple]=\"false\"/>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["checked"], false);
    // Existing portable picker semantics are presence-based even for `false`.
    assert_eq!(ir["root"][1]["multiple"], true);
}

#[test]
fn attribute_bind_and_interval_do_not_select_model_behavior() {
    let ir = render(
        "<input [attr.bind]=\"'secret.path'\"/><checkbox [attr.bind]=\"'secret.flag'\"/><select [attr.bind]=\"'secret.choice'\"><option value=\"a\">A</option></select><slot-rotate [attr.interval]=\"99\">one<!--split-->two</slot-rotate>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["bind"], "");
    assert!(ir["root"][1].get("bind").is_none());
    assert_eq!(ir["root"][2]["bind"], "");
    assert_eq!(ir["root"][3]["intervalMs"], 3200);
    let ir = render(
        "<input [bind]=\"model.title\"/><slot-rotate [interval]=\"99\">one<!--split-->two</slot-rotate>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["bind"], "model.title");
    assert_eq!(ir["root"][1]["intervalMs"], 99);
}

#[test]
fn attribute_classes_are_not_interpolated_again_and_hidden_uses_presence() {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "{literal} flex");
    ctx.set("literal", "wrong");
    let ir = render(
        "<div [attr.class]=\"classes\" [attr.id]=\"false\" [attr.hidden]=\"false\"></div><div class=\"hidden\" [attr.hidden]=\"null\"></div><div [attr.id]=\"null\" [attr.class]=\"null\" [attr.hidden]=\"null\"></div>",
        &ctx,
    );
    assert_eq!(
        ir["root"][0]["style"]["classes"],
        json!(["{literal}", "flex"])
    );
    assert_eq!(ir["root"][0]["style"]["id"], "false");
    assert_eq!(ir["root"][0]["style"]["hidden"], true);
    assert_eq!(ir["root"][1]["style"]["hidden"], true);
    assert!(ir["root"][2]["style"].get("id").is_none());
    assert!(ir["root"][2]["style"].get("hidden").is_none());
}

#[test]
fn scalar_attribute_data_preserves_quotes_whitespace_and_null_omission() {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "url",
        "  https://example.invalid/?q=\"quoted\"&x={literal}  ",
    );
    ctx.set("placeholder", " \"quoted\" {literal} ");
    let ir = render(
        "<a [attr.href]=\"url\" [attr.target]=\"null\" [attr.rel]=\"false\">Link</a><img [attr.src]=\"url\" [attr.alt]=\"false\" [attr.placeholder]=\"null\"/><iframe [attr.src]=\"url\"></iframe><input [attr.placeholder]=\"placeholder\" [attr.type]=\"'password'\"/><dropzone [attr.accept]=\"null\"></dropzone>",
        &ctx,
    );
    let url = "  https://example.invalid/?q=\"quoted\"&x={literal}  ";
    assert_eq!(ir["root"][0]["href"], url);
    assert!(ir["root"][0].get("target").is_none());
    assert_eq!(ir["root"][0]["rel"], "false");
    assert_eq!(ir["root"][1]["src"], url);
    assert_eq!(ir["root"][1]["alt"], "false");
    assert!(ir["root"][1].get("placeholder").is_none());
    assert_eq!(ir["root"][2]["src"], url);
    assert_eq!(ir["root"][3]["placeholder"], " \"quoted\" {literal} ");
    assert_eq!(ir["root"][3]["secure"], true);
    assert!(ir["root"][4].get("accept").is_none());
}

#[test]
fn attribute_option_tab_and_numeric_values_remain_data() {
    let mut ctx = TemplateContext::new();
    ctx.set("choice", "chosen");
    let ir = render(
        "<select><option [attr.value]=\"choice\">One</option><option [attr.value]=\"false\">Two</option><option [attr.value]=\"null\">Fallback</option></select><tabs><tab [attr.value]=\"choice\" [attr.label]=\"'Label'\" [attr.icon]=\"null\"></tab></tabs><slider [attr.value]=\"7\" [attr.min]=\"2\" [attr.max]=\"12\" [attr.step]=\"2\"/><file-picker [attr.accept]=\"'image/*, text/plain'\"/>",
        &ctx,
    );
    assert_eq!(ir["root"][0]["options"][0]["value"], "chosen");
    assert_eq!(ir["root"][0]["options"][1]["value"], "false");
    assert_eq!(ir["root"][0]["options"][2]["value"], "Fallback");
    assert_eq!(ir["root"][1]["tabs"][0]["value"], "chosen");
    assert_eq!(ir["root"][1]["tabs"][0]["label"], "Label");
    assert!(ir["root"][1]["tabs"][0].get("icon").is_none());
    assert_eq!(ir["root"][2]["value"], 7.0);
    assert_eq!(ir["root"][2]["min"], 2.0);
    assert_eq!(ir["root"][2]["max"], 12.0);
    assert_eq!(ir["root"][2]["step"], 2.0);
    assert_eq!(ir["root"][3]["accept"], json!(["image/*", "text/plain"]));
}

#[test]
fn ordinary_conditions_and_conditional_classes_keep_expression_semantics() {
    let mut ctx = TemplateContext::new();
    ctx.set("visible", false);
    ctx.set("active", true);
    ctx.set("classes", "{unresolved}");
    let ir = render(
        "<div *ngIf=\"visible\" [class.active]=\"active\"></div><div [attr.class]=\"classes\"></div>",
        &ctx,
    );
    assert_eq!(ir["root"][0]["kind"], "if");
    assert_eq!(ir["root"][0]["condition"], "visible");
    assert_eq!(
        ir["root"][0]["thenChildren"][0]["style"]["classes"],
        json!(["active"])
    );
    assert_eq!(ir["root"][1]["style"]["classes"], json!(["{unresolved}"]));
}

#[test]
fn html_attribute_names_match_without_changing_ordinary_property_case() {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "{literal} flex");
    ctx.set("literal", "wrong");
    let ir = render(
        "<div [attr.CLASS]=\"classes\" [attr.ID]=\"'identity'\" [attr.HIDDEN]=\"false\"></div><checkbox [attr.CHECKED]=\"false\"/><input [attr.TYPE]=\"'password'\" [attr.PLACEHOLDER]=\"false\"/><file-picker [attr.MULTIPLE]=\"false\"/><checkbox [CHECKED]=\"true\"/><input [TYPE]=\"'password'\"/>",
        &ctx,
    );
    assert_eq!(
        ir["root"][0]["style"]["classes"],
        json!(["{literal}", "flex"])
    );
    assert_eq!(ir["root"][0]["style"]["id"], "identity");
    assert_eq!(ir["root"][0]["style"]["hidden"], true);
    assert_eq!(ir["root"][1]["checked"], true);
    assert_eq!(ir["root"][2]["secure"], true);
    assert_eq!(ir["root"][2]["placeholder"], "false");
    assert_eq!(ir["root"][3]["multiple"], true);
    assert_eq!(ir["root"][4]["checked"], false);
    assert_eq!(ir["root"][5]["secure"], false);

    let ir = render(
        "<div [attr.HIDDEN]=\"null\" [attr.ID]=\"null\" [attr.CLASS]=\"null\"></div><checkbox [attr.CHECKED]=\"null\"/><file-picker [attr.MULTIPLE]=\"null\"/>",
        &ctx,
    );
    assert!(ir["root"][0]["style"].get("id").is_none());
    assert!(ir["root"][0]["style"].get("hidden").is_none());
    assert_eq!(ir["root"][1]["checked"], false);
    assert_eq!(ir["root"][2]["multiple"], false);
}

#[test]
fn null_attribute_option_values_use_the_same_text_default_as_absence() {
    let ir = render(
        "<select><option [attr.value]=\"null\">  Hello World! </option><option>  Hello World! </option></select>",
        &TemplateContext::new(),
    );
    assert_eq!(ir["root"][0]["options"][0], ir["root"][0]["options"][1]);
    assert_eq!(ir["root"][0]["options"][0]["value"], "Hello World!");
}
