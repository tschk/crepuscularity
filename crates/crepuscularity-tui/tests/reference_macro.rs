use ratatui::{backend::TestBackend, Terminal};

#[test]
fn generated_fields_render_without_colliding_with_storage() {
    let mut ui =
        crepuscularity_tui::template_refs!("tests/fixtures/reference_fields.crepus").unwrap();
    ui.template.text("Public template");
    ui.__crepus_template.text("Public storage");
    assert!(ui.find("#template").is_some());
    assert!(ui.get("__crepus_template").is_some());
    let _ = ui.template();
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal.draw(|frame| ui.draw_full(frame).unwrap()).unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Public template"), "{output}");
    assert!(output.contains("Public storage"), "{output}");
}
