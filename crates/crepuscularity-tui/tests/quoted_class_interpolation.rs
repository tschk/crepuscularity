use crepuscularity_tui::{
    ratatui::{backend::TestBackend, style::Color, Terminal},
    Template,
};

fn painted_text_color(template: &Template) -> Color {
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .find(|cell| cell.symbol() == "V")
        .expect("VALUE must paint")
        .fg
}

#[test]
fn quoted_class_expression_spaces_survive_parsing_and_context_changes() {
    let mut template =
        Template::from_source("span class=\"text-[{tone + ''}] font-bold\"\n  \"VALUE\"");
    for (tone, color) in [
        ("#ff0000", Color::Rgb(255, 0, 0)),
        ("#0000ff", Color::Rgb(0, 0, 255)),
    ] {
        template.set("tone", tone);
        assert_eq!(painted_text_color(&template), color);
    }
}

#[test]
fn quoted_when_classes_are_lazy_and_resolve_spaced_expressions_when_enabled() {
    let mut template =
        Template::from_source("span when:{enabled}=\"text-{1 +} font-bold\"\n  \"VALUE\"");
    template.set("enabled", false);
    assert_eq!(painted_text_color(&template), Color::Reset);
    template.set_source("span when:{enabled}=\"text-[{tone + ''}] font-bold\"\n  \"VALUE\"");
    template.set("enabled", true);
    for (tone, color) in [
        ("#ff0000", Color::Rgb(255, 0, 0)),
        ("#0000ff", Color::Rgb(0, 0, 255)),
    ] {
        template.set("tone", tone);
        assert_eq!(painted_text_color(&template), color);
    }
}
