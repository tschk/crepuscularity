use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

#[test]
fn conditional_draws_refresh_rotating_phrases_without_context_changes() {
    let mut template = Template::from_source("slot-rotate interval=20\n  \"First\"\n  \"Second\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| assert!(template.draw_full_if_changed(frame).unwrap()))
        .unwrap();
    let initial: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(initial.contains("First") || initial.contains("Second"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
    let mut redrawn = false;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
        terminal
            .draw(|frame| redrawn = template.draw_full_if_changed(frame).unwrap())
            .unwrap();
        if redrawn {
            break;
        }
    }
    assert!(redrawn, "rotating text stayed cached past its interval");
}
