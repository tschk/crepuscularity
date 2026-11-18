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
    let keys = Rc::new(RefCell::new(Vec::new()));
    let observed_keys = keys.clone();
    let bindings = RuntimeBindings::new()
        .on("keys", move |event, _, _| {
            if let crepuscularity_runtime::RuntimeEventKind::KeyDown(key) = event.kind {
                observed_keys.borrow_mut().push(key.keystroke.key.clone());
            }
        })
        .on("edited", move |event, _, _| {
            if let crepuscularity_runtime::RuntimeEventKind::Input(input) = event.kind {
                output.borrow_mut().push(input.text.clone());
            }
        });
    let (_, cx) = cx.add_window_view(|_, _| TemplateFixture {
        nodes: parse_template("div flex flex-col\n    input #readonly readonly @input=edited @keydown=keys\n    input #disabled disabled @input=edited\n    input #editable @input=edited @keydown=keys").unwrap(),
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
    cx.simulate_keystrokes("tab");
    cx.update(|window, _| assert!(!readonly.is_focused(window)));
    redraw(cx);
    cx.simulate_input("ok");
    assert_eq!(drafts.borrow().last().map(String::as_str), Some("ok"));
    cx.simulate_keystrokes("shift-tab");
    cx.update(|window, _| assert!(readonly.is_focused(window)));
    redraw(cx);
    keys.borrow_mut().clear();
    cx.simulate_keystrokes("ctrl-tab");
    cx.update(|window, _| assert!(readonly.is_focused(window)));
    assert_eq!(keys.borrow().last().map(String::as_str), Some("tab"));
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

#[gpui::test]
fn keyed_loop_factory_identity_survives_reordering(cx: &mut TestAppContext) {
    use crepuscularity_core::TemplateValue;
    let records = Rc::new(RefCell::new(Vec::new()));
    let output = records.clone();
    let bindings = RuntimeBindings::new().element("Probe", move |request, _, _| {
        output
            .borrow_mut()
            .push((request.context.get_str("name"), request.id.to_string()));
        Ok(gpui::div()
            .w(gpui::px(20.))
            .h(gpui::px(20.))
            .into_any_element())
    });
    let mut first = TemplateContext::new();
    first.set("name", "first");
    let mut second = TemplateContext::new();
    second.set("name", "second");
    let initial = vec![first.clone(), second.clone()];
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut context = TemplateContext::new();
        context.set("rows", TemplateValue::List(initial));
        TemplateFixture {
            context,
            bindings,
            nodes: parse_template("for row in rows\n  Probe key={name}").unwrap(),
        }
    });
    redraw(cx);
    let before: std::collections::HashMap<_, _> = records.borrow().iter().cloned().collect();
    assert_eq!(before.len(), 2);
    assert_ne!(before["first"], before["second"]);
    records.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.context
            .set("rows", TemplateValue::List(vec![second, first]));
        cx.notify();
    });
    redraw(cx);
    let after: std::collections::HashMap<_, _> = records.borrow().iter().cloned().collect();
    assert_eq!(before, after);
}

#[gpui::test]
fn keyed_inputs_keep_drafts_and_callback_context_after_reordering(cx: &mut TestAppContext) {
    use crepuscularity_core::TemplateValue;
    let records = Rc::new(RefCell::new(Vec::new()));
    let output = records.clone();
    let bindings = RuntimeBindings::new().on("edited", move |event, _, _| {
        if let crepuscularity_runtime::RuntimeEventKind::Input(input) = event.kind {
            output
                .borrow_mut()
                .push((event.context.get_str("name"), input.text.clone()));
        }
    });
    let mut first = TemplateContext::new();
    first.set("name", "first");
    let mut second = TemplateContext::new();
    second.set("name", "second");
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut context = TemplateContext::new();
        context.set("rows", TemplateValue::List(vec![first.clone(), second.clone()]));
        TemplateFixture {
            context,
            bindings,
            nodes: parse_template(
                "div flex flex-col\n  for row in rows\n    input #field key={name} @input=edited w-[200px] h-[30px]",
            )
            .unwrap(),
        }
    });
    redraw(cx);
    cx.simulate_click(
        gpui::point(gpui::px(190.), gpui::px(5.)),
        Default::default(),
    );
    cx.simulate_input("alpha");
    assert_eq!(
        records.borrow().last(),
        Some(&("first".into(), "alpha".into()))
    );
    redraw(cx);
    cx.simulate_click(
        gpui::point(gpui::px(190.), gpui::px(35.)),
        Default::default(),
    );
    cx.simulate_input("beta");
    assert_eq!(
        records.borrow().last(),
        Some(&("second".into(), "beta".into()))
    );
    view.update(cx, |view, cx| {
        view.context
            .set("rows", TemplateValue::List(vec![second, first]));
        cx.notify();
    });
    redraw(cx);
    cx.simulate_click(
        gpui::point(gpui::px(190.), gpui::px(5.)),
        Default::default(),
    );
    cx.simulate_input("X");
    assert_eq!(
        records.borrow().last(),
        Some(&("second".into(), "betaX".into()))
    );
}

