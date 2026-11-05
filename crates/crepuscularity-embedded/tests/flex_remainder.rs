#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rect, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn flex_children_cover_every_available_pixel_on_both_axes() {
    for row in [false, true] {
        for size in [0, 1, 2, 3, 10, 12, 17] {
            let direction = if row { "flex-row" } else { "flex-col" };
            let full = if row { "w-full" } else { "h-full" };
            let source = format!(
                "div {direction} justify-center bg-[#000000]\n  div #a flex-1 bg-[#ffffff]\n  div #b {full} bg-[#ffffff]\n  div #c flex-1 bg-[#ffffff]"
            );
            let screen = if row {
                ScreenSize::new(size, 4)
            } else {
                ScreenSize::new(4, size)
            };
            let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
            let doc =
                render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
                    .unwrap();
            let mut start = 0;
            for (i, id) in ["a", "b", "c"].iter().enumerate() {
                let length = size / 3 + u16::from(i < (size % 3) as usize);
                let expected = if row {
                    Rect::new(start, 0, length, 4)
                } else {
                    Rect::new(0, start, 4, length)
                };
                assert_eq!(
                    doc.node_by_id(id).unwrap().bounds,
                    expected,
                    "{direction} size {size}"
                );
                for p in start..start + length {
                    assert_eq!(
                        doc.hit_test(if row { p } else { 0 }, if row { 0 } else { p }),
                        Some(*id)
                    );
                }
                start += length;
            }
            assert!(fb.pixels().iter().all(|p| *p == u16::MAX));
        }
    }
}

#[test]
fn fixed_children_gaps_and_hidden_children_do_not_consume_remainder_shares() {
    let source = "div flex-row gap-[1px]\n  div #a flex-1\n  div #fixed w-[2px]\n  div class='hidden' flex-1\n  div #b w-full\n  div #c flex-1";
    let screen = ScreenSize::new(12, 4);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc =
        render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb).unwrap();
    for (id, x, width) in [("a", 0, 3), ("fixed", 4, 2), ("b", 7, 2), ("c", 10, 2)] {
        assert_eq!(
            doc.node_by_id(id).unwrap().bounds,
            Rect::new(x, 0, width, 4)
        );
    }
}

#[test]
fn overcommitted_fixed_sizes_and_gaps_leave_zero_flex_space() {
    let source = "div flex-row gap-[3px]\n  div #a flex-1\n  div #fixed w-[10px]\n  div #b flex-1";
    let screen = ScreenSize::new(8, 4);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc =
        render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb).unwrap();
    for id in ["a", "b"] {
        assert_eq!(doc.node_by_id(id).unwrap().bounds.w, 0);
    }
    assert_eq!(doc.node_by_id("fixed").unwrap().bounds.w, 8);
}
