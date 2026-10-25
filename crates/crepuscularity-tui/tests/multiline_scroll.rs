use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

fn render(source: &str, offset: i64) -> Buffer {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "zero\none\ntwo");
    ctx.set("offset", offset);
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    buffer
}

#[test]
fn flexible_multiline_children_contribute_their_logical_rows() {
    let source = "div h-full overflow-y-scroll scroll-offset={offset}\n  div\n    \"{content}\"";
    for offset in [1, 999] {
        let buffer = render(source, offset);
        assert_eq!(buffer[(0, 0)].symbol(), "o");
        assert_eq!(buffer[(0, 1)].symbol(), "t");
    }
}

#[test]
fn text_only_scroll_elements_keep_alignment_and_style() {
    for body in ["\"{content}\"", "{=content}"] {
        let source = format!("div h-full overflow-y-scroll text-right text-red-500 scroll-offset={{offset}}\n  {body}");
        let buffer = render(&source, 1);
        assert_eq!(buffer[(17, 0)].symbol(), "o");
        assert_eq!(buffer[(17, 1)].symbol(), "t");
        assert_eq!(buffer[(17, 1)].fg, Color::Red);
        assert_eq!(buffer[(0, 0)].symbol(), " ");
    }
}

#[test]
fn scrolling_to_bottom_keeps_gaps_and_the_final_child() {
    let source = "div h-full overflow-y-scroll gap-[1] scroll-bottom=true\n  div\n    \"{content}\"\n  div\n    \"End\"";
    let buffer = render(source, 0);
    assert_eq!(buffer[(0, 0)].symbol(), " ");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
    assert_eq!(buffer[(1, 1)].symbol(), "n");
}

#[test]
fn explicit_child_height_stays_fixed_when_measuring_scroll_extent() {
    let source = "div h-full overflow-y-scroll scroll-offset={offset}\n  div h-[1]\n    \"{content}\"\n  div h-[1]\n    \"End\"";
    let buffer = render(source, 999);
    assert_eq!(buffer[(0, 0)].symbol(), "z");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn nested_flexible_children_keep_their_natural_heights_when_scrolled() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div\n    div\n      \"{content}\"\n    div\n      \"End\"";
    let buffer = render(source, 0);
    assert_eq!(buffer[(0, 0)].symbol(), "t");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn nested_explicit_height_retains_its_flex_distribution() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div h-[4]\n    div\n      \"{content}\"\n    div\n      \"End\"";
    let buffer = render(source, 0);
    assert_eq!(buffer[(0, 0)].symbol(), "E");
    assert_eq!(buffer[(0, 1)].symbol(), " ");
}

#[test]
fn natural_scroll_height_reaches_columns_inside_rows() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div flex-row\n    div\n      div\n        \"{content}\"\n      div\n        \"End\"\n    div\n      \"Side\"";
    let buffer = render(source, 0);
    assert_eq!(buffer[(0, 0)].symbol(), "t");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn text_only_horizontal_layout_keeps_paragraph_behavior() {
    let source = "div h-full flex-row overflow-y-scroll\n  \"{content}\"";
    let buffer = render(source, 1);
    assert_eq!(buffer[(0, 0)].symbol(), "z");
    assert_eq!(buffer[(0, 1)].symbol(), "o");
}
