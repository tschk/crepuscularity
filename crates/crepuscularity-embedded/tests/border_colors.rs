#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn border_color_defaults_to_the_resolved_text_color() {
    for (child_style, expected) in [
        ("border", Color::rgb(255, 0, 0)),
        ("border text-[#0000ff]", Color::rgb(0, 0, 255)),
        ("border border-[#00ff00]", Color::rgb(0, 255, 0)),
        ("border-[#00ff00] border", Color::rgb(0, 255, 0)),
    ] {
        let screen = ScreenSize::new(12, 12);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let source =
            format!("div text-[#ff0000] bg-[#000000]\n  div #box w-full h-full {child_style}");
        render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb).unwrap();
        for y in 0..12 {
            for x in 0..12 {
                let border = x == 0 || x == 11 || y == 0 || y == 11;
                assert_eq!(
                    fb.pixels()[y * 12 + x],
                    if border { expected.to_rgb565().0 } else { 0 },
                    "{child_style}"
                );
            }
        }
    }
}

#[test]
fn border_colors_never_enable_a_zero_width_border() {
    for classes in [
        "border-[#ffffff]",
        "border-0 border-[#ffffff]",
        "border-[#ffffff] border-0",
        "border-2 border-0 border-[#ffffff]",
    ] {
        let screen = ScreenSize::new(16, 16);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let source = format!("div #box {classes} bg-[#000000]\n  span #label\n    \"K\"");
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("box").unwrap().style.border_width,
            0,
            "{classes}"
        );
        assert_eq!(doc.node_by_id("label").unwrap().bounds.x, 0);
        assert_eq!(doc.node_by_id("label").unwrap().bounds.y, 0);
        assert_eq!(fb.pixels()[15], 0);
    }
}
