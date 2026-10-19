use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

#[test]
fn cached_cells_preserve_unicode_style_and_neighbor_widgets() {
    use ratatui::{
        layout::Rect,
        style::{Color, Style},
        widgets::Paragraph,
    };
    let mut template = Template::from_source("span text-red-500 \"雪界\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    let area = Rect::new(0, 0, 5, 1);
    terminal
        .draw(|frame| assert!(template.draw_if_changed(frame, area).unwrap()))
        .unwrap();
    let original = terminal.backend().buffer()[(0, 0)].clone();
    assert_ne!(original.fg, Color::Reset);
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new("Neighbor").style(Style::default().fg(Color::Green)),
                Rect::new(5, 0, 15, 1),
            );
            assert!(!template.draw_if_changed(frame, area).unwrap());
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)], original);
    assert_eq!(buffer[(0, 0)].symbol(), "雪");
    assert_eq!(buffer[(2, 0)].symbol(), "界");
    assert_eq!(buffer[(5, 0)].symbol(), "N");
    assert_eq!(buffer[(5, 0)].fg, Color::Green);
}

#[test]
fn expanded_frame_repaints_newly_visible_cached_area() {
    let mut template = Template::from_source("span \"Newly visible\"");
    let mut terminal = Terminal::new(TestBackend::new(5, 2)).unwrap();
    let area = ratatui::layout::Rect::new(0, 0, 20, 1);
    terminal
        .draw(|frame| assert!(template.draw_if_changed(frame, area).unwrap()))
        .unwrap();
    terminal.backend_mut().resize(20, 2);
    terminal
        .resize(ratatui::layout::Rect::new(0, 0, 20, 2))
        .unwrap();
    terminal
        .draw(|frame| assert!(template.draw_if_changed(frame, area).unwrap()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Newly visible"), "{output}");
}

#[test]
fn unchanged_conditional_draw_preserves_visible_content() {
    let mut template = Template::from_source("span \"Still visible\"");
    let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    terminal
        .draw(|frame| assert!(!template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Still visible"), "{output}");
}

#[test]
fn changed_draw_area_invalidates_layout() {
    let mut template = Template::from_source("span \"Layout\"");
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    let first = ratatui::layout::Rect::new(0, 0, 10, 1);
    let second = ratatui::layout::Rect::new(10, 1, 20, 1);
    terminal
        .draw(|frame| assert!(template.draw_if_changed(frame, first).unwrap()))
        .unwrap();
    terminal
        .draw(|frame| assert!(template.draw_if_changed(frame, second).unwrap()))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(10, 1)].symbol(), "L");
}

#[test]
fn source_changes_refresh_focus_and_preserve_existing_target() {
    let mut template = Template::from_source("input #old focusable");
    template.refresh_focus();
    assert_eq!(template.focus().focused_id.as_deref(), Some("old"));
    template.set_source("input #new focusable");
    assert_eq!(template.focus().focusable_ids, ["new"]);
    assert_eq!(template.focus().focused_id.as_deref(), Some("new"));
    template.set_source("input #first focusable\ninput #new focusable");
    assert_eq!(template.focus().focused_id.as_deref(), Some("new"));
    template.set_source("span");
    assert!(template.focus().focusable_ids.is_empty());
    assert_eq!(template.focus().focused_id, None);
}

#[test]
fn source_changes_invalidate_conditional_drawing() {
    let mut template = Template::from_source("span \"Before\"");
    template.mark_rendered();
    template.set_source("span \"After\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("After"));
    template.set_source("span \"After\"");
    terminal
        .draw(|frame| assert!(!template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
}

#[test]
fn reload_invalidates_only_after_successful_source_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("view.crepus");
    std::fs::write(&path, "span \"Before\"").unwrap();
    let mut template = Template::from_path(&path).unwrap();
    template.mark_rendered();
    std::fs::write(&path, "span \"After\"").unwrap();
    template.reload().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(template.reload().is_err());
    terminal
        .draw(|frame| assert!(!template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
}

#[test]
fn malformed_reload_preserves_last_valid_source_and_render_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("view.crepus");
    std::fs::write(&path, "span \"Before\"").unwrap();
    let mut template = Template::from_path(&path).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| {
            template.draw_full_if_changed(frame).unwrap();
        })
        .unwrap();
    std::fs::write(&path, "if {").unwrap();
    assert!(template.reload().is_err());
    assert_eq!(template.source(), "span \"Before\"");
    terminal
        .draw(|frame| assert!(!template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    std::fs::write(&path, "span \"After\"").unwrap();
    template.reload().unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    assert!(terminal
        .backend()
        .buffer()
        .content
        .iter()
        .any(|cell| cell.symbol() == "A"));
}

#[test]
fn failed_changed_draw_repaints_last_successful_frame() {
    let mut template = Template::from_source("span \"Before\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| {
            template.draw_full_if_changed(frame).unwrap();
        })
        .unwrap();
    let before = terminal.backend().buffer().clone();
    template.set_source("if {");
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).is_err()))
        .unwrap();
    assert_eq!(terminal.backend().buffer(), &before);
}

#[test]
fn failed_draw_with_smaller_area_preserves_neighbor_widgets() {
    use ratatui::{layout::Rect, widgets::Paragraph};
    let mut template = Template::from_source("span \"Original\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| {
            template
                .draw_if_changed(frame, Rect::new(0, 0, 10, 1))
                .unwrap();
        })
        .unwrap();
    template.set_source("if {");
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("Neighbor"), Rect::new(3, 0, 8, 1));
            assert!(template
                .draw_if_changed(frame, Rect::new(0, 0, 3, 1))
                .is_err());
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let text: String = (0..11).map(|x| buffer[(x, 0)].symbol()).collect();
    assert_eq!(text, "OriNeighbor");
}
