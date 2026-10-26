use crepuscularity_tui::{BufferRenderer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn recursive_includes_render_warning_and_renderer_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let mut renderer = BufferRenderer::new();
    let area = Rect::new(0, 0, 80, 3);
    for (first, second) in [
        ("include first.crepus", ""),
        ("include second.crepus", "include first.crepus"),
    ] {
        std::fs::write(dir.path().join("first.crepus"), first).unwrap();
        std::fs::write(dir.path().join("second.crepus"), second).unwrap();
        let nodes = crepuscularity_core::parse_template("include first.crepus").unwrap();
        let mut buffer = Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buffer, area)
            .unwrap();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            output.contains("include nesting limit exceeded"),
            "{output}"
        );
    }
    std::fs::write(dir.path().join("first.crepus"), "div\n  \"Recovered\"").unwrap();
    let nodes = crepuscularity_core::parse_template("include first.crepus").unwrap();
    let mut buffer = Buffer::empty(area);
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(output.contains("Recovered"), "{output}");
}

#[test]
fn sixty_four_nested_includes_work_and_the_next_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..63 {
        std::fs::write(
            dir.path().join(format!("{index}.crepus")),
            format!("include {}.crepus", index + 1),
        )
        .unwrap();
    }
    std::fs::write(dir.path().join("63.crepus"), "div\n  \"At limit\"").unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let mut renderer = BufferRenderer::new();
    let nodes = crepuscularity_core::parse_template("include 0.crepus").unwrap();
    let area = Rect::new(0, 0, 80, 3);
    let mut buffer = Buffer::empty(area);
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(output.contains("At limit"), "{output}");
    std::fs::write(dir.path().join("63.crepus"), "include 64.crepus").unwrap();
    std::fs::write(dir.path().join("64.crepus"), "div\n  \"Too deep\"").unwrap();
    let mut buffer = Buffer::empty(area);
    renderer
        .render_nodes(&nodes, &ctx, &mut buffer, area)
        .unwrap();
    let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(
        output.contains("include nesting limit exceeded"),
        "{output}"
    );
}
