/// GPUI backend for Crepuscularity.
///
/// Re-exports the upstream GPUI API plus Crepuscularity's GPUI-oriented `view!` macro and build
/// helpers so existing GPUI consumers can migrate incrementally.
pub use crepuscularity_core::build;
pub use crepuscularity_macros::{view, view_file};
pub use crepuscularity_runtime;
pub use gpui::*;
pub use gpui_platform;
pub use pollster::block_on;

/// Create a new GPUI [`Application`] for the current platform.
///
/// gpui-ce's platform split removed `Application::new()`; the equivalent is
/// `gpui_platform::application()`. Re-exported here so consumers call
/// `crepuscularity_gpui::application()`.
pub fn application() -> gpui::Application {
    gpui_platform::application()
}

pub mod animation;

/// Corner of an element/panel. gpui-ce's core no longer ships a `Corner` enum (it was a
/// macOS traffic-light/CSD helper in zed 0.2.x), so crepuscularity defines its own for the
/// `Anchor` ↔ titlebar-corner mapping below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl From<Anchor> for Corner {
    fn from(anchor: Anchor) -> Self {
        match anchor {
            Anchor::TopLeft | Anchor::TopCenter | Anchor::LeftCenter => Corner::TopLeft,
            Anchor::TopRight | Anchor::RightCenter => Corner::TopRight,
            Anchor::BottomLeft => Corner::BottomLeft,
            Anchor::BottomCenter | Anchor::BottomRight => Corner::BottomRight,
        }
    }
}

pub const GPUI_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

/// Embed a view with paint recycling when its parent re-renders.
///
/// Mirrors Zed's `AnyView::cached` pattern: previous layout/paint is reused unless
/// the child entity called `cx.notify()` (or the window is force-refreshed).
/// Style must fill the parent slot — default style lays out as empty block.
#[deprecated(
    note = "gpui-ce no longer exposes a public `.cached()` API on AnyView; this is now an identity conversion and should be removed from callers"
)]
pub fn cached_view(view: impl Into<AnyView>) -> AnyView {
    view.into()
}

pub fn gpui_window_options(
    app_id: impl Into<String>,
    title: impl Into<SharedString>,
    window_bounds: Option<WindowBounds>,
    window_min_size: Option<Size<Pixels>>,
) -> WindowOptions {
    WindowOptions {
        app_id: Some(app_id.into()),
        titlebar: Some(TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        window_bounds,
        window_min_size,
        ..Default::default()
    }
}

pub mod prelude {
    pub use crate::animation;
    pub use crate::application;
    pub use crate::gpui_window_options;
    pub use crepuscularity_macros::{view, view_file};
    pub use crepuscularity_runtime;
    pub use gpui::prelude::*;
    pub use gpui::{
        black, div, px, relative, rems, rgb, white, Animation, AnimationExt, App, AppContext,
        Application, Context, Entity, FontWeight, IntoElement, Render, SharedString, Window,
        WindowOptions,
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    LeftCenter,
    RightCenter,
}

pub const MAX_BUTTONS_PER_SIDE: usize = 3;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowButtonLayout {
    pub left: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
    pub right: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
}

impl WindowButtonLayout {
    pub fn linux_default() -> Self {
        Self {
            left: [None; MAX_BUTTONS_PER_SIDE],
            right: [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close),
            ],
        }
    }

    pub fn parse(layout_string: &str) -> Result<Self> {
        let layout_string = layout_string.trim();
        if layout_string.is_empty() {
            return Ok(Self::linux_default());
        }

        let (left, right) = layout_string
            .split_once(':')
            .map_or(("", layout_string), |(left, right)| (left, right));

        Ok(Self {
            left: parse_window_button_side(left)?,
            right: parse_window_button_side(right)?,
        })
    }
}

fn parse_window_button_side(side: &str) -> Result<[Option<WindowButton>; MAX_BUTTONS_PER_SIDE]> {
    let mut buttons = [None; MAX_BUTTONS_PER_SIDE];
    let mut index = 0;

    for raw_button in side.split(',') {
        let button = raw_button.trim();
        if button.is_empty() || button == "appmenu" {
            continue;
        }
        if index >= MAX_BUTTONS_PER_SIDE {
            return Err(gpui::private::anyhow::anyhow!(
                "too many window buttons in layout side: {side}"
            ));
        }
        buttons[index] = Some(match button {
            "minimize" => WindowButton::Minimize,
            "maximize" => WindowButton::Maximize,
            "close" => WindowButton::Close,
            other => {
                return Err(gpui::private::anyhow::anyhow!(
                    "unknown window button: {other}"
                ))
            }
        });
        index += 1;
    }

    Ok(buttons)
}

#[cfg(test)]
mod tests {
    use crate::{WindowButton, WindowButtonLayout};

    #[test]
    fn parses_linux_window_button_layout() {
        let layout = WindowButtonLayout::parse(":minimize,maximize,close").unwrap();

        assert_eq!(layout, WindowButtonLayout::linux_default());
    }

    #[test]
    fn parses_left_and_right_window_button_layout() {
        let layout = WindowButtonLayout::parse("close:minimize,maximize").unwrap();

        assert_eq!(
            layout,
            WindowButtonLayout {
                left: [Some(WindowButton::Close), None, None],
                right: [
                    Some(WindowButton::Minimize),
                    Some(WindowButton::Maximize),
                    None,
                ],
            }
        );
    }

    #[test]
    fn rejects_unknown_window_button_layout_tokens() {
        let error = WindowButtonLayout::parse(":minimize,fullscreen").unwrap_err();

        assert!(error.to_string().contains("unknown window button"));
    }
}
