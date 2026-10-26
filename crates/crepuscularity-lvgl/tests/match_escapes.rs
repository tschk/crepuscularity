use crepuscularity_core::context::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn quoted_match_patterns_decode_escapes() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "a\nb");
    let source = "match {status}\n\"a\\nb\" =>\n  \"Matched\"\n_ =>\n  \"Missed\"";
    let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
    assert!(xml.contains("Matched"), "{xml}");
    assert!(!xml.contains("Missed"), "{xml}");
}

#[test]
fn quoted_patterns_match_decoded_values_only() {
    for (pattern, value, matched) in [
        ("\"ready\"", "ready", true),
        ("\"ready\"", "\"ready\"", false),
        ("\"a\\nb\"", "a\nb", true),
        ("\"a\\nb\"", "\"a\\nb\"", false),
        ("\"a\\\"b\"", "a\"b", true),
        ("\"a\\\"b\"", "\"a\\\"b\"", false),
        ("ready", "ready", true),
        ("ready", "different", false),
        ("_", "anything", true),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("status", value);
        let fallback = if pattern == "_" {
            ""
        } else {
            "\n_ =>\n  span #fallback \"Fallback\""
        };
        let source =
            format!("match {{status}}\n{pattern} =>\n  span #selected \"Selected\"{fallback}");
        let xml = render_template_to_lvgl_xml(&source, &ctx).unwrap();
        assert_eq!(
            xml.contains("id=\"selected\""),
            matched,
            "pattern={pattern:?} value={value:?}: {xml}"
        );
        assert_eq!(
            xml.contains("id=\"fallback\""),
            !matched,
            "pattern={pattern:?} value={value:?}: {xml}"
        );
    }
}
