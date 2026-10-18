use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html;

fn render(tpl: &str) -> String {
    render_template_to_html(tpl, &TemplateContext::new()).unwrap()
}

#[test]
fn safe_href_is_kept() {
    let html = render("a href=\"/docs\"\n  \"Docs\"\n");
    assert!(html.contains(r#"href="/docs""#), "{html}");
}

#[test]
fn javascript_href_is_dropped() {
    let html = render("a href=\"javascript:alert(1)\"\n  \"x\"\n");
    assert!(!html.contains("javascript:"), "{html}");
    assert!(!html.contains("href="), "{html}");
}

#[test]
fn javascript_scheme_is_matched_past_whitespace_and_case() {
    let html = render("a href=\"  JaVaScRiPt:alert(1)\"\n  \"x\"\n");
    assert!(!html.to_ascii_lowercase().contains("javascript:"), "{html}");
}

#[test]
fn javascript_scheme_is_matched_through_embedded_control_chars() {
    let html = render("a href=\"java\tscript:alert(1)\"\n  \"x\"\n");
    assert!(!html.contains("script:"), "{html}");
}

#[test]
fn vbscript_href_is_dropped() {
    let html = render("a href=\"vbscript:msgbox(1)\"\n  \"x\"\n");
    assert!(!html.contains("vbscript:"), "{html}");
}

#[test]
fn data_image_src_is_kept() {
    let html = render("img src=\"data:image/png;base64,iVBORw0KGgo=\"\n");
    assert!(html.contains("data:image/png"), "{html}");
}

#[test]
fn data_href_is_dropped() {
    let html = render("a href=\"data:text/html,<script>alert(1)</script>\"\n  \"x\"\n");
    assert!(!html.contains("data:text/html"), "{html}");
}

#[test]
fn scheme_check_does_not_touch_non_url_attributes() {
    let html = render("div title=\"javascript:not a url\"\n");
    assert!(html.contains("javascript:not a url"), "{html}");
}

#[test]
fn url_attribute_names_are_filtered_case_insensitively() {
    for (attribute, value) in [
        ("HREF", "javascript:alert(1)"),
        ("SrC", "vbscript:unsafe"),
        ("ACTION", "data:text/html,unsafe"),
        ("FormAction", "data:text/html,unsafe"),
    ] {
        let source = format!("<a {attribute}=\"{value}\">Link</a>");
        let html = render(&source);
        assert!(!html.contains(&format!("{attribute}=")), "{html}");
    }
    let html = render(r#"<a HREF="/docs">Link</a>"#);
    assert!(html.contains("HREF=\"/docs\""), "{html}");
}
