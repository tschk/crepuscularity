// The same alias is needed at an application's crate root unless Cargo already
// names its crepuscularity-gpui-kit dependency `gpui`.
extern crate crepuscularity_gpui_kit as gpui;

#[path = "support/counter.rs"]
mod counter;

use gpui::prelude::*;

fn main() {
    gpui::application()
        .with_assets(gpui::assets::Assets)
        .run(|cx| {
            gpui::init(cx);
            gpui::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| counter::Counter::new(window, cx))
            })
            .expect("open counter window");
        });
}
