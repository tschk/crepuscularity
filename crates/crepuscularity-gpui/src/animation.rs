use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use gpui::{ease_in_out, px, Animation, AnimationExt, Div, ElementId, IntoElement, Styled};

// Keep the caller's typed identity intact while restarting state on transition edits.
fn transition_id(
    id: impl Into<ElementId>,
    property: &str,
    duration: Duration,
    from: f32,
    to: f32,
) -> ElementId {
    let mut config = DefaultHasher::new();
    (property, duration, from.to_bits(), to.to_bits()).hash(&mut config);
    ElementId::NamedChild(
        Arc::new(id.into()),
        format!("crepus-transition-{:016x}", config.finish()).into(),
    )
}

/// Animate the width of an element between `from` and `to` pixels.
///
/// Unchanged `id`, duration and endpoints retain progress. Changing the duration
/// or either endpoint starts a new animation from the given start width. This is intended for state-driven transitions
/// such as collapsing/expanding a sidebar.
pub fn width(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        transition_id(id, "width", duration, from, to),
        Animation::new(duration).with_easing(ease_in_out),
        move |el, delta| {
            let width = from + (to - from) * delta;
            el.w(px(width))
        },
    )
}

/// Animate the height of an element between `from` and `to` pixels.
/// Unchanged arguments retain progress; duration or endpoint edits restart from `from`.
pub fn height(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        transition_id(id, "height", duration, from, to),
        Animation::new(duration).with_easing(ease_in_out),
        move |el, delta| {
            let height = from + (to - from) * delta;
            el.h(px(height))
        },
    )
}

/// Animate the opacity of an element between `from` and `to`.
/// Unchanged arguments retain progress; duration or endpoint edits restart from `from`.
pub fn opacity(
    el: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    duration: Duration,
    from: f32,
    to: f32,
) -> impl IntoElement {
    el.with_animation(
        transition_id(id, "opacity", duration, from, to),
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
    fn transition_identity_retains_typed_parent_and_tracks_arguments() {
        let id = |property, duration, from, to| {
            transition_id(ElementId::Integer(7), property, duration, from, to)
        };
        let duration = Duration::from_millis(100);
        let original = id("width", duration, 0., 1.);
        assert_eq!(original, id("width", duration, 0., 1.));
        for changed in [
            id("height", duration, 0., 1.),
            id("width", duration * 2, 0., 1.),
            id("width", duration, 0.5, 1.),
            id("width", duration, 0., 2.),
        ] {
            assert_ne!(original, changed);
        }
        let ElementId::NamedChild(parent, _) = original else {
            panic!("expected namespaced state")
        };
        assert_eq!(*parent, ElementId::Integer(7));
        assert_ne!(
            transition_id(ElementId::Integer(7), "width", duration, 0., 1.),
            transition_id(ElementId::Name("7".into()), "width", duration, 0., 1.)
        );
    }

    #[test]
    fn test_div_factory() {
        // Ensure the factory function can be called and returns a Div
        let _ = div();
    }
}
