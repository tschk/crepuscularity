use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

fn render(source: &str, content: &str, width: u16, height: u16) -> Buffer {
    let mut ctx = TemplateContext::new();
    ctx.set("content", content);
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    buffer
}

#[test]
fn text_only_scroll_reaches_wrapped_rows() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  \"{content}\"";
    let buffer = render(source, "aaaabbbbcccc", 4, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "b");
    assert_eq!(buffer[(0, 1)].symbol(), "c");
}

#[test]
fn nested_wrapped_text_keeps_final_sibling_reachable() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div\n    div\n      \"{content}\"\n    div\n      \"End\"";
    let buffer = render(source, "aaaabbbbcccc", 4, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "c");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn columns_in_rows_measure_wrapping_at_the_allocated_width() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div flex-row\n    div w-[4]\n      \"{content}\"\n    div w-[4]\n      \"Side\"";
    let buffer = render(source, "aaaabbbbcccc", 8, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "b");
    assert_eq!(buffer[(0, 1)].symbol(), "c");
}

#[test]
fn child_border_and_padding_reduce_available_wrap_width() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div border p-[1]\n    \"{content}\"\n  div\n    \"End\"";
    let buffer = render(source, "aaaabbbbcccc", 8, 4);
    assert_eq!(buffer[(2, 0)].symbol(), "c");
    assert_eq!(buffer[(0, 2)].symbol(), "└");
    assert_eq!(buffer[(0, 3)].symbol(), "E");
}

#[test]
fn short_wrapped_content_uses_available_viewport_rows() {
    let source = "div h-full overflow-y-scroll\n  \"{content}\"";
    let buffer = render(source, "aaaabbbb", 4, 4);
    assert_eq!(buffer[(0, 0)].symbol(), "a");
    assert_eq!(buffer[(0, 1)].symbol(), "b");
}

#[test]
fn explicit_height_clips_wrapping_before_the_final_sibling() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div h-[1]\n    \"{content}\"\n  div h-[1]\n    \"End\"";
    let buffer = render(source, "aaaabbbbcccc", 4, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "a");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn resizing_recomputes_wrapping_and_handles_empty_inner_width() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  \"{content}\"";
    let narrow = render(source, "aaaabbbbcccc", 4, 2);
    let wide = render(source, "aaaabbbbcccc", 12, 2);
    assert_eq!(narrow[(0, 0)].symbol(), "b");
    assert_eq!(wide[(0, 0)].symbol(), "a");
    assert_eq!(wide[(0, 1)].symbol(), " ");
    render(source, "界界界", 0, 2);
    render(
        "div h-full overflow-y-scroll\n  div border p-[1]\n    \"{content}\"",
        "界界界",
        2,
        2,
    );
}

#[test]
fn unicode_wrap_and_offset_use_terminal_display_width() {
    let source = "div h-full overflow-y-scroll scroll-offset=1\n  \"{content}\"";
    let buffer = render(source, "界語文天地人", 4, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "文");
    assert_eq!(buffer[(2, 1)].symbol(), "人");
}

#[test]
fn nested_content_that_fits_does_not_lose_rows_to_equal_flex_shares() {
    let source =
        "div h-full overflow-y-scroll\n  div\n    div\n      \"{content}\"\n    div\n      \"End\"";
    let buffer = render(source, "aaaabbbbccccddddeeee", 4, 6);
    assert_eq!(buffer[(0, 4)].symbol(), "e");
    assert_eq!(buffer[(0, 5)].symbol(), "E");
}

#[test]
fn nested_scroll_container_keeps_its_wrapped_content() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div overflow-y-scroll\n    \"{content}\"\n  div h-[1]\n    \"End\"";
    let buffer = render(source, "aaaabbbbcccc", 4, 2);
    assert_eq!(buffer[(0, 0)].symbol(), "c");
    assert_eq!(buffer[(0, 1)].symbol(), "E");
}

#[test]
fn wrapped_scroll_retains_alignment_and_inherited_color() {
    let source =
        "div h-full overflow-y-scroll scroll-offset=1 text-right text-red-500\n  \"{content}\"";
    let buffer = render(source, "aaaa bbbb cccc", 5, 2);
    assert_eq!(buffer[(1, 0)].symbol(), "b");
    assert_eq!(buffer[(1, 1)].symbol(), "c");
    assert_eq!(buffer[(0, 0)].symbol(), " ");
    assert_eq!(buffer[(1, 1)].fg, ratatui::style::Color::Red);
}