#[gpui::test]
fn component_click_registry_and_arguments_survive_hot_reload(cx: &mut TestAppContext) {
    use crepuscularity_core::{context::value_to_str, TemplateValue};
    use crepuscularity_runtime::{EventHandlers, HotReloadState, HotReloadView};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("view.crepus");
    let component = dir.path().join("counter_button.crepus");
    let entry = |instances: &str| {
        format!(
            "div flex flex-col\n{instances}  button w-[100px] h-[40px] @click=reset\n    \"Reset\""
        )
    };
    std::fs::write(
        &component,
        "button w-[100px] h-[40px] @click=action\n  \"{label}\"",
    )
    .unwrap();
    std::fs::write(
        &path,
        entry("  include counter_button.crepus label=\"Add\" action=\"increment\"\n"),
    )
    .unwrap();
    let calls = Rc::new(RefCell::new(Vec::<String>::new()));
    let record = |name: &'static str| {
        let calls = calls.clone();
        move |args: &[TemplateValue], _: &mut Window, _: &mut App| {
            let args: Vec<_> = args.iter().map(value_to_str).collect();
            calls
                .borrow_mut()
                .push(format!("{name}({})", args.join(",")));
        }
    };
    let handlers = EventHandlers::new()
        .on_click("increment", record("increment"))
        .on_click("add", record("add"))
        .on_click("save-item", record("save-item"))
        .on_click("reset", record("reset"));
    let state = cx.new(|cx| {
        HotReloadState::new(path.clone(), TemplateContext::new(), cx).with_event_handlers(handlers)
    });
    let (_, cx) = cx.add_window_view(|_, _| HotReloadView::new(state.clone()));
    cx.run_until_parked();
    redraw(cx);
    let click = |cx: &mut gpui::VisualTestContext, y| {
        cx.simulate_click(gpui::point(gpui::px(10.), gpui::px(y)), Default::default());
    };
    click(cx, 10.);
    click(cx, 50.);
    assert_eq!(*calls.borrow(), ["increment()", "reset()"]);
    std::fs::write(
        &component,
        "button w-[100px] h-[40px] bg-red-500 @click={action}\n  \"+ {label}\"",
    )
    .unwrap();
    std::fs::write(
        &path,
        entry(concat!(
            "  include counter_button.crepus label=\"Add\" action=\"increment\"\n",
            "  include counter_button.crepus label=\"Add 5\" action=\"add(5, label)\"\n",
            "  button w-[100px] h-[40px] @click={save-item('a\\',b', 2)}\n    \"Save\"\n",
        )),
    )
    .unwrap();
    state.update(cx, |state, _| state.reload());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    redraw(cx);
    calls.borrow_mut().clear();
    for y in [10., 50., 90., 130.] {
        click(cx, y);
    }
    assert_eq!(
        *calls.borrow(),
        [
            "increment()",
            "add(5,Add 5)",
            "save-item(a',b,2)",
            "reset()"
        ]
    );
}

#[gpui::test]
fn component_input_retains_draft_and_both_callback_registries_across_reload(
    cx: &mut TestAppContext,
) {
    use crepuscularity_runtime::{EventHandlers, HotReloadState, HotReloadView};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("view.crepus");
    let component = dir.path().join("field.crepus");
    std::fs::write(
        &component,
        "input #field value=\"\" @input=edited @click=action w-[200px] h-[30px]",
    )
    .unwrap();
    std::fs::write(
        &path,
        "include field.crepus label=\"before\" action=\"selected(label)\"",
    )
    .unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let click_output = clicks.clone();
    let handlers = EventHandlers::new().on_click("selected", move |args, _, _| {
        click_output
            .borrow_mut()
            .push(crepuscularity_core::context::value_to_str(&args[0]));
    });
    let drafts = Rc::new(RefCell::new(Vec::new()));
    let draft_output = drafts.clone();
    let bindings = RuntimeBindings::new().on("edited", move |event, _, _| {
        if let crepuscularity_runtime::RuntimeEventKind::Input(input) = event.kind {
            draft_output
                .borrow_mut()
                .push((event.context.get_str("label"), input.text.clone()));
        }
    });
    let state = cx.new(|cx| {
        HotReloadState::new(path.clone(), TemplateContext::new(), cx)
            .with_bindings(bindings)
            .with_event_handlers(handlers)
    });
    let (_, cx) = cx.add_window_view(|_, _| HotReloadView::new(state.clone()));
    cx.run_until_parked();
    redraw(cx);
    cx.simulate_click(
        gpui::point(gpui::px(190.), gpui::px(5.)),
        Default::default(),
    );
    cx.simulate_input("draft");
    assert_eq!(
        drafts.borrow().last(),
        Some(&("before".into(), "draft".into()))
    );
    assert_eq!(&*clicks.borrow(), &["before"]);
    std::fs::write(
        &path,
        "include field.crepus label=\"after\" action=\"selected(label)\"",
    )
    .unwrap();
    std::fs::write(&component, "input #field value=\"\" placeholder=\"Updated\" @input=edited @click=action w-[200px] h-[30px] hover:bg-red-500 focus:bg-blue-500").unwrap();
    state.update(cx, |state, _| state.reload());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    redraw(cx);
    cx.simulate_click(
        gpui::point(gpui::px(190.), gpui::px(5.)),
        Default::default(),
    );
    cx.simulate_input("X");
    assert_eq!(
        drafts.borrow().last(),
        Some(&("after".into(), "draftX".into()))
    );
    assert_eq!(&*clicks.borrow(), &["before", "after"]);
}
