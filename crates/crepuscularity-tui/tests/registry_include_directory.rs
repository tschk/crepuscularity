use crepuscularity_tui::{ComponentRegistry, TemplateContext};
use ratatui::{backend::TestBackend, Terminal};

fn rendered(registry: &ComponentRegistry, context: &TemplateContext) -> String {
    let mut terminal = Terminal::new(TestBackend::new(60, 3)).unwrap();
    terminal
        .draw(|frame| {
            registry
                .render("Card", &[], context, frame, frame.area())
                .unwrap();
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
#[ignore = "Known defect: file origins are discarded; fixing storage requires an API compatibility decision"]
fn registered_file_components_resolve_includes_beside_the_registration_file() {
    let dir = tempfile::tempdir().unwrap();
    let registered = dir.path().join("registered");
    let caller = dir.path().join("caller");
    std::fs::create_dir(&registered).unwrap();
    std::fs::create_dir(&caller).unwrap();
    let source = registered.join("cards.crepus");
    std::fs::write(&source, "--- Card\ninclude child.crepus").unwrap();
    std::fs::write(
        registered.join("child.crepus"),
        "span \"REGISTERED-SIBLING\"",
    )
    .unwrap();
    std::fs::write(caller.join("child.crepus"), "span \"WRONG-CALLER-SIBLING\"").unwrap();
    let mut registry = ComponentRegistry::new();
    registry.register_file(&source).unwrap();
    for base_dir in [None, Some(caller)] {
        let mut context = TemplateContext::new();
        context.base_dir = base_dir.clone();
        let text = rendered(&registry, &context);
        assert!(
            text.contains("REGISTERED-SIBLING"),
            "base_dir={base_dir:?}: {text}"
        );
        assert!(!text.contains("WRONG-CALLER-SIBLING"), "{text}");
        assert_eq!(
            context.base_dir, base_dir,
            "render must not mutate caller directory"
        );
    }
}

#[test]
fn in_memory_registration_keeps_the_callers_include_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("child.crepus"), "span \"CALLER-SIBLING\"").unwrap();
    let mut registry = ComponentRegistry::new();
    registry.register("Card", "include child.crepus");
    let mut context = TemplateContext::new();
    context.base_dir = Some(dir.path().into());
    assert!(rendered(&registry, &context).contains("CALLER-SIBLING"));
}
