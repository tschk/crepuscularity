use crepuscularity_tui::{Template, TemplateContext};
use ratatui::{backend::TestBackend, Terminal};
use std::sync::Arc;

fn draw(template: &mut Template, terminal: &mut Terminal<TestBackend>) -> bool {
    let mut changed = false;
    terminal
        .draw(|frame| changed = template.draw_full_if_changed(frame).unwrap())
        .unwrap();
    changed
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
fn replacing_a_registered_component_invalidates_cached_cells() {
    let mut template = Template::from_source("include Card");
    template.register_component("Card", "span \"Before\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Before"));
    template.register_component("Card", "span \"After\"");
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn context_virtual_file_edits_and_removals_invalidate_cached_cells() {
    let mut template = Template::from_source("include child.crepus");
    Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("child.crepus".into(), "span \"Before\"".into());
    let mut terminal = Terminal::new(TestBackend::new(80, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("child.crepus".into(), "span \"After\"".into());
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
    Arc::make_mut(&mut template.context_mut().virtual_files).remove("child.crepus");
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("include error"));
}

#[test]
fn changing_the_include_base_directory_invalidates_cached_cells() {
    let dir = tempfile::tempdir().unwrap();
    for (name, label) in [("first", "Before"), ("second", "After")] {
        let path = dir.path().join(name);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("child.crepus"), format!("span \"{label}\"")).unwrap();
    }
    let mut template = Template::from_source("include child.crepus");
    template.context_mut().base_dir = Some(dir.path().join("first"));
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    template.context_mut().base_dir = Some(dir.path().join("second"));
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
}

#[test]
fn slot_nodes_and_caller_context_changes_invalidate_cached_cells() {
    let mut template = Template::from_source("slot\n  \"Fallback\"");
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Fallback"));
    let mut caller = TemplateContext::new();
    caller.set("label", "Before");
    template.context_mut().slot = Some((
        crepuscularity_core::parse_template("span \"{label}\"").unwrap(),
        Arc::new(caller),
    ));
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Before"));
    let slot = template.context_mut().slot.as_mut().unwrap();
    Arc::make_mut(&mut slot.1).set("label", "After");
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
    template.context_mut().slot.as_mut().unwrap().0 =
        crepuscularity_core::parse_template("span \"Changed nodes\"").unwrap();
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Changed nodes"));
    template.context_mut().slot = None;
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Fallback"));
}

#[test]
fn equivalent_context_resources_keep_cached_cells() {
    let mut template = Template::from_source("include child.crepus");
    Arc::make_mut(&mut template.context_mut().virtual_files)
        .insert("child.crepus".into(), "span \"Same\"".into());
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    let files = template.context().virtual_files.as_ref().clone();
    template.context_mut().virtual_files = Arc::new(files);
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn slot_caller_virtual_file_edits_invalidate_cached_cells() {
    let mut caller = TemplateContext::new();
    Arc::make_mut(&mut caller.virtual_files)
        .insert("child.crepus".into(), "span \"Before\"".into());
    let mut template = Template::from_source("slot");
    template.context_mut().slot = Some((
        crepuscularity_core::parse_template("include child.crepus").unwrap(),
        Arc::new(caller),
    ));
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("Before"));
    let caller = Arc::make_mut(&mut template.context_mut().slot.as_mut().unwrap().1);
    Arc::make_mut(&mut caller.virtual_files).insert("child.crepus".into(), "span \"After\"".into());
    assert!(draw(&mut template, &mut terminal));
    assert!(text(&terminal).contains("After"));
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn structurally_equivalent_slots_keep_cached_cells() {
    let mut caller = TemplateContext::new();
    caller.set("label", "Same");
    let mut template = Template::from_source("slot");
    template.context_mut().slot = Some((
        crepuscularity_core::parse_template("span \"{label}\"").unwrap(),
        Arc::new(caller),
    ));
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    assert!(draw(&mut template, &mut terminal));
    let (nodes, caller) = template.context_mut().slot.as_mut().unwrap();
    *nodes = crepuscularity_core::parse_template("span \"{label}\"").unwrap();
    *caller = Arc::new(caller.as_ref().clone());
    assert!(!draw(&mut template, &mut terminal));
}

#[test]
fn resource_changes_do_not_invent_changed_variable_names() {
    use crepuscularity_tui::DiffTracker;
    let mut context = TemplateContext::new();
    context.set("stable", "value");
    let mut tracker = DiffTracker::new();
    tracker.update(&context);
    context.base_dir = Some("changed-directory".into());
    assert!(tracker.has_changed(&context));
    assert!(tracker.changed_keys(&context).is_empty());
    tracker.update(&context);
    assert!(!tracker.has_changed(&context));
    Arc::make_mut(&mut context.virtual_files)
        .insert("child.crepus".into(), "span \"Changed\"".into());
    assert!(tracker.has_changed(&context));
    assert!(tracker.changed_keys(&context).is_empty());
    tracker.update(&context);
    assert!(!tracker.has_changed(&context));
}
