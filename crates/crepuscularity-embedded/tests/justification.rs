#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn justification_classes_reach_layout_paint_and_hit_testing() {
    for (class, left, right) in [("justify-around", 17, 72), ("justify-evenly", 23, 66)] {
        let source = format!(
            "div flex flex-row {class} gap-[10px] bg-[#000000]\n  div #left w-[10px] h-full bg-[#ffffff]\n  div #right w-[10px] h-full bg-[#ffffff]"
        );
        let screen = ScreenSize::new(100, 20);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(doc.node_by_id("left").unwrap().bounds.x, left);
        assert_eq!(doc.node_by_id("right").unwrap().bounds.x, right);
        assert_eq!(doc.hit_test(left, 0), Some("left"));
        assert_eq!(doc.hit_test(right, 0), Some("right"));
        assert_eq!(doc.hit_test(left - 1, 0), None);
        for row in fb.pixels().as_chunks::<100>().0 {
            for (x, pixel) in row.iter().enumerate() {
                let lit = (left as usize..left as usize + 10).contains(&x)
                    || (right as usize..right as usize + 10).contains(&x);
                assert_eq!(*pixel, if lit { u16::MAX } else { 0 });
            }
        }
    }
}
