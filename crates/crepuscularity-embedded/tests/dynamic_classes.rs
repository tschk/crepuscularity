#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Rect, Rgb565Buffer, ScreenSize, TemplateContext, DEFAULT_BG,
};

#[test]
fn dynamic_class_lists_apply_layout_and_color() {
    let size = ScreenSize::new(64, 32);
    let mut ctx = TemplateContext::new();
    ctx.set("theme", "flex-row gap-[3px] p-[2px] bg-red-500");
    let mut dynamic_fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let mut literal_fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let children = "\n  span #first\n    \"A\"\n  span #second\n    \"B\"";
    let dynamic = render_template_to_framebuffer(
        &format!("div class={{theme}}{children}"),
        &ctx,
        size,
        &mut dynamic_fb,
    )
    .unwrap();
    let literal = render_template_to_framebuffer(
        &format!("div flex-row gap-[3px] p-[2px] bg-red-500{children}"),
        &ctx,
        size,
        &mut literal_fb,
    )
    .unwrap();
    for id in ["first", "second"] {
        assert_eq!(
            dynamic.node_by_id(id).unwrap().bounds,
            literal.node_by_id(id).unwrap().bounds
        );
    }
    assert_eq!(
        dynamic.node_by_id("second").unwrap().bounds,
        Rect::new(13, 2, 8, 8)
    );
    assert_eq!(dynamic_fb.pixels(), literal_fb.pixels());
}
