use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn multiline_values_preserve_rows_in_all_text_paths() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "First\n界Second");
    for source in [
        "\"{content}\"",
        "{=content}",
        "div\n  \"{content}\"",
        "div\n  {=content}",
        "div\n  {=content}\n  span\n    \"Sibling\"",
    ] {
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        assert_eq!(buffer[(0, 0)].symbol(), "F", "{source}");
        assert_eq!(buffer[(0, 1)].symbol(), "界", "{source}");
        assert_eq!(buffer[(2, 1)].symbol(), "S", "{source}");
    }
}

#[test]
fn multiline_parts_preserve_crlf_blank_rows_and_inherited_style() {
    let mut ctx = TemplateContext::new();
    ctx.set("left", "First\r");
    ctx.set("right", "\n\n界Second");
    let source = "div text-red-500\n  \"{left}{right}\"";
    let area = Rect::new(0, 0, 20, 4);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "F");
    assert_eq!(buffer[(0, 1)].symbol(), " ");
    assert_eq!(buffer[(0, 2)].symbol(), "界");
    assert_eq!(buffer[(0, 2)].fg, ratatui::style::Color::Red);
}

#[test]
fn multiline_unicode_is_clipped_in_tiny_and_offset_viewports() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "A\n界B");
    for (width, height) in [(0, 0), (0, 2), (1, 0), (1, 1), (1, 2), (3, 2)] {
        let area = Rect::new(7, 9, width, height);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer("\"{content}\"", &ctx, &mut buffer, area).unwrap();
        if width > 0 && height > 0 {
            assert_eq!(buffer[(7, 9)].symbol(), "A");
        }
        if width >= 2 && height >= 2 {
            assert_eq!(buffer[(7, 10)].symbol(), "界");
        }
    }
}
