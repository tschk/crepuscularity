use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

#[test]
fn includes_use_the_included_filename_frontend() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("child.vue"),
        "<script>const ignored = 1 < 2;</script><template><div>Included Vue</div></template>",
    )
    .unwrap();
    let template =
        Template::from_source_with_path("include child.vue", dir.path().join("entry.crepus"));
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Included Vue"), "{output}");
}

#[test]
fn vue_filename_selects_frontend_for_drawing_and_focus() {
    let mut template = Template::from_source_with_path(
        "<script>const ignored = 1 < 2;</script><template><div id=\"entry\" class=\"focusable\">Hello</div></template>",
        "view.vue",
    );
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["entry"]);
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(output.contains("Hello"), "{output}");
    terminal
        .draw(|frame| {
            template.draw_full_if_changed(frame).unwrap();
        })
        .unwrap();
}
