#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rect, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn bordered_text_auto_size_reserves_border_and_padding() {
    for direction in ["flex-row", "flex-col"] {
        let screen = ScreenSize::new(40, 40);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let source=format!("div {direction} bg-[#000000]\n  span #label border-2 border-[#00ff00] p-[1px] text-[#ffffff]\n    \"K\"");
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("label").unwrap().bounds,
            Rect::new(0, 0, 14, 14)
        );
        let green = Color::rgb(0, 255, 0).to_rgb565().0;
        for y in 0..14 {
            for x in 0..14 {
                let pixel = fb.pixels()[y * 40 + x];
                if x < 2 || y < 2 || x >= 12 || y >= 12 {
                    assert_eq!(pixel, green);
                } else if !(3..11).contains(&x) || !(3..11).contains(&y) {
                    assert_eq!(pixel, 0);
                }
            }
        }
        assert!(fb.pixels().contains(&u16::MAX));
    }
}

#[test]
fn child_fill_uses_the_content_rectangle_inside_border_and_padding() {
    let screen = ScreenSize::new(30, 20);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc = render_template_to_framebuffer(
        "div border-2 pl-[3px] pr-[1px] pt-[1px] pb-[3px]\n  div #child w-full h-full",
        &TemplateContext::new(),
        screen,
        &mut fb,
    )
    .unwrap();
    assert_eq!(
        doc.node_by_id("child").unwrap().bounds,
        Rect::new(5, 3, 22, 12)
    );
    assert_eq!(doc.hit_test(5, 3), Some("child"));
    assert_eq!(doc.hit_test(4, 3), None);
}

#[test]
fn text_root_auto_size_includes_border_and_fixed_size_stays_outer_size() {
    for (size, expected) in [("", 14), ("w-[20px] h-[20px]", 20)] {
        let screen = ScreenSize::new(32, 32);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = render_template_to_framebuffer(
            &format!("span #label border-2 p-[1px] {size}\n  \"K\""),
            &TemplateContext::new(),
            screen,
            &mut fb,
        )
        .unwrap();
        assert_eq!(
            doc.node_by_id("label").unwrap().bounds,
            Rect::new(0, 0, expected, expected)
        );
    }
}

#[test]
fn oversized_borders_leave_no_text_or_child_content_area() {
    for body in ["  \"K\"", "  div #child w-full h-full bg-[#ffffff]"] {
        let screen = ScreenSize::new(4, 4);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let source = format!("div border-4 border-[#00ff00] text-[#ffffff]\n{body}");
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert!(fb
            .pixels()
            .iter()
            .all(|p| *p == Color::rgb(0, 255, 0).to_rgb565().0));
        if let Some(child) = doc.node_by_id("child") {
            assert_eq!((child.bounds.w, child.bounds.h), (0, 0));
        }
    }
}
