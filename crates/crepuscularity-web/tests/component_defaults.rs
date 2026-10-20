use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_component_file_to_html;

#[test]
fn component_default_literals_keep_their_values_in_html() {
    let source = "+++\n[Card.defaults]\namount = 1e20\npath = 'C:\\new\\tools'\n+++\n--- Card\nspan\n  \"{amount} {path}\"";
    let html = render_component_file_to_html(source, "Card", &TemplateContext::new()).unwrap();
    assert!(
        html.contains(r"100000000000000000000 C:\new\tools"),
        "{html}"
    );
}

#[test]
fn unsupported_component_defaults_are_reported_by_the_html_renderer() {
    let source = "+++\n[Card.defaults]\nvalue = [1, 2]\n+++\n--- Card\nspan";
    let error = render_component_file_to_html(source, "Card", &TemplateContext::new()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid component default Card.value"),
        "{error}"
    );
}
#[test]
fn multiline_component_text_keeps_section_markers_and_escaped_quotes() {
    let source = "--- Card\nspan\n  \"Literal \\\" quote\n  --- Other\n  still literal\"\n--- Button\nbutton \"Real button\"";
    let ctx = crepuscularity_core::TemplateContext::new();
    let html = crepuscularity_web::render_component_file_to_html(source, "Card", &ctx).unwrap();
    assert!(html.contains("--- Other"), "{html}");
    assert!(html.contains("Literal"));
    assert!(html.contains("still literal"));
    let html = crepuscularity_web::render_component_file_to_html(source, "Button", &ctx).unwrap();
    assert!(html.contains("Real button"));
    assert!(!html.contains("--- Other"));
}
