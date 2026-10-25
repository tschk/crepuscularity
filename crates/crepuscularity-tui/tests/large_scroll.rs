use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_tui::BufferRenderer;
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn scrolling_beyond_u16_height_keeps_the_final_logical_row() {
    let mut ctx = TemplateContext::new();
    let items = (0..65_536)
        .map(|index| {
            let mut item = TemplateContext::new();
            item.set("value", format!("ROW-{index:05}"));
            item
        })
        .collect();
    ctx.set("rows", TemplateValue::List(items));
    ctx.set("offset", 65_535i64);
    let area = Rect::new(0, 0, 16, 1);
    for scroll in ["scroll-bottom={true}", "scroll-offset={offset}"] {
        let source = format!(
            "div h-full overflow-y-scroll {scroll}\n  for row in rows\n    div h-[1]\n      \"{{row}}\""
        );
        let nodes = parse_template(&source).unwrap();
        let mut buffer = Buffer::empty(area);
        BufferRenderer::new()
            .render_nodes(&nodes, &ctx, &mut buffer, area)
            .unwrap();
        let visible = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert_eq!(visible.trim_end(), "ROW-65535", "{scroll}: {visible:?}");
    }
}

#[test]
fn large_aggregate_gaps_and_offsets_preserve_partial_viewport_clipping() {
    let mut ctx = TemplateContext::new();
    ctx.set("offset", i64::MAX);
    for scroll in ["scroll-bottom={true}", "scroll-offset={offset}"] {
        let nodes = parse_template(&format!(
            "div h-full gap-[32768] overflow-y-scroll {scroll}\n  div h-[1]\n    \"FIRST\"\n  div h-[1]\n    \"MIDDLE\"\n  div h-[1]\n    \"ROW-TAIL\""
        )).unwrap();
        // The viewport starts at x=2, but only its suffix intersects the buffer.
        let viewport = Rect::new(2, 3, 16, 1);
        let mut buffer = Buffer::empty(Rect::new(5, 3, 10, 1));
        BufferRenderer::new()
            .render_nodes(&nodes, &ctx, &mut buffer, viewport)
            .unwrap();
        let visible = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert_eq!(visible.trim_end(), "-TAIL", "{scroll}: {visible:?}");
    }
}

#[test]
fn maximum_u16_child_height_keeps_its_last_text_row() {
    let nodes = parse_template(
        "div h-full overflow-y-scroll scroll-bottom={true}\n  div h-[65535]\n    \"{content}\"",
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("content", format!("{}LAST-CHILD-ROW", "\n".repeat(65_534)));
    let area = Rect::new(0, 0, 20, 1);
    let mut buffer = Buffer::empty(area);
    BufferRenderer::new()
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    let visible = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(visible.trim_end(), "LAST-CHILD-ROW", "{visible:?}");
}
