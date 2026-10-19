use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn height_fit_text_reserves_each_multiline_row_before_its_sibling() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "A\n界B");
    for classes in ["h-fit", "h-fit w-fit", "h-fit border p-1"] {
        let source =
            format!("div\n  div {classes}\n    \"{{content}}\"\n  div h-[1]\n    \"After\"");
        let area = Rect::new(0, 0, 20, 10);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(&source, &ctx, &mut buffer, area).unwrap();
        let row = if classes.contains("border") { 6 } else { 2 };
        assert_eq!(buffer[(0, row)].symbol(), "A", "{classes}");
        assert_eq!(buffer[(1, row)].symbol(), "f", "{classes}");
    }
}

#[test]
fn width_fit_uses_the_longest_multiline_display_width() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "A\n界B");
    for classes in ["w-fit", "w-fit h-fit"] {
        let source =
            format!("div flex-row\n  div {classes}\n    \"{{content}}\"\n  div\n    \"After\"");
        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(&source, &ctx, &mut buffer, area).unwrap();
        assert_eq!(buffer[(0, 1)].symbol(), "界", "{classes}");
        assert_eq!(buffer[(3, 0)].symbol(), "A", "{classes}");
        assert_eq!(buffer[(4, 0)].symbol(), "f", "{classes}");
    }
}

#[test]
fn cross_axis_width_fit_does_not_override_explicit_height() {
    let source = "div\n  div w-fit h-[1]\n    \"Long text\"\n  div h-[1]\n    \"After\"";
    let area = Rect::new(0, 0, 20, 10);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "L");
    assert_eq!(buffer[(0, 1)].symbol(), "A");
    assert_eq!(buffer[(1, 1)].symbol(), "f");
}
