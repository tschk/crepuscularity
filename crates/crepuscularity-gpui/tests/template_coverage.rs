//! Downstream macro/type fixtures. They construct elements without opening a window.
//! Native dispatch, image layout and accessibility acceptance need headless/UI tests.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crepuscularity_gpui::view;
use gpui::{prelude::*, App, IntoElement, RenderOnce, Window};

#[derive(gpui::IntoElement)]
struct MyCard;

impl MyCard {
    fn new() -> Self {
        Self
    }

    fn id(self, _id: &str) -> Self {
        self
    }
}

impl RenderOnce for MyCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        gpui::div().child("Card")
    }
}

#[test]
fn custom_loop_roots_do_not_require_an_interactive_builder() {
    let items = [1, 2];
    let _ = view!(
        r#"
div
    for item in {items.iter()}
        MyCard key={item} bind:id={"custom-id"}
"#
    )
    .into_any_element();
}

#[test]
fn native_tuple_ids_remain_typed_inside_loops() {
    let items = [1usize, 2];
    let _ = view!(
        r#"
div
    for item in {items.iter()}
        button bind:id={("row", *item)}
            "Save"
"#
    )
    .into_any_element();
}

#[test]
fn filtered_inline_handlers_can_retain_non_copy_captures() {
    let counter = Arc::new(AtomicUsize::new(0));
    let _ = view!(r#"
div @keydown|ctrl+s={move |event, _window, _cx| { if event.keystroke.key == "s" { counter.fetch_add(1, Ordering::Relaxed); } }}
"#).into_any_element();
}

#[test]
fn images_state_styles_and_native_builders_expand_against_ce() {
    let selected = true;
    let _ = view!(
        r#"
div hover:bg-red-500 hover:text-white class:hover:p-4={selected}
    img src={"logo.png"} alt={"Logo"} w-6 h-6
    svg src={"icon.svg"} w-6 h-6
    button checked disabled bind:native={|el| el.occlude()}
        "Disabled"
"#
    )
    .into_any_element();
}

#[test]
fn original_gpui_namespace_preserves_shadowed_types() {
    // These paths deliberately bypass Crepuscularity's compatibility Anchor.
    let _: Option<crepuscularity_gpui::gpui::Anchor> = None;
    let _: Option<crepuscularity_gpui::gpui::WindowButton> = None;
}
