#![cfg(feature = "test-support")]
//! Synthetic TestPlatform fixtures. These do not establish native OS acceptance.
use crepuscularity_core::{ast::Node, parser::parse_template, TemplateContext};
use crepuscularity_runtime::{render_nodes_with_bindings, RuntimeBindings, TextInput};
use gpui::{
    prelude::*, App, Context, EntityInputHandler, Focusable, Render, TestAppContext, Window,
};
use std::{cell::RefCell, rc::Rc};

fn redraw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

struct TemplateFixture {
    nodes: Vec<Node>,
    context: TemplateContext,
    bindings: RuntimeBindings,
}
impl Render for TemplateFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_nodes_with_bindings("fixture", &self.nodes, &self.context, &self.bindings)
    }
}

#[gpui::test]
fn runtime_dispatch_captures_context_and_disabled_removes_listeners(cx: &mut TestAppContext) {
    let records = Rc::new(RefCell::new(Vec::new()));
    let output = records.clone();
    let bindings = RuntimeBindings::new().on("record", move |event, _, _| {
        output.borrow_mut().push(event.context.get_str("caption"));
    });
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut context = TemplateContext::new();
        context.set("caption", "first").set("disabled", false);
        TemplateFixture {
            context,
            bindings,
            nodes: parse_template("button w-[100px] h-[30px] disabled={disabled} @click=record")
                .unwrap(),
        }
    });
    redraw(cx);
    cx.simulate_click(gpui::point(gpui::px(5.), gpui::px(5.)), Default::default());
    assert_eq!(&*records.borrow(), &["first"]);
    view.update(cx, |view, cx| {
        view.context.set("disabled", true);
        cx.notify();
    });
    redraw(cx);
    cx.simulate_click(gpui::point(gpui::px(5.), gpui::px(5.)), Default::default());
    assert_eq!(records.borrow().len(), 1);
}

#[gpui::test]
fn entity_input_handler_keeps_composition_and_exposes_utf16_selection(cx: &mut TestAppContext) {
    let (field, cx) = cx.add_window_view(|window, cx| TextInput::new("a😀b", window, cx));
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.set_selected_text_range(1..3, window, cx);
            field.replace_and_mark_text_in_range(None, "拼", Some(1..1), window, cx);
            assert_eq!(field.marked_text_range(window, cx), Some(1..2));
            // An unchanged external value must not overwrite active composition.
            field.set_value("a😀b", cx);
            assert_eq!(field.model().text(), "a拼b");
            field.replace_text_in_range(None, "拼音", window, cx);
            assert_eq!(field.model().text(), "a拼音b");
            assert_eq!(
                field.selected_text_range(false, window, cx).unwrap().range,
                3..3
            );
            assert_eq!(field.text_length_utf16(window, cx), Some(4));
            assert!(field.accepts_text_input(window, cx));
            field.set_permissions(false, true, window, cx);
            field.replace_text_in_range(None, "blocked", window, cx);
            assert_eq!(field.model().text(), "a拼音b");
            assert!(!field.accepts_text_input(window, cx));
        });
    });
    cx.run_until_parked();
}

#[gpui::test]
fn platform_input_registration_survives_a_parent_render(cx: &mut TestAppContext) {
    let (field, cx) = cx.add_window_view(|window, cx| TextInput::new("", window, cx));
    cx.update(|window, cx| {
        let focus = field.read(cx).focus_handle(cx);
        focus.focus(window, cx);
    });
    redraw(cx);
    cx.simulate_input("draft");
    field.update(cx, |field, cx| field.set_value("", cx));
    redraw(cx);
    cx.update(|_, cx: &mut App| assert_eq!(field.read(cx).model().text(), "draft"));
}

#[gpui::test]
fn runtime_input_keeps_its_entity_when_the_template_rerenders(cx: &mut TestAppContext) {
    let drafts = Rc::new(RefCell::new(Vec::new()));
    let output = drafts.clone();
    let bindings = RuntimeBindings::new().on("edited", move |event, _, _| {
        if let crepuscularity_runtime::RuntimeEventKind::Input(input) = event.kind {
            output.borrow_mut().push(input.text.clone());
        }
    });
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut context = TemplateContext::new();
        context.set("value", "");
        TemplateFixture {
            context,
            bindings,
            nodes: parse_template("input #name value={value} @input=edited w-[200px] h-[30px]")
                .unwrap(),
        }
    });
    redraw(cx);
    cx.simulate_click(gpui::point(gpui::px(5.), gpui::px(5.)), Default::default());
    cx.simulate_input("draft");
    view.update(cx, |_, cx| cx.notify());
    redraw(cx);
    cx.simulate_input("X");
    assert_eq!(drafts.borrow().last().map(String::as_str), Some("draftX"));
}

#[gpui::test]
fn tab_navigation_includes_readonly_and_skips_disabled_inputs(cx: &mut TestAppContext) {
    let drafts = Rc::new(RefCell::new(Vec::new()));
    let output = drafts.clone();
    let bindings = RuntimeBindings::new().on("edited", move |event, _, _| {
        if let crepuscularity_runtime::RuntimeEventKind::Input(input) = event.kind {
            output.borrow_mut().push(input.text.clone());
        }
    });
    let (_, cx) = cx.add_window_view(|_, _| TemplateFixture {
        nodes: parse_template("div flex flex-col\n    input #readonly readonly @input=edited\n    input #disabled disabled @input=edited\n    input #editable @input=edited").unwrap(),
        context: TemplateContext::new(),
        bindings,
    });
    redraw(cx);
    let readonly = cx.update(|window, cx| {
        window.focus_next(cx);
        window
            .focused(cx)
            .expect("readonly field must be a tab stop")
    });
    redraw(cx);
    cx.simulate_input("blocked");
    assert!(drafts.borrow().is_empty());
    cx.update(|window, cx| {
        window.focus_next(cx);
        assert!(!readonly.is_focused(window));
    });
    redraw(cx);
    cx.simulate_input("ok");
    assert_eq!(drafts.borrow().last().map(String::as_str), Some("ok"));
    cx.update(|window, cx| {
        window.focus_prev(cx);
        assert!(readonly.is_focused(window));
    });
}

#[gpui::test]
fn native_shortcuts_preserve_word_navigation_selection_and_undo(cx: &mut TestAppContext) {
    let (field, cx) = cx.add_window_view(|window, cx| TextInput::new("one two", window, cx));
    cx.update(|window, cx| {
        let focus = field.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        field.update(cx, |field, cx| {
            field.set_selected_text_range(7..7, window, cx)
        });
    });
    redraw(cx);
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "alt-left"
    } else {
        "ctrl-left"
    });
    cx.update(|_, cx| assert_eq!(field.read(cx).model().selection_utf16(), 4..4));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    cx.simulate_input("X");
    cx.update(|_, cx| assert_eq!(field.read(cx).model().text(), "X"));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    });
    cx.update(|_, cx| assert_eq!(field.read(cx).model().text(), "one two"));
}
