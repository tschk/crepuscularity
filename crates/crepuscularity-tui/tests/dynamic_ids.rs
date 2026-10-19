use crepuscularity_tui::Template;
#[test]
fn dynamic_ids_participate_in_focus_refresh() {
    let mut template =
        Template::from_source("<button id={name} class=\"focusable\">Press</button>");
    template.set("name", "first");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["first"]);
    template.set("name", "second");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["second"]);
}

#[test]
fn initial_event_setup_discovers_dynamic_ids_and_focus_style_renders() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, style::Color, Terminal};
    let mut template = Template::from_source(
        "<div id={name} class=\"focusable focus:border-red-500\">Content</div>",
    );
    template.set("name", "entry");
    template.handle_event(&Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
    assert_eq!(template.focus().focusable_ids, ["entry"]);
    assert_eq!(template.context().get_str("focused_id"), "entry");
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_ne!(buffer[(0, 0)].fg, Color::Reset);
    assert_ne!(buffer[(0, 0)].symbol(), " ");
}

#[test]
fn focus_refresh_selects_visible_conditional_branch() {
    let source = "if enabled\n  button #yes focusable\nelse\n  button #no focusable";
    assert_eq!(
        crepuscularity_tui::collect_focusable_ids(source).unwrap(),
        ["yes", "no"]
    );
    let mut template = Template::from_source(source);
    template.set("enabled", true);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["yes"]);
    template.set("enabled", false);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["no"]);
    assert_eq!(template.context().get_str("focused_id"), "no");
}

#[test]
fn focus_discovery_uses_renderer_target_defaults() {
    let mut template = Template::from_source(
        "if is_tui\n  button #terminal focusable\nelse\n  button #other focusable",
    );
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["terminal"]);
    template.set("is_tui", false);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["other"]);
}

#[test]
fn focus_expands_loop_aliases_and_selects_match_arms() {
    use crepuscularity_tui::{TemplateContext, TemplateValue};
    let mut template = Template::from_source(
        r#"<for in={items} let="item"><if condition={item.visible}><button id={item.id} class="focusable">Press</button></if></for>"#,
    );
    let items = [("first", true), ("hidden", false), ("last", true)]
        .into_iter()
        .map(|(id, visible)| {
            let mut item = TemplateContext::new();
            item.set("id", id);
            item.set("visible", visible);
            item
        })
        .collect::<Vec<_>>();
    template.set("items", TemplateValue::List(items));
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["first", "last"]);
    template.set("items", TemplateValue::List(vec![]));
    template.refresh_focus();
    assert!(template.focus().focusable_ids.is_empty());

    let source = "match {status}\n  \"a\\nb\" =>\n    button #matched focusable\n  _ =>\n    button #fallback focusable";
    assert_eq!(
        crepuscularity_tui::collect_focusable_ids(source).unwrap(),
        ["matched", "fallback"]
    );
    let mut template = Template::from_source(source);
    template.set("status", "a\nb");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["matched"]);
    template.set("status", "other");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["fallback"]);
}

#[test]
fn focus_declarations_follow_source_order_and_stay_local() {
    let mut template = Template::from_source(
        r#"<div>
<button id={name} class="focusable">Before</button>
<let name="name" value={"after"}/>
<button id={name} class="focusable">After</button>
<let-default name="name" value={"ignored"}/>
<div><let name="name" value={"inner"}/><button id={name} class="focusable">Inner</button></div>
<button id={"sibling-" + name} class="focusable">Sibling</button>
</div>"#,
    );
    template.set("name", "before");
    template.refresh_focus();
    assert_eq!(
        template.focus().focusable_ids,
        ["before", "after", "inner", "sibling-after"]
    );
    assert_eq!(template.context().get_str("name"), "before");
}

#[test]
fn focus_uses_active_class_tokens_and_target_prefixes() {
    let mut template = Template::from_source(
        r#"<div>
<button id="dynamic" class={classes}>Dynamic</button>
<button id="conditional" class:focusable={enabled}>Conditional</button>
<button id="web" class="web:focusable">Web</button>
<button id="terminal" class="tui:focusable">Terminal</button>
<button id="substring" class="not-focusable">Substring</button>
</div>"#,
    );
    template.set("classes", "border focusable");
    template.set("enabled", true);
    template.refresh_focus();
    assert_eq!(
        template.focus().focusable_ids,
        ["dynamic", "conditional", "terminal"]
    );
    template.set("classes", "web:focusable border");
    template.set("enabled", false);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["terminal"]);
}

#[test]
fn dynamic_class_tokens_apply_render_styles_individually() {
    use ratatui::{backend::TestBackend, style::Color, Terminal};
    let mut template = Template::from_source(r#"<div id="entry" class={classes}>Content</div>"#);
    template.set("classes", "focusable focus:border-red-500");
    template.refresh_focus();
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_ne!(buffer[(0, 0)].fg, Color::Reset);
    assert_ne!(buffer[(0, 0)].symbol(), " ");
}

#[test]
fn events_refresh_context_dependent_focus_and_preserve_manual_lists() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let mut template = Template::from_source(
        "if enabled\n  button #visible focusable\nelse\n  button #replacement focusable",
    );
    template.set("enabled", true);
    let event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    template.handle_event(&event);
    assert_eq!(template.context().get_str("focused_id"), "visible");
    template.set("enabled", false);
    template.handle_event(&event);
    assert_eq!(template.focus().focusable_ids, ["replacement"]);
    assert_eq!(template.context().get_str("focused_id"), "replacement");
    template
        .focus_mut()
        .set_focusable_ids(vec!["manual".into()]);
    template.handle_event(&event);
    assert_eq!(template.focus().focusable_ids, ["manual"]);
    let mut cloned = template.clone();
    cloned.handle_event(&event);
    assert_eq!(cloned.focus().focusable_ids, ["manual"]);
    template.refresh_focus();
    template.set("enabled", true);
    template.handle_event(&event);
    assert_eq!(template.focus().focusable_ids, ["visible"]);
}

#[test]
fn invalid_source_clears_stale_focus_and_valid_source_recovers() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let mut template = Template::from_source("button #old focusable");
    template.refresh_focus();
    assert_eq!(template.context().get_str("focused_id"), "old");
    template.set_source("<div>");
    assert!(template.focus().focusable_ids.is_empty());
    assert_eq!(template.context().get_str("focused_id"), "");
    template.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(template.focus().focused_id.is_none());
    template.set_source("button #new focusable");
    assert_eq!(template.focus().focusable_ids, ["new"]);
    assert_eq!(template.context().get_str("focused_id"), "new");
}

#[test]
fn rotating_text_does_not_create_a_phantom_focus_target() {
    let source = "div\n  slot-rotate #rotation focusable\n    \"One\"\n    \"Two\"\n  button #actual focusable\n    \"Press\"";
    let mut template = Template::from_source(source);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["actual"]);
    assert_eq!(
        crepuscularity_tui::collect_focusable_ids(source).unwrap(),
        ["rotation", "actual"]
    );
}

#[test]
fn indentation_id_expressions_participate_in_focus() {
    let mut template = Template::from_source("button id={name} focusable");
    template.set("name", "entry");
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["entry"]);
}
