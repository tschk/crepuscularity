use crepuscularity_core::{eval::eval_expr, parse_template, TemplateContext, TemplateValue};
use crepuscularity_tui::{render_template_to_buffer, BufferRenderer};
use ratatui::{buffer::Buffer, layout::Rect};

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn active_control_expression_errors_preserve_the_original_error_and_pixels() {
    for source in [
        "if {1 +}\n  span \"BAD\"",
        "for item in {1 +}\n  span \"BAD\"",
        "match {1 +}\n  _ =>\n    span \"BAD\"",
    ] {
        parse_template(source).unwrap();
        let ctx = TemplateContext::new();
        let expected = eval_expr("1 +", &ctx).unwrap_err().to_string();
        let area = Rect::new(2, 3, 40, 5);
        let mut buffer = Buffer::empty(area);
        for cell in &mut buffer.content {
            cell.set_symbol("X");
        }
        let previous = buffer.clone();
        let error = render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap_err();
        assert_eq!(error.to_string(), expected, "{source}");
        assert_eq!(buffer, previous, "{source}");
    }
}

#[test]
fn inactive_branches_and_empty_loops_do_not_evaluate_nested_controls() {
    let mut ctx = TemplateContext::new();
    ctx.set("empty", TemplateValue::List(vec![]));
    for source in [
        "if false\n  if {1 +}\n    span \"BAD\"\nelse\n  span \"VISIBLE\"",
        "if false\n  for item in {1 +}\n    span \"BAD\"\nspan \"VISIBLE\"",
        "for item in empty\n  match {1 +}\n    _ =>\n      span \"BAD\"\nspan \"VISIBLE\"",
        "match 'chosen'\n  'other' =>\n    if {1 +}\n      span \"BAD\"\n  _ =>\n    span \"VISIBLE\"",
    ] {
        let area=Rect::new(0,0,40,5);
        let mut buffer=Buffer::empty(area);
        render_template_to_buffer(source,&ctx,&mut buffer,area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"),"{source}");
        assert!(!text(&buffer).contains("BAD"),"{source}");
    }
}

#[test]
fn missing_null_and_nonlist_iterators_keep_empty_loop_compatibility() {
    let mut ctx = TemplateContext::new();
    ctx.set("nil", TemplateValue::Null);
    for iterator in ["missing", "nil", "null", "false", "42"] {
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        let source =
            format!("for item in {{{iterator}}}\n  if {{1 +}}\n    span \"BAD\"\nspan \"VISIBLE\"");
        render_template_to_buffer(&source, &ctx, &mut buffer, area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"), "{iterator}");
    }
}

#[test]
fn the_same_renderer_recovers_after_each_control_error() {
    let mut renderer = BufferRenderer::new();
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    let ctx = TemplateContext::new();
    let valid = parse_template("span \"RECOVERED\"").unwrap();
    for source in [
        "if {1 +}\n  span \"BAD\"",
        "for item in {1 +}\n  span \"BAD\"",
        "match {1 +}\n  _ =>\n    span \"BAD\"",
    ] {
        let invalid = parse_template(source).unwrap();
        assert!(renderer
            .render_nodes(&invalid, &ctx, &mut buffer, area)
            .is_err());
        renderer
            .render_nodes(&valid, &ctx, &mut buffer, area)
            .unwrap();
        assert!(text(&buffer).contains("RECOVERED"));
    }
}
