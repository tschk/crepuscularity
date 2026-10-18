use crepuscularity_tui::{render_template_to_buffer, BufferRenderer, Template, TemplateContext};
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};
use std::sync::Arc;

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn malformed_rotators_return_errors_without_painting_partial_frames() {
    for (source, message) in [
        (
            "div\n  \"Header\"\n  slot-rotate\n    span \"Bad child\"",
            "only allows quoted text lines",
        ),
        (
            "slot-rotate\n  \"{label}\"\n  \"Literal\"",
            "must be plain text",
        ),
    ] {
        let area = Rect::new(2, 3, 60, 3);
        let mut buffer = Buffer::empty(area);
        for cell in &mut buffer.content {
            cell.set_symbol("X");
        }
        let previous = buffer.clone();
        let error = render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area)
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert_eq!(buffer, previous);
    }
}

#[test]
fn the_first_rotation_error_is_preserved() {
    let source = "div\n  slot-rotate\n    \"{label}\"\n  slot-rotate\n    span \"Bad\"";
    let area = Rect::new(0, 0, 30, 3);
    let mut buffer = Buffer::empty(area);
    let error =
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap_err();
    assert!(error.to_string().contains("must be plain text"), "{error}");
}

#[test]
fn included_rotation_errors_propagate_and_the_renderer_recovers() {
    let nodes = crepuscularity_core::parse_template("include child.crepus").unwrap();
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "slot-rotate\n  span \"Bad\"".into());
    let area = Rect::new(0, 0, 30, 3);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    assert!(renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .is_err());
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "slot-rotate\n  \"Recovered\"".into());
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    assert!(text(&buffer).contains("Recovered"));
}

#[test]
fn only_selected_branches_report_rotation_errors() {
    let nodes = crepuscularity_core::parse_template(
        "if enabled\n  slot-rotate\n    span \"Bad\"\nelse\n  span \"Static\"",
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", false);
    let area = Rect::new(0, 0, 30, 3);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    assert!(text(&buffer).contains("Static"));
    let previous = buffer.clone();
    ctx.set("enabled", true);
    assert!(renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .is_err());
    assert_eq!(buffer, previous);
    ctx.set("enabled", false);
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
}

#[test]
fn cached_drawing_preserves_the_previous_frame_on_invalid_rotation_and_recovers() {
    let mut template = Template::from_source("span \"Before\"");
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let previous = terminal.backend().buffer().clone();
    template.set_source("slot-rotate\n  span \"Bad\"");
    for _ in 0..2 {
        terminal
            .draw(|frame| assert!(template.draw_full_if_changed(frame).is_err()))
            .unwrap();
        assert_eq!(terminal.backend().buffer(), &previous);
    }
    template.set_source("slot-rotate\n  \"Recovered\"");
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    assert!(text(terminal.backend().buffer()).contains("Recovered"));
}

#[test]
fn frontend_selected_templates_report_the_same_rotation_validation_error() {
    for (filename, source) in [
        ("view.crepus", "slot-rotate\n  span \"Bad\""),
        (
            "view.vue",
            "<template><slot-rotate><span>Bad</span></slot-rotate></template>",
        ),
        ("view.svelte", "<slot-rotate><span>Bad</span></slot-rotate>"),
        ("view.astro", "<slot-rotate><span>Bad</span></slot-rotate>"),
        ("view.ng", "<slot-rotate><span>Bad</span></slot-rotate>"),
        ("view.jsx", "<slot-rotate><span>Bad</span></slot-rotate>"),
    ] {
        let template = Template::from_source_with_path(source, filename);
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
        terminal
            .draw(|frame| {
                let error = template.draw_full(frame).unwrap_err();
                assert!(
                    error.contains("only allows quoted text lines"),
                    "{filename}: {error}"
                );
            })
            .unwrap();
    }
}
