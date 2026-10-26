use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_tui::BufferRenderer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

fn assert_crop(source: &str, logical: Rect, clipped: Rect) {
    let nodes = parse_template(source).unwrap();
    let ctx = TemplateContext::new();
    let mut full = Buffer::empty(logical);
    let mut partial = Buffer::empty(clipped);
    let underlying = Style::default()
        .fg(Color::Magenta)
        .bg(Color::Yellow)
        .add_modifier(Modifier::ITALIC);
    // Both renders start with identical host cells, including styles not overridden by text.
    for cell in full.content.iter_mut().chain(partial.content.iter_mut()) {
        cell.set_style(underlying);
    }
    BufferRenderer::new()
        .render_nodes(&nodes, &ctx, &mut full, logical)
        .unwrap();
    BufferRenderer::new()
        .render_nodes(&nodes, &ctx, &mut partial, logical)
        .unwrap();
    for y in clipped.y..clipped.bottom() {
        for x in clipped.x..clipped.right() {
            assert_eq!(
                partial[(x, y)],
                full[(x, y)],
                "{source}: cell ({x},{y}), logical={logical:?}, clipped={clipped:?}"
            );
        }
    }
}

#[test]
fn left_and_right_clipping_preserve_logical_text_positions() {
    let logical = Rect::new(2, 3, 16, 1);
    for clipped in [
        Rect::new(5, 3, 5, 1),
        Rect::new(2, 3, 5, 1),
        Rect::new(5, 3, 10, 1),
    ] {
        assert_crop("span \"ABCDEFGHIJ\"", logical, clipped);
    }
}

#[test]
fn clipped_wrapping_keeps_original_width_and_vertical_line_offsets() {
    let logical = Rect::new(2, 3, 18, 8);
    let source="div h-full\n  \"zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen\"";
    for clipped in [
        Rect::new(6, 3, 8, 6),
        Rect::new(2, 5, 8, 4),
        Rect::new(6, 5, 8, 4),
    ] {
        assert_crop(source, logical, clipped);
    }
}

#[test]
fn aligned_bordered_and_styled_text_matches_full_render_crops() {
    let logical = Rect::new(2, 3, 20, 8);
    for alignment in ["text-left", "text-center", "text-right"] {
        let source=format!("div h-full border border-green-500 bg-red-500 text-blue-500 title=\"TITLE\" p-[1] {alignment}\n  \"ABCDEFGHIJ KLMNOPQRST UVWXYZ\"");
        for clipped in [
            Rect::new(6, 3, 9, 8),
            Rect::new(2, 5, 9, 4),
            Rect::new(6, 5, 9, 4),
        ] {
            assert_crop(&source, logical, clipped);
        }
    }
}

#[test]
fn wide_unicode_and_combining_text_keep_cell_boundaries_when_clipped() {
    let logical = Rect::new(2, 3, 18, 6);
    for source in [
        "div h-full\n  \"界語文天地人界語文天地人\"",
        "div h-full text-center\n  \"Aé界B語C文D天地人\"",
    ] {
        for clipped in [
            Rect::new(3, 3, 8, 6),
            Rect::new(4, 4, 7, 3),
            Rect::new(2, 3, 9, 6),
        ] {
            assert_crop(source, logical, clipped);
        }
    }
}
