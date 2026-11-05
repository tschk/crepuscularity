#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rect, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn root_size_hints_constrain_bounds_paint_and_hits() {
    for (classes, expected) in [
        ("w-[12px] h-[10px]", Rect::new(0, 0, 12, 10)),
        ("w-1/2 h-1/4", Rect::new(0, 0, 16, 6)),
        ("w-full h-full", Rect::new(0, 0, 32, 24)),
        ("flex-1", Rect::new(0, 0, 32, 24)),
        ("w-[99px] h-[99px]", Rect::new(0, 0, 32, 24)),
        ("w-[0px] h-[0px]", Rect::new(0, 0, 0, 0)),
        ("w-[12px]", Rect::new(0, 0, 12, 24)),
        ("h-[10px]", Rect::new(0, 0, 32, 10)),
    ] {
        let screen = ScreenSize::new(32, 24);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let source = format!("div bg-[#000000]\ndiv #target {classes} bg-[#ffffff]");
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("target").unwrap().bounds,
            expected,
            "{classes}"
        );
        for y in 0..24 {
            for x in 0..32 {
                let inside = expected.contains(x, y);
                assert_eq!(
                    fb.pixels()[y as usize * 32 + x as usize],
                    if inside { u16::MAX } else { 0 },
                    "{classes}: ({x},{y})"
                );
                assert_eq!(
                    doc.hit_test(x, y),
                    if inside { Some("target") } else { None }
                );
            }
        }
    }
}

#[test]
fn explicit_text_root_dimensions_and_descendant_sizes_use_the_root_rectangle() {
    let screen = ScreenSize::new(32, 24);
    for source in [
        "span #target w-[20px] h-[12px]\n  \"K\"",
        "div #target w-[20px] h-[12px]\n  div #child w-full h-full",
    ] {
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("target").unwrap().bounds,
            Rect::new(0, 0, 20, 12)
        );
        if let Some(child) = doc.node_by_id("child") {
            assert_eq!(child.bounds, Rect::new(0, 0, 20, 12));
        }
    }
}
