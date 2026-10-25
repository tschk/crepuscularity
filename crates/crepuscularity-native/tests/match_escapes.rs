use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_template_to_ir, to_json};

#[test]
fn quoted_match_patterns_decode_escapes() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "a\nb");
    for source in [
        "match {status}\n\"a\\nb\" =>\n  \"Matched\"\n_ =>\n  \"Missed\"",
        "match {status}\n  \"a\\nb\" =>\n    \"Matched\"\n  _ =>\n    \"Missed\"",
    ] {
        let json = to_json(&render_template_to_ir(source, &ctx).unwrap()).unwrap();
        assert!(json.contains("Matched"), "{json}");
        assert!(!json.contains("Missed"), "{json}");
    }
}

#[test]
fn quoted_match_patterns_do_not_fall_back_to_raw_source_spelling() {
    for (pattern, value, matched) in [
        (r#""ready""#, "ready", true),
        (r#""ready""#, r#""ready""#, false),
        (r#""a\nb""#, "a\nb", true),
        (r#""a\nb""#, r#""a\nb""#, false),
        (r#""a\nb""#, r#"a\nb"#, false),
        ("ready", "ready", true),
        ("ready", "other", false),
        ("_", "anything", true),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("status", value);
        let source =
            format!("match {{status}}\n  {pattern} =>\n    \"Matched\"\n  _ =>\n    \"Missed\"");
        let output = to_json(&render_template_to_ir(&source, &ctx).unwrap()).unwrap();
        assert_eq!(
            output.contains("Matched"),
            matched,
            "pattern {pattern:?}, value {value:?}: {output}"
        );
        assert_eq!(
            output.contains("Missed"),
            !matched,
            "pattern {pattern:?}, value {value:?}: {output}"
        );
    }
}
