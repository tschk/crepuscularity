use crepuscularity_core::context::TemplateContext;
use crepuscularity_tui::render_template_to_buffer;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

#[test]
fn malformed_hex_classes_preserve_text_and_valid_colors() {
    let area = Rect::new(0, 0, 20, 3);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(
        "div flex-col\n  span text-[#a界bc] bg-[#00ff00]\n    \"safe\"",
        &TemplateContext::new(),
        &mut buffer,
        area,
    )
    .unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "s");
    assert_eq!(buffer[(0, 0)].bg, Color::Rgb(0, 255, 0));
}
