use crepuscularity_core::{eval::eval_expr, parse_template, TemplateContext};
use crepuscularity_tui::{render_template_to_buffer, BufferRenderer};
use ratatui::{buffer::Buffer, layout::Rect};
use std::sync::Arc;

fn context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).insert(
        "child.crepus".into(),
        "div\n  span \"child:{title}\"\n  slot".into(),
    );
    ctx
}
fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn malformed_active_include_props_return_the_original_error_before_paint() {
    let ctx = context();
    let source = "span \"BEFORE\"\ninclude child.crepus title={1 +}";
    parse_template(source).unwrap();
    let area = Rect::new(2, 3, 40, 6);
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_symbol("X");
    }
    let previous = buffer.clone();
    let expected = eval_expr("1 +", &ctx).unwrap_err().to_string();
    let error = render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap_err();
    assert_eq!(error.to_string(), expected);
    assert_eq!(buffer, previous);
}

#[test]
fn inactive_and_hidden_includes_do_not_evaluate_props() {
    let ctx = context();
    for source in [
        "if false\n  include child.crepus title={1 +}\nspan \"VISIBLE\"",
        "div hidden\n  include child.crepus title={1 +}\nspan \"VISIBLE\"",
        "div class=\"hidden\"\n  include child.crepus title={1 +}\nspan \"VISIBLE\"",
    ] {
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"), "{source}");
    }
}

#[test]
fn missing_props_remain_null_and_caller_slot_scope_is_preserved() {
    let mut ctx = context();
    ctx.set("title", "CALLER");
    let area = Rect::new(0, 0, 40, 8);
    for (source, expected) in [
        ("include child.crepus title={missing}", "child:"),
        (
            "include child.crepus title='CHILD'\n  span \"slot:{title}\"",
            "child:CHILD",
        ),
    ] {
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        let output = text(&buffer);
        assert!(output.contains(expected), "{output}");
        if source.contains("missing") {
            assert!(
                !output.contains("CALLER"),
                "missing prop must remain Null: {output}"
            );
        }
        if source.contains("slot:") {
            assert!(output.contains("slot:CALLER"), "{output}");
        }
        assert_eq!(ctx.get_str("title"), "CALLER");
    }
}

#[test]
fn buffer_renderer_recovers_with_valid_props_and_observes_new_context_values() {
    let mut ctx = context();
    let bad = parse_template("include child.crepus title={1 +}").unwrap();
    let good = parse_template("include child.crepus title={current}").unwrap();
    let area = Rect::new(0, 0, 40, 6);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    assert!(renderer
        .render_nodes(&bad, &ctx, &mut buffer, area)
        .is_err());
    for label in ["FIRST", "SECOND"] {
        ctx.set("current", label);
        renderer
            .render_nodes(&good, &ctx, &mut buffer, area)
            .unwrap();
        assert!(text(&buffer).contains(&format!("child:{label}")));
    }
}
