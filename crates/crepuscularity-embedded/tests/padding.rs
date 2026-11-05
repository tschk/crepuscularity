#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Rect, Rgb565Buffer, ScreenSize, TemplateContext, DEFAULT_BG,
};

#[test]
fn asymmetric_padding_sets_child_origin_and_text_size() {
    let mut fb = Rgb565Buffer::new(ScreenSize::new(30, 20), DEFAULT_BG);
    let source = "div pl-[2px] pr-[6px] pt-[1px] pb-[3px]\n  span #label pl-[3px] pr-[1px] pt-[2px] pb-[1px]\n    \"A\"";
    let doc = render_template_to_framebuffer(
        source,
        &TemplateContext::new(),
        ScreenSize::new(30, 20),
        &mut fb,
    )
    .unwrap();
    assert_eq!(
        doc.node_by_id("label").unwrap().bounds,
        Rect::new(2, 1, 12, 11)
    );
    assert_eq!(doc.hit_test(2, 1), Some("label"));
    assert_eq!(doc.hit_test(1, 1), None);
}

#[test]
fn text_nodes_respect_explicit_width_and_height() {
    let mut fb = Rgb565Buffer::new(ScreenSize::new(30, 20), DEFAULT_BG);
    let doc = render_template_to_framebuffer(
        "div\n  span #label w-[20px] h-[12px]\n    \"A\"",
        &TemplateContext::new(),
        ScreenSize::new(30, 20),
        &mut fb,
    )
    .unwrap();
    assert_eq!(
        doc.node_by_id("label").unwrap().bounds,
        Rect::new(0, 0, 20, 12)
    );
}

#[test]
fn stretch_fills_only_auto_sized_cross_axes() {
    for (direction, explicit, expected) in [
        ("flex-row", "", Rect::new(0, 0, 8, 20)),
        ("flex-col", "", Rect::new(0, 0, 30, 8)),
        ("flex-row", "h-[12px]", Rect::new(0, 0, 8, 12)),
        ("flex-col", "w-[12px]", Rect::new(0, 0, 12, 8)),
    ] {
        let source = format!("div {direction} items-stretch\n  span #label {explicit}\n    \"A\"");
        let mut fb = Rgb565Buffer::new(ScreenSize::new(30, 20), DEFAULT_BG);
        let doc = render_template_to_framebuffer(
            &source,
            &TemplateContext::new(),
            ScreenSize::new(30, 20),
            &mut fb,
        )
        .unwrap();
        assert_eq!(doc.node_by_id("label").unwrap().bounds, expected);
    }
}
