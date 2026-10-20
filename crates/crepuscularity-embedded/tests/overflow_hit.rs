#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, EmbeddedDocument, Rgb565Buffer, ScreenSize,
    TemplateContext,
};

fn render(source: &str) -> (EmbeddedDocument, Vec<u16>) {
    let screen = ScreenSize::new(32, 32);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc =
        render_template_to_framebuffer(source, &TemplateContext::new(), screen, &mut fb).unwrap();
    (doc, fb.pixels().to_vec())
}

fn source(ancestor_classes: &str, child_id: &str) -> String {
    format!("div bg-[#000000]\n  div #box h-[8px] {ancestor_classes}\n    div #first w-[8px] h-[8px] bg-[#ff0000]\n    div {child_id} w-[8px] h-[8px] bg-[#0000ff]")
}

#[test]
fn visible_overflow_pixels_have_their_child_hit_target() {
    let (doc, pixels) = render(&source("", "#overflow"));
    let child = doc.node_by_id("overflow").unwrap().bounds;
    assert_eq!((child.y, child.h), (8, 8));
    assert_eq!(doc.node_by_id("box").unwrap().bounds.h, 8);
    assert_eq!(pixels[8 * 32], Color::rgb(0, 0, 255).to_rgb565().0);
    assert_eq!(doc.hit_test(0, 8), Some("overflow"));
    assert_eq!(doc.node_at(0, 15), Some("overflow"));
    assert_eq!(doc.hit_test(8, 8), None);
}

#[test]
fn hidden_and_invisible_ancestors_suppress_overflow_but_opacity_keeps_hit_policy() {
    for classes in ["hidden", "invisible", "opacity-0"] {
        let (doc, pixels) = render(&source(classes, "#overflow"));
        assert_eq!(
            pixels[8 * 32],
            Color::rgb(0, 0, 0).to_rgb565().0,
            "{classes}"
        );
        assert_eq!(
            doc.hit_test(0, 8),
            if classes == "opacity-0" {
                Some("overflow")
            } else {
                None
            },
            "{classes}"
        );
    }
}

#[test]
fn unnamed_overflow_bubbles_to_its_named_ancestor() {
    let (doc, pixels) = render(&source("", ""));
    assert_eq!(pixels[8 * 32], Color::rgb(0, 0, 255).to_rgb565().0);
    assert_eq!(doc.hit_test(0, 8), Some("box"));
}

#[test]
fn later_siblings_and_roots_keep_paint_and_hit_precedence() {
    let sibling = format!(
        "{}\n  div #later w-[8px] h-[8px] bg-[#00ff00]",
        source("", "#overflow")
    );
    let root = format!(
        "{}\ndiv #later w-[8px] h-[16px] bg-[#00ff00]",
        source("", "#overflow")
    );
    for source in [sibling, root] {
        let (doc, pixels) = render(&source);
        assert_eq!(pixels[8 * 32], Color::rgb(0, 255, 0).to_rgb565().0);
        assert_eq!(doc.hit_test(0, 8), Some("later"));
    }
}

#[test]
fn overflow_hit_testing_still_clips_to_screen() {
    let (mut doc, _) = render(&source("", "#overflow"));
    doc.screen = ScreenSize::new(4, 12);
    assert_eq!(doc.hit_test(3, 11), Some("overflow"));
    for (x, y) in [(4, 8), (0, 12), (32, 32), (u16::MAX, u16::MAX)] {
        assert_eq!(doc.hit_test(x, y), None);
        assert_eq!(doc.node_at(x, y), None);
    }
}
