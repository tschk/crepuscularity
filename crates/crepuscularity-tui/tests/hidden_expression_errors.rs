use crepuscularity_core::{eval::eval_expr, parse_template, TemplateContext, TemplateValue};
use crepuscularity_tui::{render_template_to_buffer, BufferRenderer};
use ratatui::{buffer::Buffer, layout::Rect};

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn malformed_hidden_expression_returns_its_original_error_before_paint() {
    let source = "span \"BEFORE\"\ndiv hidden={1 +}\n  span \"SHOULD-NOT-APPEAR\"";
    parse_template(source).unwrap();
    let area = Rect::new(2, 3, 40, 5);
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_symbol("X");
    }
    let previous = buffer.clone();
    let error =
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap_err();
    assert_eq!(
        error.to_string(),
        eval_expr("1 +", &TemplateContext::new())
            .unwrap_err()
            .to_string()
    );
    assert_eq!(buffer, previous);
}

#[test]
fn hidden_ancestors_and_resolved_hidden_classes_keep_errors_lazy() {
    for source in [
        "if false\n  div hidden={1 +}\n    span \"BAD\"\nspan \"VISIBLE\"",
        "div hidden\n  div hidden={1 +}\n    span \"BAD\"\nspan \"VISIBLE\"",
        "div class=\"hidden\" hidden={1 +}\n  span \"BAD\"\nspan \"VISIBLE\"",
        "div hidden hidden={1 +}\n  span \"BAD\"\nspan \"VISIBLE\"",
    ] {
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"), "{source}");
        assert!(!text(&buffer).contains("BAD"), "{source}");
    }
}

#[test]
fn valid_hidden_values_and_renderer_recovery_preserve_presence_semantics() {
    let mut renderer = BufferRenderer::new();
    let invalid = parse_template("div hidden={1 +}\n  span \"BAD\"").unwrap();
    let valid = parse_template("div hidden={hide}\n  span \"CONTENT\"").unwrap();
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    let mut ctx = TemplateContext::new();
    assert!(renderer
        .render_nodes(&invalid, &ctx, &mut buffer, area)
        .is_err());
    for (value, hidden) in [
        (TemplateValue::Bool(false), false),
        (TemplateValue::Null, false),
        (TemplateValue::Bool(true), true),
        (TemplateValue::Str("false".into()), true),
        (TemplateValue::Str("".into()), true),
    ] {
        ctx.set("hide", value);
        buffer.reset();
        renderer
            .render_nodes(&valid, &ctx, &mut buffer, area)
            .unwrap();
        assert_eq!(text(&buffer).contains("CONTENT"), !hidden);
    }
    ctx.vars.remove("hide");
    buffer.reset();
    renderer
        .render_nodes(&valid, &ctx, &mut buffer, area)
        .unwrap();
    assert!(text(&buffer).contains("CONTENT"));
}
