extern crate crepuscularity_gpui_kit as gpui;

#[path = "../examples/support/counter.rs"]
mod counter;

use gpui::kit::test::TestWindowExt;
use gpui::prelude::*;
use gpui::{point, size, Bounds, TestAppContext, WindowBounds};

#[gpui::kit::test]
fn native_controls_update_a_compiled_template(cx: &mut TestAppContext) {
    let (window, _content) = cx.update(|cx| {
        gpui::init(cx);
        gpui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(640.), px(480.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| counter::Counter::new(window, cx)),
        )
        .expect("open test window")
    });
    // Use Kit's production Root wrapper; opening a bare GPUI view is not enough.
    assert!(window.downcast::<gpui::base::Root>().is_some());

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("status").label(), Some(": 0"));
        assert!(window.try_find("reset").is_none());

        window.click("increment", cx);
        assert_eq!(window.find("status").label(), Some(": 1"));
        assert!(window.find("reset").visible());

        window.click("name", cx);
        assert_eq!(window.find("name").focused(), Some(true));
        window.input("Ada 🦀", cx);
        assert_eq!(window.find("name").value(), Some("Ada 🦀"));
        assert_eq!(window.find("status").label(), Some("Ada 🦀: 1"));

        window.click("reset", cx);
        assert_eq!(window.find("status").label(), Some("Ada 🦀: 0"));
        assert!(window.try_find("reset").is_none());
    })
    .expect("drive counter controls");
}
