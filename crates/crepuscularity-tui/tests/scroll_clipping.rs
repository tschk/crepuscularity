use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

fn render(source: &str, offset: i64, width: u16, height: u16) -> Buffer {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "aaaabbbbccccddddeeeeffff");
    ctx.set("offset", offset);
    let area = Rect::new(3, 5, width, height);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    buffer
}

#[test]
fn clipped_scroll_output_matches_full_buffer_rendering() {
    let cases = [
        ("paragraph", "div\n  \"{content}\""),
        ("block", "div border title=\"Title\" p-[1]\n  \"{content}\""),
        ("title", "div title=\"Title\"\n  \"{content}\""),
        ("padding", "div p-[2]\n  \"{content}\""),
        ("siblings", "div h-[2] border\n  \"First\"\ndiv gap-[1]\n  div\n    \"{content}\"\n  div\n    \"End\""),
        ("nested", "div h-[8] overflow-y-scroll scroll-offset=2 border title=\"Inner\" p-[1]\n  div\n    \"{content}\"\n  div\n    \"End\"\ndiv h-[1]\n  \"Footer\""),
        ("unicode", "div border\n  \"界語文天地人\"\ndiv\n  \"End\""),
    ];
    let mut snapshot = String::new();
    for (name, body) in cases {
        let indented = body
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!("div h-full overflow-y-scroll scroll-offset={{offset}}\n{indented}");
        for offset in [0, 1, 2, 3, 999] {
            let buffer = render(&source, offset, 8, 3);
            let rows: Vec<_> = (buffer.area.y..buffer.area.bottom())
                .map(|y| {
                    (buffer.area.x..buffer.area.right())
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect();
            snapshot.push_str(&format!("{name} {offset}: {rows:?}\n"));
        }
    }
    // Captured with the former full-height buffer to verify clipping compatibility.
    assert_eq!(
        snapshot,
        include_str!("fixtures/scroll_clipping.snap").replace("\r\n", "\n")
    );
}

#[test]
fn huge_content_tiny_viewport() {
    let source = "div h-full overflow-y-scroll scroll-bottom=true\n  div h-[8000]\n    \"Top\"\n  div h-[1]\n    \"End\"";
    let buffer = render(source, 0, 80, 4);
    assert_eq!(buffer[(3, 8)].symbol(), "E");
}

#[test]
fn clipped_blocks_retain_text_border_and_background_styles() {
    let source = "div h-full overflow-y-scroll scroll-offset=3\n  div border border-green-500 bg-red-500 text-blue-500 title=\"Title\" p-[1]\n    \"{content}\"";
    let buffer = render(source, 0, 8, 3);
    assert_eq!(buffer[(3, 5)].symbol(), "│");
    assert_eq!(buffer[(3, 5)].fg, ratatui::style::Color::Green);
    assert_eq!(buffer[(5, 5)].symbol(), "b");
    assert_eq!(buffer[(5, 5)].fg, ratatui::style::Color::Blue);
    assert_eq!(buffer[(5, 5)].bg, ratatui::style::Color::Red);
}

#[test]
fn empty_scroll_viewports_skip_large_content() {
    let source =
        "div h-full overflow-y-scroll scroll-bottom=true\n  div h-[65535]\n    \"{content}\"";
    for (width, height) in [(0, 3), (8, 0), (0, 0)] {
        let buffer = render(source, 0, width, height);
        assert_eq!(buffer.area, Rect::new(3, 5, width, height));
        assert!(buffer.content.is_empty());
    }
}
