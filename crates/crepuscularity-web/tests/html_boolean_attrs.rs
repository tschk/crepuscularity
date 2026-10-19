use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_template_to_html;

#[test]
fn boolean_html_bindings_use_presence_semantics() {
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", true);
    ctx.set("disabled", false);

    let template = r#"
button disabled={disabled} checked={enabled} inert={disabled} hidden={disabled} aria-disabled={disabled} contenteditable={disabled}
  "Save"
"#;

    let html = render_template_to_html(template, &ctx).unwrap();
    assert_eq!(
        html,
        r#"<button checked="" aria-disabled="false" contenteditable="false">Save</button>"#
    );

    ctx.set("enabled", false);
    ctx.set("disabled", true);

    let html = render_template_to_html(template, &ctx).unwrap();
    assert_eq!(
        html,
        r#"<button disabled="" inert="" hidden="hidden" aria-disabled="true" contenteditable="true">Save</button>"#
    );
}

#[test]
fn boolean_html_bindings_preserve_user_supplied_string_values() {
    let template = r#"
button disabled="false"
  "Still disabled under HTML presence semantics"
"#;

    let html = render_template_to_html(template, &TemplateContext::new()).unwrap();
    assert_eq!(
        html,
        r#"<button disabled="false">Still disabled under HTML presence semantics</button>"#
    );
}
