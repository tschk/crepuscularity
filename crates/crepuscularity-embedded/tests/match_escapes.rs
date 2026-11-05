#![cfg(feature = "std")]

use crepuscularity_core::{context::TemplateContext, parser::parse_template};
use crepuscularity_embedded::{render_nodes_to_document, ScreenSize};

#[test]
fn quoted_match_patterns_decode_escapes() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "a\nb");
    for source in [
        "match {status}\n\"a\\nb\" =>\n  \"Matched\"\n_ =>\n  \"Missed\"",
        "match {status}\n  \"a\\nb\" =>\n    \"Matched\"\n  _ =>\n    \"Missed\"",
    ] {
        let nodes = parse_template(source).unwrap();
        let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(64, 32)).unwrap();
        let rendered = format!("{document:?}");
        assert!(rendered.contains("Matched"), "{rendered}");
        assert!(!rendered.contains("Missed"), "{rendered}");
    }
}

#[test]
fn quoted_patterns_paint_only_the_decoded_matching_branch() {
    use crepuscularity_embedded::{render_template_to_framebuffer, Color, Rgb565Buffer};
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
            "\n_ =>\n  div #fallback w-full h-full bg-[#0000ff]"
        };
        let source = format!(
            "match {{status}}\n{pattern} =>\n  div #selected w-full h-full bg-[#ff0000]{fallback}"
        );
        let screen = ScreenSize::new(16, 8);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 255, 0));
        let document = render_template_to_framebuffer(&source, &ctx, screen, &mut fb).unwrap();
        let expected = if matched {
            Color::rgb(255, 0, 0)
        } else {
            Color::rgb(0, 0, 255)
        }
        .to_rgb565()
        .0;
        assert!(
            fb.pixels().iter().all(|pixel| *pixel == expected),
            "pattern={pattern:?} value={value:?}: {document:?}"
        );
        assert_eq!(
            document.node_at(0, 0),
            Some(if matched { "selected" } else { "fallback" })
        );
    }
}
