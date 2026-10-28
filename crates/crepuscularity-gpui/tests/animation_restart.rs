#![cfg(target_os = "macos")]

use crepuscularity_gpui::{self as gpui, animation, gpui_platform};
use gpui::{div, prelude::*, px, rgb, AppContext};
use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

#[derive(Clone, Copy)]
enum Property {
    Width,
    Height,
    Opacity,
}

struct TransitionView {
    property: Property,
    changed: Rc<Cell<bool>>,
}

impl gpui::Render for TransitionView {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let changed = self.changed.get();
        let duration = Duration::from_millis(1);
        let child = div().bg(rgb(0));
        let child = match self.property {
            Property::Width => {
                let (from, to) = if changed { (20., 80.) } else { (10., 20.) };
                animation::width(child.h(px(100.)), "transition", duration, from, to)
                    .into_any_element()
            }
            Property::Height => {
                let (from, to) = if changed { (20., 80.) } else { (10., 20.) };
                animation::height(child.w(px(100.)), "transition", duration, from, to)
                    .into_any_element()
            }
            Property::Opacity => {
                let (from, to) = if changed { (0.2, 1.) } else { (0., 0.2) };
                animation::opacity(child.size_full(), "transition", duration, from, to)
                    .into_any_element()
            }
        };
        div().size_full().bg(rgb(0xffffff)).child(child)
    }
}

#[test]
#[ignore = "requires a Metal device"]
fn completed_transitions_restart_when_endpoints_change() {
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    for property in [Property::Width, Property::Height, Property::Opacity] {
        let changed = Rc::new(Cell::new(false));
        let flag = changed.clone();
        let window = cx
            .open_window(gpui::size(px(100.), px(100.)), move |_, app| {
                app.new(|_| TransitionView {
                    property,
                    changed: flag,
                })
            })
            .unwrap()
            .into();
        let draw = |cx: &mut gpui::HeadlessAppContext| {
            cx.update_window(window, |_, window, app| window.draw(app).clear(app))
                .unwrap();
        };
        let center = |cx: &mut gpui::HeadlessAppContext| {
            let image = cx.capture_screenshot(window).unwrap();
            image.get_pixel(image.width() / 2, image.height() / 2).0[0]
        };
        draw(&mut cx);
        std::thread::sleep(Duration::from_millis(15));
        draw(&mut cx);
        let completed = center(&mut cx);
        assert!(completed > 190); // Small dimensions or 20% black over white.
        draw(&mut cx);
        assert_eq!(
            center(&mut cx),
            completed,
            "unchanged arguments retain completion"
        );
        changed.set(true);
        draw(&mut cx);
        assert!(
            center(&mut cx).abs_diff(completed) < 5,
            "edited endpoints restart from the supplied start value"
        );
        std::thread::sleep(Duration::from_millis(15));
        draw(&mut cx);
        assert!(center(&mut cx) < 5, "new transition reaches its endpoint");
    }
}
