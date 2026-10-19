use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn oversized_fit_content_is_clamped_without_overflow() {
    let ctx = TemplateContext::new();
    for source in [
        "div h-fit\n  div h-[40000]\n    \"First\"\n  div h-[40000]\n    \"Second\"",
        "div flex-row w-fit\n  div w-[40000]\n    \"First\"\n  div w-[40000]\n    \"Second\"",
        "div h-fit gap-[40000]\n  div\n  div\n  div",
    ] {
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    }
}

#[test]
fn long_fit_text_does_not_wrap_its_width_to_zero() {
    let content = "A".repeat(65536);
    let source = format!("div w-fit\n  \"{content}\"");
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(&source, &TemplateContext::new(), &mut buffer, area).unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "A");
    assert_eq!(buffer[(19, 0)].symbol(), "A");
}

#[test]
fn oversized_padding_does_not_overflow_layout_measurement() {
    let ctx = TemplateContext::new();
    for source in [
        "div h-1 p-[40000] border\n  \"Hidden\"",
        "div flex-row\n  div w-1 p-[40000] border\n    \"Hidden\"",
        "div w-fit px-[40000] border\n  \"Hidden\"",
        "div h-fit py-[40000] border\n  div\n    \"Hidden\"",
        "div h-[65535] pt-[65535] border\n  \"Hidden\"",
    ] {
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        assert_eq!(buffer.area, area);
    }
}
