#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn container_text_colors_reach_raw_text_spans_and_buttons() {
    for child in [
        "  \"K\"",
        "  span\n    \"K\"",
        "  button\n    \"K\"",
        "  div\n    span\n      \"K\"",
    ] {
        let screen = ScreenSize::new(8, 8);
        let mut actual = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        render_template_to_framebuffer(
            &format!("div text-[#ff0000] bg-[#000000]\n{child}"),
            &TemplateContext::new(),
            screen,
            &mut actual,
        )
        .unwrap();
        let mut expected = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        render_template_to_framebuffer(
            "span text-[#ff0000] bg-[#000000]\n  \"K\"",
            &TemplateContext::new(),
            screen,
            &mut expected,
        )
        .unwrap();
        assert_eq!(actual.pixels(), expected.pixels(), "{child}");
        assert!(actual.pixels().iter().any(|p| *p != 0));
    }
}

#[test]
fn child_overrides_do_not_leak_into_siblings() {
    let source = "div flex-row text-[#ff0000] bg-[#000000]\n  div w-[8px] text-[#00ff00]\n    span\n      \"K\"\n  span\n    \"K\"";
    let screen = ScreenSize::new(16, 8);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb).unwrap();
    let red = Color::rgb(255, 0, 0).to_rgb565().0;
    let green = Color::rgb(0, 255, 0).to_rgb565().0;
    for row in fb.pixels().chunks(16) {
        for x in 0..8 {
            assert_eq!(row[x] == green, row[x + 8] == red);
            assert!(row[x] == 0 || row[x] == green);
            assert!(row[x + 8] == 0 || row[x + 8] == red);
        }
    }
    assert!(fb.pixels().contains(&red));
    assert!(fb.pixels().contains(&green));
}

#[test]
fn manually_constructed_document_roots_start_with_independent_colors() {
    use crepuscularity_embedded::{
        paint::paint_document, EmbeddedDocument, EmbeddedNode, EmbeddedStyle, Rect, DEFAULT_TEXT,
    };
    let screen = ScreenSize::new(16, 8);
    let first = EmbeddedNode {
        id: None,
        tag: "span".into(),
        text: Some("K".into()),
        on_click: None,
        style: EmbeddedStyle {
            text: Some(Color::rgb(255, 0, 0)),
            bg: Some(Color::rgb(0, 0, 0)),
            ..EmbeddedStyle::default()
        },
        bounds: Rect::new(0, 0, 8, 8),
        children: vec![],
    };
    let mut second = first.clone();
    second.style.text = None;
    second.bounds.x = 8;
    let doc = EmbeddedDocument::new(vec![first, second], screen);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    paint_document(&mut fb, &doc);
    let red = Color::rgb(255, 0, 0).to_rgb565().0;
    let default = DEFAULT_TEXT.to_rgb565().0;
    for row in fb.pixels().chunks(16) {
        for x in 0..8 {
            assert_eq!(row[x] == red, row[x + 8] == default);
            assert!(row[x] == 0 || row[x] == red);
            assert!(row[x + 8] == 0 || row[x + 8] == default);
        }
    }
    assert!(fb.pixels().contains(&default));
}
