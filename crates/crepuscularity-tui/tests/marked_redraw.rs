use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

fn draw(template: &mut Template, terminal: &mut Terminal<TestBackend>) -> bool {
    let mut redrawn = false;
    terminal
        .draw(|frame| redrawn = template.draw_full_if_changed(frame).unwrap())
        .unwrap();
    redrawn
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn marking_variables_rendered_does_not_skip_the_first_cached_draw() {
    let mut template = Template::from_source("span \"Before\"");
    template.mark_rendered();
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Before"));
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn marking_changed_variables_does_not_validate_old_cached_cells() {
    let mut template = Template::from_source("span \"{label}\"");
    template.set("label", "Before");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    template.set("label", "After");
    template.mark_rendered();
    assert!(template.changed_keys().is_empty());
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn marking_unchanged_variables_preserves_cached_cells() {
    let mut template = Template::from_source("span \"Same\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    template.mark_rendered();
    assert!(!draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Same"));
}

#[test]
fn marked_failed_draws_still_return_errors_and_repaint_previous_cells() {
    let mut template = Template::from_source("span \"Before\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    let previous = terminal.backend().buffer().clone();
    template.set_source("if {");
    template.mark_rendered();
    for _ in 0..2 {
        terminal
            .draw(|frame| assert!(template.draw_full_if_changed(frame).is_err()))
            .unwrap();
        assert_eq!(terminal.backend().buffer(), &previous);
    }
    template.set_source("span \"After\"");
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
}

#[test]
fn marking_changed_virtual_files_does_not_validate_old_cached_cells() {
    let mut template = Template::from_source("include Card");
    template.register_component("Card", "span \"Before\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    template.register_component("Card", "span \"After\"");
    template.mark_rendered();
    assert!(template.changed_keys().is_empty());
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
}

#[test]
fn first_marked_draw_handles_empty_and_offset_regions() {
    use ratatui::layout::Rect;
    for area in [
        Rect::new(3, 1, 10, 1),
        Rect::new(3, 1, 0, 1),
        Rect::new(30, 20, 5, 2),
    ] {
        let mut template = Template::from_source("span \"Text\"");
        template.mark_rendered();
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| assert!(template.draw_if_changed(frame, area).unwrap()))
            .unwrap();
        terminal
            .draw(|frame| assert!(!template.draw_if_changed(frame, area).unwrap()))
            .unwrap();
        if area.width == 10 {
            assert_eq!(terminal.backend().buffer()[(3, 1)].symbol(), "T");
        }
    }
}
