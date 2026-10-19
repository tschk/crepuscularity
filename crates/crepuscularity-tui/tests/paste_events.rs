use crepuscularity_tui::{Event, EventResult, Template};
use crossterm::event::Event as TerminalEvent;
use std::sync::{Arc, Mutex};

#[test]
fn pasted_text_is_not_reported_as_focus_loss() {
    let event = Event::from(TerminalEvent::Paste("界\r\npasted\ttext".into()));
    assert_eq!(event, Event::Paste("界\r\npasted\ttext".into()));
}

#[test]
fn paste_reaches_the_focused_callback_without_triggering_tab_navigation() {
    let mut template =
        Template::from_source("div\n  input #first focusable\n  input #second focusable");
    template.refresh_focus();
    let events = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&events);
    template.dispatcher_mut().on("first", move |event| {
        received.lock().unwrap().push(event.clone());
        true
    });
    assert_eq!(
        template.handle_event(&TerminalEvent::Paste("\t\n\u{1b}[D".into())),
        EventResult::Handled
    );
    assert_eq!(template.focus().focused_id.as_deref(), Some("first"));
    assert_eq!(template.context().get_str("focused_id"), "first");
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], Event::Paste("\t\n\u{1b}[D".into()));
}

#[test]
fn conversion_preserves_empty_unicode_and_control_character_payloads() {
    for text in [
        "",
        "plain text",
        "界 e\u{301} 👩‍💻",
        "a\r\nb\nc\t",
        "\0\u{1b}[200~payload\u{1b}[201~",
    ] {
        assert_eq!(
            Event::from(TerminalEvent::Paste(text.into())),
            Event::Paste(text.into())
        );
    }
}

#[test]
fn paste_bubbles_without_changing_the_payload() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut template = Template::from_source("input #input focusable");
    for (id, handled) in [("input", false), ("parent", true), ("root", true)] {
        let calls = Arc::clone(&calls);
        template.dispatcher_mut().on(id, move |event| {
            calls.lock().unwrap().push((id, event.clone()));
            handled
        });
    }
    template.dispatcher_mut().parent("input", "parent");
    template.dispatcher_mut().parent("parent", "root");
    let text = "hello\n界\t";
    assert_eq!(
        template.handle_event(&TerminalEvent::Paste(text.into())),
        EventResult::Handled
    );
    assert_eq!(
        *calls.lock().unwrap(),
        [
            ("input", Event::Paste(text.into())),
            ("parent", Event::Paste(text.into()))
        ]
    );
}

#[test]
fn unhandled_paste_keeps_the_current_focus() {
    let mut template = Template::from_source("input #input focusable");
    template.refresh_focus();
    assert_eq!(
        template.handle_event(&TerminalEvent::Paste("\t".into())),
        EventResult::Unhandled
    );
    assert_eq!(template.focus().focused_id.as_deref(), Some("input"));
}

#[test]
fn other_terminal_event_conversions_preserve_all_fields() {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent, MouseEventKind,
    };
    let key = KeyEvent::new_with_kind_and_state(
        KeyCode::Char('界'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
        KeyEventKind::Release,
        KeyEventState::CAPS_LOCK | KeyEventState::NUM_LOCK,
    );
    let mouse = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 9,
        row: 7,
        modifiers: KeyModifiers::SHIFT,
    };
    for (input, expected) in [
        (TerminalEvent::Key(key), Event::Key(key)),
        (TerminalEvent::Mouse(mouse), Event::Mouse(mouse)),
        (TerminalEvent::Resize(123, 45), Event::Resize(123, 45)),
        (TerminalEvent::FocusGained, Event::FocusGained),
        (TerminalEvent::FocusLost, Event::FocusLost),
    ] {
        assert_eq!(Event::from(input), expected);
    }
}
