use crepuscularity_core::{eval::eval_expr, parse_template, TemplateContext};
use crepuscularity_tui::{render_template_to_buffer, BufferRenderer};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn active_class_errors_return_the_original_evaluator_error_before_paint() {
    for (source, expression) in [
        ("span class=\"{1+}\"\n  \"BAD\"", "1+"),
        ("span class:text-red-500={1 +}\n  \"BAD\"", "1 +"),
        ("slot-rotate class=\"{1+}\"\n  \"ONE\"\n  \"TWO\"", "1+"),
    ] {
        parse_template(source).unwrap();
        let area = Rect::new(2, 3, 40, 5);
        let mut buffer = Buffer::empty(area);
        for cell in &mut buffer.content {
            cell.set_symbol("X");
        }
        let previous = buffer.clone();
        let expected = eval_expr(expression, &TemplateContext::new())
            .unwrap_err()
            .to_string();
        let error = render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area)
            .unwrap_err();
        assert_eq!(error.to_string(), expected, "{source}");
        assert_eq!(buffer, previous, "{source}");
    }
}

#[test]
fn conditional_class_values_are_interpolated_only_when_selected() {
    let source = "span class:text-{1+}={enabled}\n  \"VISIBLE\"";
    parse_template(source).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", false);
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    assert!(text(&buffer).contains("VISIBLE"));
    ctx.set("enabled", true);
    let previous = buffer.clone();
    let error = render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap_err();
    assert_eq!(
        error.to_string(),
        eval_expr("1+", &ctx).unwrap_err().to_string()
    );
    assert_eq!(buffer, previous);
}

#[test]
fn inactive_and_hidden_subtrees_do_not_resolve_descendant_classes() {
    for source in [
        "if false\n  span class=\"{1+}\"\n    \"BAD\"\nspan \"VISIBLE\"",
        "div hidden\n  span class:text-red-500={1 +}\n    \"BAD\"\nspan \"VISIBLE\"",
        "div class=\"hidden\"\n  slot-rotate class=\"{1+}\"\n    \"ONE\"\n    \"TWO\"\nspan \"VISIBLE\"",
    ] {
        let area=Rect::new(0,0,40,5);
        let mut buffer=Buffer::empty(area);
        render_template_to_buffer(source,&TemplateContext::new(),&mut buffer,area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"),"{source}");
    }
}

#[test]
fn renderer_recovers_and_observes_valid_and_missing_dynamic_class_values() {
    let mut renderer = BufferRenderer::new();
    let mut ctx = TemplateContext::new();
    let invalid = parse_template("span class=\"{1+}\"\n  \"BAD\"").unwrap();
    let valid = parse_template("span class=\"{tone}\"\n  \"VISIBLE\"").unwrap();
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    assert!(renderer
        .render_nodes(&invalid, &ctx, &mut buffer, area)
        .is_err());
    for (tone, expected) in [
        (Some("text-[#ff0000]"), Some(Color::Rgb(255, 0, 0))),
        (Some("text-[#0000ff]"), Some(Color::Rgb(0, 0, 255))),
        (None, None),
    ] {
        if let Some(tone) = tone {
            ctx.set("tone", tone);
        } else {
            ctx.vars.remove("tone");
        }
        buffer.reset();
        renderer
            .render_nodes(&valid, &ctx, &mut buffer, area)
            .unwrap();
        assert!(text(&buffer).contains("VISIBLE"));
        let first = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "V")
            .unwrap();
        if let Some(expected) = expected {
            assert_eq!(first.fg, expected);
        } else {
            assert_eq!(first.fg, Color::Reset);
        }
    }
}
