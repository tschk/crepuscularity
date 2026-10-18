use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn raw_html_is_visible_in_plain_and_structural_text_paths() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "<b>Literal content</b>");
    for source in [
        "{=content}",
        "div\n  {=content}",
        "div\n  {=content}\n  span\n    \"Sibling\"",
    ] {
        let area = Rect::new(0, 0, 50, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            output.contains("<b>Literal content</b>"),
            "{source}: {output}"
        );
    }
}
