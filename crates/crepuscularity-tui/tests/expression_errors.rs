use crepuscularity_tui::{render_template_to_buffer, BufferRenderer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn active_malformed_text_raw_expressions_and_declarations_fail_before_paint() {
    for source in [
        "\"before:{1 +}:after\"",
        "span\n  \"before:{1 +}:after\"",
        "{1 +}",
        "div\n  {1 +}",
        "{!1 +}",
        "$: let label = 1 +\nspan \"after\"",
        "span\n  $: let label = 1 +\n  \"after\"",
    ] {
        // Ensure these are runtime expression failures, not malformed DSL fixtures.
        crepuscularity_core::parse_template(source).unwrap();
        let area = Rect::new(2, 3, 40, 5);
        let mut buffer = Buffer::empty(area);
        for cell in &mut buffer.content {
            cell.set_symbol("X");
        }
        let previous = buffer.clone();
        assert!(
            render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).is_err(),
            "{source}"
        );
        assert_eq!(buffer, previous, "{source}");
    }
}

#[test]
fn buffer_renderer_recovers_after_an_expression_error() {
    let invalid = crepuscularity_core::parse_template("span\n  \"{1 +}\"").unwrap();
    let valid = crepuscularity_core::parse_template("span\n  \"RECOVERED\"").unwrap();
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    assert!(renderer
        .render_nodes(&invalid, &TemplateContext::new(), &mut buffer, area)
        .is_err());
    renderer
        .render_nodes(&valid, &TemplateContext::new(), &mut buffer, area)
        .unwrap();
    assert!(text(&buffer).contains("RECOVERED"));
}

#[test]
fn inactive_and_hidden_expressions_remain_lazy_and_satisfied_defaults_are_skipped() {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "PROVIDED");
    for source in [
        "if false\n  span \"{1 +}\"\nelse\n  span \"VISIBLE\"",
        "div hidden\n  {1 +}\nspan \"VISIBLE\"",
        "div class=\"hidden\"\n  $: let label = 1 +\nspan \"VISIBLE\"",
        "$: default label = 1 +\nspan \"VISIBLE:{label}\"",
        "span\n  $: default label = 1 +\n  \"VISIBLE:{label}\"",
    ] {
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
        assert!(text(&buffer).contains("VISIBLE"), "{source}");
    }
}

#[test]
fn missing_variables_keep_empty_text_and_null_declaration_behavior() {
    for source in [
        "span \"before:{missing}:after\"",
        "span\n  {missing}\n  \"before::after\"",
        "$: let absent = missing\nspan \"before:{absent}:after\"",
    ] {
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap();
        assert!(
            text(&buffer).contains("before::after"),
            "{source}: {}",
            text(&buffer)
        );
    }
}

#[test]
fn component_defaults_fail_atomically_or_skip_when_already_provided() {
    let mut file =
        crepuscularity_core::parser::parse_component_file("--- Card\nspan \"{label}\"").unwrap();
    let mut component = file.components.remove("Card").unwrap();
    component.meta.defaults.insert("label".into(), "1 +".into());
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_symbol("X");
    }
    let previous = buffer.clone();
    let mut renderer = BufferRenderer::new();
    assert!(renderer
        .render_component(&component, &TemplateContext::new(), &mut buffer, area)
        .is_err());
    assert_eq!(buffer, previous);
    let mut ctx = TemplateContext::new();
    ctx.set("label", "PROVIDED");
    renderer
        .render_component(&component, &ctx, &mut buffer, area)
        .unwrap();
    assert!(text(&buffer).contains("PROVIDED"));
}

#[test]
fn the_first_evaluator_error_is_preserved_across_bad_siblings() {
    let source = "span \"{1 +}\"\nspan \"{2 *}\"";
    let expected = crepuscularity_core::eval::eval_expr("1 +", &TemplateContext::new())
        .unwrap_err()
        .to_string();
    let area = Rect::new(0, 0, 40, 5);
    let mut buffer = Buffer::empty(area);
    let error =
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap_err();
    assert_eq!(error.to_string(), expected);
}

#[test]
fn included_and_caller_slot_expression_errors_propagate_before_paint() {
    for bad in ["span \"{1 +}\"", "$: let label = 1 +\nspan \"{label}\""] {
        for caller_slot in [false, true] {
            let mut ctx = TemplateContext::new();
            let source = if caller_slot {
                std::sync::Arc::make_mut(&mut ctx.virtual_files)
                    .insert("child.crepus".into(), "section\n  slot".into());
                format!("include child.crepus\n  {}", bad.replace('\n', "\n  "))
            } else {
                std::sync::Arc::make_mut(&mut ctx.virtual_files)
                    .insert("child.crepus".into(), bad.into());
                "include child.crepus".into()
            };
            crepuscularity_core::parse_template(&source).unwrap();
            let area = Rect::new(0, 0, 40, 5);
            let mut buffer = Buffer::empty(area);
            for cell in &mut buffer.content {
                cell.set_symbol("X");
            }
            let previous = buffer.clone();
            assert!(
                render_template_to_buffer(&source, &ctx, &mut buffer, area).is_err(),
                "{source}"
            );
            assert_eq!(buffer, previous, "{source}");
        }
    }
}

#[test]
fn cached_template_retries_expression_errors_without_losing_the_last_good_frame() {
    use crepuscularity_tui::Template;
    use ratatui::{backend::TestBackend, Terminal};
    let mut template = Template::from_source("span \"BEFORE\"");
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let previous = terminal.backend().buffer().clone();
    template.set_source("span \"{1 +}\"");
    for _ in 0..2 {
        terminal
            .draw(|frame| assert!(template.draw_full_if_changed(frame).is_err()))
            .unwrap();
        assert_eq!(terminal.backend().buffer(), &previous);
    }
    template.set_source("span \"RECOVERED\"");
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    assert!(text(terminal.backend().buffer()).contains("RECOVERED"));
}
