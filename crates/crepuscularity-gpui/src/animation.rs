use std::time::Duration;

use gpui::{ease_in_out, px, Animation, AnimationExt, Div, ElementId, IntoElement, Styled};

/// Animate the width of an element between `from` and `to` pixels.
///
/// The animation is keyed by `id`; changing the `from`/`to` values will start a new
/// animation from the given start width. This is intended for state-driven transitions
/// such as collapsing/expanding a sidebar.
pub fn width(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(duration).with_easing(ease_in_out),
        move |el, delta| {
            let width = from + (to - from) * delta;
            el.w(px(width))
        },
    )
}

/// Animate the height of an element between `from` and `to` pixels.
pub fn height(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(duration).with_easing(ease_in_out),
        move |el, delta| {
            let height = from + (to - from) * delta;
            el.h(px(height))
        },
    )
}

/// Animate the opacity of an element between `from` and `to`.
pub fn opacity(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(duration).with_easing(ease_in_out),
        move |el, delta| {
            let opacity = from + (to - from) * delta;
            el.opacity(opacity)
        },
    )
}

/// Build a `Div` that can be animated; convenience wrapper around `gpui::div`.
pub fn div() -> Div {
    gpui::div()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_factory() {
        // Ensure the factory function can be called and returns a Div
        let _ = div();
    }
}
