use crepuscularity_core::{parse_template_with_path, TemplateContext, TemplateValue};
use crepuscularity_tui::BufferRenderer;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use std::path::Path;

#[test]
fn angular_attribute_presence_classes_and_ids_keep_literal_semantics() {
    let nodes = parse_template_with_path(
        "<span [attr.id]='name' [attr.class]='classes' [attr.hidden]='state'>VISIBLE</span>",
        Some(Path::new("a.ng")),
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("name", TemplateValue::Null);
    ctx.set("classes", "text-red-500");
    ctx.set("state", false);
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    assert!(!buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>()
        .contains("VISIBLE"));
    ctx.set("state", TemplateValue::Null);
    ctx.set("name", "{missing}");
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    assert!(buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>()
        .contains("VISIBLE"));
    assert_ne!(buffer[(0, 0)].fg, Color::Reset);
}

#[test]
fn angular_attribute_interval_does_not_change_rotation_control() {
    let nodes = parse_template_with_path(
        "<slot-rotate [attr.interval]='99'>one<!--split-->two</slot-rotate>",
        Some(Path::new("a.ng")),
    )
    .unwrap();
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    BufferRenderer::new()
        .render_nodes(&nodes, &TemplateContext::new(), &mut buffer, area)
        .unwrap();
}
