use gpui::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
};
use gpui::prelude::*;
use gpui::{Subscription, TestSupportExt};

pub struct Counter {
    input: Entity<InputState>,
    name: SharedString,
    count: usize,
    // Dropping the subscription would disconnect native input changes.
    _input_subscription: Subscription,
}

impl Counter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Your name"));
        let subscription = cx.subscribe_in(&input, window, |this, input, event, _, cx| {
            if let InputEvent::Change = event {
                this.name = input.read(cx).value();
                cx.notify();
            }
        });
        Self {
            input,
            name: SharedString::default(),
            count: 0,
            _input_subscription: subscription,
        }
    }
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = Input::new(&self.input).id("name").w(px(240.));
        let increment = Button::new("increment")
            .label("Increment")
            .on_click(cx.listener(|this, _, _, cx| {
                this.count += 1;
                cx.notify();
            }));
        let reset = Button::new("reset")
            .label("Reset")
            .on_click(cx.listener(|this, _, _, cx| {
                this.count = 0;
                cx.notify();
            }));
        let status = format!("{}: {}", self.name, self.count);
        let status = div()
            .id("status")
            .test_support()
            .aria_label(status.clone())
            .child(status);

        view_file!("examples/support/counter.crepus")
    }
}
