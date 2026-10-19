use crepuscularity_tui::{DispatchResult, Event, EventDispatcher, FocusManager};
use std::sync::{Arc, Mutex};

#[test]
fn duplicate_focus_ids_keep_first_occurrence_and_do_not_trap_navigation() {
    let mut focus = FocusManager::new();
    focus.set_focusable_ids(["a", "a", "b", "c", "b"].map(String::from).to_vec());
    assert_eq!(focus.focusable_ids, ["a", "b", "c"]);
    assert_eq!(focus.focus_next(), Some("b"));
    assert_eq!(focus.focus_next(), Some("c"));
    assert_eq!(focus.focus_next(), Some("a"));
    assert_eq!(focus.focus_prev(), Some("c"));
}

#[test]
fn replacing_duplicate_ids_preserves_focus_and_removal_selects_first_id() {
    let mut focus = FocusManager::new();
    focus.set_focusable_ids(["a", "b"].map(String::from).to_vec());
    assert!(focus.focus("b"));
    focus.set_focusable_ids(["c", "b", "c", "a", "b"].map(String::from).to_vec());
    assert_eq!(focus.focusable_ids, ["c", "b", "a"]);
    assert_eq!(focus.focused_id.as_deref(), Some("b"));
    assert_eq!(focus.focus_next(), Some("a"));
    focus.set_focusable_ids(["c", "c"].map(String::from).to_vec());
    assert_eq!(focus.focused_id.as_deref(), Some("c"));
    focus.set_focusable_ids(Vec::new());
    assert_eq!(focus.focused_id, None);
}

fn track_callback(
    dispatcher: &mut EventDispatcher,
    id: &'static str,
    calls: &Arc<Mutex<Vec<&'static str>>>,
    handles: bool,
) {
    let calls = Arc::clone(calls);
    dispatcher.on(id, move |_| {
        let mut calls = calls.lock().unwrap();
        // A repeated callback handles the event so the unfixed implementation cannot hang this test.
        let repeated = calls.contains(&id);
        calls.push(id);
        handles || repeated
    });
}

#[test]
fn self_parent_dispatches_each_callback_once() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut dispatcher = EventDispatcher::new();
    dispatcher.set_parent("self", "self");
    track_callback(&mut dispatcher, "self", &calls, false);
    assert_eq!(
        dispatcher.dispatch(&Event::FocusGained, Some("self")),
        DispatchResult::NotHandled
    );
    assert_eq!(*calls.lock().unwrap(), ["self"]);
}

#[test]
fn cyclic_parents_stop_after_the_first_visit_including_missing_callbacks() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut dispatcher = EventDispatcher::new();
    dispatcher.set_parent("leaf", "a");
    dispatcher.set_parent("a", "missing");
    dispatcher.set_parent("missing", "b");
    dispatcher.set_parent("b", "a");
    for id in ["leaf", "a", "b"] {
        track_callback(&mut dispatcher, id, &calls, false);
    }
    assert_eq!(
        dispatcher.dispatch(&Event::FocusGained, Some("leaf")),
        DispatchResult::NotHandled
    );
    assert_eq!(*calls.lock().unwrap(), ["leaf", "a", "b"]);
}

#[test]
fn a_handler_still_stops_bubbling_before_a_parent_cycle() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut dispatcher = EventDispatcher::new();
    dispatcher.set_parent("leaf", "parent");
    dispatcher.set_parent("parent", "leaf");
    track_callback(&mut dispatcher, "leaf", &calls, false);
    track_callback(&mut dispatcher, "parent", &calls, true);
    assert_eq!(
        dispatcher.dispatch(&Event::FocusGained, Some("leaf")),
        DispatchResult::Handled
    );
    assert_eq!(*calls.lock().unwrap(), ["leaf", "parent"]);
}

#[test]
fn deep_valid_parent_chains_are_not_truncated() {
    let mut dispatcher = EventDispatcher::new();
    for i in 0..5000 {
        dispatcher.set_parent(&i.to_string(), &(i + 1).to_string());
    }
    dispatcher.on("5000", |_| true);
    assert_eq!(
        dispatcher.dispatch(&Event::FocusGained, Some("0")),
        DispatchResult::Handled
    );
    assert_eq!(
        dispatcher.dispatch(&Event::FocusGained, None),
        DispatchResult::NotHandled
    );
}

#[test]
fn automatic_template_focus_does_not_get_stuck_on_duplicate_ids() {
    use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEvent, KeyModifiers};
    let mut template = crepuscularity_tui::Template::from_source(
        "<div><button id=\"a\" class=\"focusable\">One</button><button id=\"a\" class=\"focusable\">Duplicate</button><button id=\"b\" class=\"focusable\">Two</button></div>",
    );
    let tab = TerminalEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    template.handle_event(&tab);
    assert_eq!(template.context().get_str("focused_id"), "b");
    template.handle_event(&tab);
    assert_eq!(template.context().get_str("focused_id"), "a");
}

#[test]
fn cycles_without_callbacks_terminate() {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut dispatcher = EventDispatcher::new();
        dispatcher.set_parent("a", "b");
        dispatcher.set_parent("b", "a");
        sender
            .send(dispatcher.dispatch(&Event::FocusGained, Some("a")))
            .unwrap();
    });
    assert_eq!(
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        DispatchResult::NotHandled
    );
}
