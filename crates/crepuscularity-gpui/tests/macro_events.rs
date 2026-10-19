use crepuscularity_gpui::view;
use gpui::{prelude::*, Context, KeyDownEvent, SharedString, Window};

#[test]
fn generated_lengths_reject_negative_sizes_and_preserve_signed_offsets() {
    let mut actual = view! {r#"div w-[20px] w-[-2px] h-[30px] h-[-25%] p-2 p-[-1rem] p-auto gap-2 gap-[-3px] gap-auto text-[14px] text-[-1px] text-[50%] m-[-4px] top-[-3px]"#};
    let mut expected = gpui::div()
        .w(gpui::px(20.0))
        .h(gpui::px(30.0))
        .p_2()
        .gap_2()
        .text_size(gpui::px(14.0))
        .m(gpui::px(-4.0))
        .top(gpui::px(-3.0));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn static_css_hex_colors_expand_and_preserve_alpha_in_generated_views() {
    let mut actual = view! {r#"div bg-[#123] bg-[#12] text-[#1234] text-[#xyz] border-[#aabbcc80] border-[#12345]"#};
    let mut expected = gpui::div()
        .bg(gpui::rgb(0x112233))
        .text_color(gpui::rgba(0x11223344))
        .border_color(gpui::rgba(0xaabbcc80));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn static_grid_utilities_construct_the_requested_gpui_style() {
    let mut actual = view! {r#"div grid grid-cols-2 grid-cols-0 grid-rows-3 grid-rows-0 col-span-2 col-span-0 row-span-2 row-span-0 col-start--1 col-start-0 row-start-2 row-start-0 col-end-3 col-end-0 row-end-4 row-end-0 items-stretch justify-evenly"#};
    let mut expected = gpui::div()
        .grid()
        .grid_cols(2)
        .grid_rows(3)
        .col_span(2)
        .row_span(2)
        .col_start(-1)
        .row_start(2)
        .col_end(3)
        .row_end(4)
        .items_stretch()
        .justify_evenly();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn bound_ids_and_event_closures_compile_against_gpui() {
    let button_id = SharedString::from("bound-button");
    let _ = view! {r#"<button id={button_id} @click={|_, _, _| {}}>Press</button>"#};
    let _ = view! {r#"<div @hover={|_, _, _| {}}>Hover</div>"#};
    let _ = view! {r#"button @click={|_, _, _| {}}"#};
    let _ = KeyboardView::render;
}

struct KeyboardView;

impl KeyboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"<div @keydown|ctrl+s={handle_key} @keyup|cmd+Esc={|_, _, _| {}}>Key</div>"#}
    }

    fn handle_key(&mut self, _event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
    }
}

#[test]
fn dynamic_class_lists_compile_with_ids_and_handlers() {
    let theme = String::from("flex flex-row gap-4 p-2 bg-red-500");
    let _ = view! {r#"<div id="dynamic" class={theme} @click={|_, _, _| {}}>Content</div>"#};
    let theme = SharedString::from("flex flex-col");
    let _ = view! {r#"div class={theme}"#};
}

#[test]
fn multiple_state_utilities_construct_without_panicking() {
    let _ = view! {r#"div hover:bg-red-500 hover:text-white focus:p-2 focus:m-1 active:bg-blue-500 active:text-black"#};
}

#[test]
fn conditional_target_classes_construct() {
    let enabled = true;
    let _ = view! {r#"div class:gpui:flex-row={enabled} class:gui:p-2={enabled} class:web:hidden={enabled}"#};
}

#[test]
fn mixed_static_and_dynamic_state_classes_construct_and_evaluate_once() {
    let mut evaluations = 0;
    let _ = view! {r#"div hover:bg-blue-500 class={{ evaluations += 1; "hover:bg-red-500 hover:text-white focus:p-2 active:bg-black" }} hover:p-2"#};
    assert_eq!(evaluations, 1);
}
