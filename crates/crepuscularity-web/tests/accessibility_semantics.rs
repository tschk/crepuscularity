use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_template_to_html;

#[test]
fn explicit_label_for_associates_with_control_id() {
    let template = r#"
main
  label for="email" "Email address"
  input #email type="email" autocomplete="email"
"#;

    let html = render_template_to_html(template, &TemplateContext::new()).unwrap();
    assert!(
        html.contains(r#"<label for="email">Email address</label>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<input id="email" type="email" autocomplete="email" />"#),
        "{html}"
    );
}
