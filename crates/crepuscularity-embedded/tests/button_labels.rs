#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn nested_hidden_or_empty_containers_do_not_swallow_later_button_labels() {
    for prefix in [
        "  span\n    span hidden\n      \"secret\"\n",
        "  span\n    span class='hidden'\n      \"secret\"\n",
        "  span\n    span\n",
        "  span\n    span\n      span hidden\n        \"secret\"\n",
    ] {
        let screen = ScreenSize::new(32, 8);
        let source =
            format!("button #target bg-[#000000] text-[#ffffff]\n{prefix}  span\n    \"OK\"");
        let mut actual = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc =
            render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut actual)
                .unwrap();
        assert_eq!(
            doc.node_by_id("target").unwrap().text.as_deref(),
            Some("OK"),
            "{prefix}"
        );
        let mut expected = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        render_template_to_framebuffer(
            "button #target bg-[#000000] text-[#ffffff]\n  \"OK\"",
            &TemplateContext::new(),
            screen,
            &mut expected,
        )
        .unwrap();
        assert_eq!(actual.pixels(), expected.pixels());
        assert_eq!(doc.node_by_id("target").unwrap().bounds.w, 16);
        assert_eq!(doc.hit_test(15, 0), Some("target"));
        assert_eq!(doc.hit_test(16, 0), None);
    }
}

#[test]
fn explicitly_empty_primary_labels_still_take_precedence() {
    let source = "button #target\n  span\n    \"\"\n  span\n    \"fallback\"";
    let screen = ScreenSize::new(64, 8);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc =
        render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb).unwrap();
    assert_eq!(doc.node_by_id("target").unwrap().text.as_deref(), Some(""));
}
