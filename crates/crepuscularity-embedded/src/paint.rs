//! Rasterize [`EmbeddedDocument`] trees into a [`Framebuffer`].

use crate::color::Color;
use crate::document::{EmbeddedDocument, EmbeddedNode, Rect, DEFAULT_BG, DEFAULT_TEXT};
use crate::font::{draw_text, FontMetrics};
use crate::framebuffer::Framebuffer;

pub fn paint_document(fb: &mut impl Framebuffer, doc: &EmbeddedDocument) {
    let bg = doc
        .root
        .iter()
        .find(|node| {
            !node.style.hidden && !node.style.invisible && node.style.opacity_percent != Some(0)
        })
        .and_then(|n| n.style.bg)
        .unwrap_or(DEFAULT_BG);
    fb.fill(bg);
    for node in &doc.root {
        paint_node(fb, node, DEFAULT_TEXT);
    }
}

fn paint_node(fb: &mut impl Framebuffer, node: &EmbeddedNode, inherited_text: Color) {
    if node.style.hidden || node.style.invisible || node.style.opacity_percent == Some(0) {
        return;
    }
    let text_color = node.style.text.unwrap_or(inherited_text);
    if let Some(bg) = node.style.bg {
        fill_rect(fb, node.bounds, bg);
    }
    for child in &node.children {
        paint_node(fb, child, text_color);
    }
    if node.style.border_width > 0 {
        let border_color = node.style.border_color.unwrap_or(text_color);
        stroke_rect(fb, node.bounds, node.style.border_width, border_color);
    }
    if let Some(text) = &node.text {
        draw_text(
            fb,
            node.style.content_bounds(node.bounds),
            text,
            text_color,
            &FontMetrics::default(),
        );
    }
}

fn fill_rect(fb: &mut impl Framebuffer, r: Rect, color: Color) {
    for y in r.y..r.y.saturating_add(r.h).min(fb.height()) {
        for x in r.x..r.x.saturating_add(r.w).min(fb.width()) {
            fb.set_pixel(x, y, color);
        }
    }
}

fn stroke_rect(fb: &mut impl Framebuffer, r: Rect, width: u16, color: Color) {
    if r.w == 0 || r.h == 0 || width == 0 {
        return;
    }
    for inset in 0..u32::from(width) {
        let x0 = u32::from(r.x) + inset;
        let y0 = u32::from(r.y) + inset;
        let x1 = (u32::from(r.x) + u32::from(r.w) - 1).saturating_sub(inset);
        let y1 = (u32::from(r.y) + u32::from(r.h) - 1).saturating_sub(inset);
        if x0 > x1 || y0 > y1 || x0 >= u32::from(fb.width()) || y0 >= u32::from(fb.height()) {
            break;
        }
        for x in x0..=x1.min(u32::from(fb.width()) - 1) {
            fb.set_pixel(x as u16, y0 as u16, color);
            if y1 != y0 && y1 < u32::from(fb.height()) {
                fb.set_pixel(x as u16, y1 as u16, color);
            }
        }
        for y in y0..=y1.min(u32::from(fb.height()) - 1) {
            fb.set_pixel(x0 as u16, y as u16, color);
            if x1 != x0 && x1 < u32::from(fb.width()) {
                fb.set_pixel(x1 as u16, y as u16, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{framebuffer::Rgb565Buffer, screen::ScreenSize};

    #[test]
    fn text_is_painted_inside_its_padding() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let mut actual = Rgb565Buffer::new(ScreenSize::new(12, 10), black);
        let mut expected = Rgb565Buffer::new(ScreenSize::new(12, 10), black);
        let node = EmbeddedNode {
            id: None,
            tag: "span".into(),
            text: Some("A".into()),
            on_click: None,
            style: crate::EmbeddedStyle {
                padding_x: 2,
                padding_y: 1,
                text: Some(white),
                ..crate::EmbeddedStyle::default()
            },
            bounds: Rect::new(0, 0, 12, 10),
            children: alloc::vec![],
        };
        paint_node(&mut actual, &node, DEFAULT_TEXT);
        draw_text(
            &mut expected,
            Rect::new(2, 1, 8, 8),
            "A",
            white,
            &FontMetrics::default(),
        );
        assert_eq!(actual.pixels(), expected.pixels());
    }

    #[test]
    fn clipped_border_does_not_move_offscreen_edges_onto_screen() {
        let mut fb = Rgb565Buffer::new(ScreenSize::new(3, 3), Color::rgb(0, 0, 0));
        let white = Color::rgb(255, 255, 255);
        stroke_rect(&mut fb, Rect::new(0, 0, 5, 5), 1, white);
        let on = white.to_rgb565().0;
        assert_eq!(fb.pixels(), &[on, on, on, on, 0, 0, on, 0, 0]);
    }

    #[test]
    fn empty_rectangles_have_no_border() {
        let mut fb = Rgb565Buffer::new(ScreenSize::new(3, 3), Color::rgb(0, 0, 0));
        stroke_rect(&mut fb, Rect::new(1, 1, 0, 2), 1, Color::rgb(255, 255, 255));
        assert!(fb.pixels().iter().all(|pixel| *pixel == 0));
    }
}
