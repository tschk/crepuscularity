use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, style::Color, Terminal};

fn template(source: &str) -> Template {
    let mut template = Template::from_source(source);
    template.set("enabled", true);
    template.set("kind", "active");
    template.set(
        "items",
        TemplateValue::List(vec![TemplateContext::new(), TemplateContext::new()]),
    );
    std::sync::Arc::make_mut(&mut template.context_mut().virtual_files).extend([
        (
            "child.crepus".into(),
            "embed widget.js adapter=\"react\"".into(),
        ),
        (
            "named.crepus".into(),
            "--- Card\nembed widget.js adapter=\"react\"".into(),
        ),
        ("shell.crepus".into(), "slot".into()),
        (
            "fallback.crepus".into(),
            "slot\n  embed widget.js adapter=\"react\"".into(),
        ),
    ]);
    template
}

fn output(template: &Template) -> (String, String) {
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let cells = &terminal.backend().buffer().content;
    let all = cells.iter().map(|cell| cell.symbol()).collect::<String>();
    let red = cells
        .iter()
        .filter(|cell| cell.fg == Color::Red)
        .map(|cell| cell.symbol())
        .collect::<String>();
    (all, red)
}

#[test]
fn active_islands_render_red_diagnostics_in_all_structural_paths() {
    for (source, count) in [
        ("embed widget.js adapter=\"react\"", 1),
        ("div\n  embed widget.js adapter=\"react\"", 1),
        ("button\n  embed widget.js adapter=\"react\"", 1),
        ("if enabled\n  embed widget.js adapter=\"react\"", 1),
        (
            "match kind\n  \"active\" =>\n    embed widget.js adapter=\"react\"",
            1,
        ),
        ("for item in items\n  embed widget.js adapter=\"react\"", 2),
        ("include child.crepus", 1),
        ("include named.crepus#Card", 1),
        (
            "include shell.crepus\n  embed widget.js adapter=\"react\"",
            1,
        ),
    ] {
        let (text, red) = output(&template(source));
        assert_eq!(text.matches("widget.js").count(), count, "{source}: {text}");
        assert_eq!(red.matches("widget.js").count(), count, "{source}: {red}");
        assert!(
            text.contains("not supported by the TUI target"),
            "{source}: {text}"
        );
    }
}

#[test]
fn inactive_islands_and_unused_slot_fallbacks_stay_unevaluated() {
    for source in [
        "if is_web\n  embed widget.js adapter=\"react\"\nelse\n  span \"FALLBACK\"",
        "include fallback.crepus\n  span \"FALLBACK\"",
    ] {
        let (text, red) = output(&template(source));
        assert!(text.contains("FALLBACK"), "{text}");
        assert!(!text.contains("widget.js"), "{text}");
        assert!(red.trim().is_empty(), "{red}");
    }
}

#[test]
fn diagnostic_preserves_neighbor_text_and_disappears_after_context_update() {
    let mut template =
        template("\"BEFORE\"\nif enabled\n  embed widget.js adapter=\"react\"\n\"AFTER\"");
    let (text, _) = output(&template);
    let before = text.find("BEFORE").unwrap();
    let error = text.find("widget.js").unwrap();
    let after = text.find("AFTER").unwrap();
    assert!(before < error && error < after, "{text}");
    template.set("enabled", false);
    let (text, red) = output(&template);
    assert!(text.contains("BEFORE") && text.contains("AFTER"), "{text}");
    assert!(!text.contains("widget.js"), "{text}");
    assert!(red.trim().is_empty(), "{red}");
}
