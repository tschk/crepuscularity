//! Built-in runtime element bindings. Custom/typed behavior belongs to a host factory.
use crepuscularity_core::{ast::Element, TemplateContext, TemplateValue};
use gpui::{
    accesskit, Div, InteractiveElement, SharedString, Stateful, StatefulInteractiveElement, Styled,
};

use crate::bindings::{RuntimeBindings, RuntimeEvent, RuntimeEventKind};

fn value(expr: &str, ctx: &TemplateContext) -> Result<TemplateValue, String> {
    crepuscularity_core::eval::eval_expr(expr, ctx).map_err(|e| e.to_string())
}

fn boolean(expr: &str, ctx: &TemplateContext) -> Result<bool, String> {
    match value(expr, ctx)? {
        TemplateValue::Bool(value) => Ok(value),
        _ => Err(format!(
            "GPUI boolean binding `{expr}` must evaluate to a bool"
        )),
    }
}

pub(crate) fn text(expr: &str, ctx: &TemplateContext) -> Result<SharedString, String> {
    Ok(crepuscularity_core::context::value_to_str(&value(expr, ctx)?).into())
}

pub(crate) fn presence_boolean(expr: &str, ctx: &TemplateContext) -> Result<bool, String> {
    if matches!(value(expr, ctx)?, TemplateValue::Str(s) if s.is_empty()) {
        Ok(true)
    } else {
        boolean(expr, ctx)
    }
}

fn disabled(el: &Element, ctx: &TemplateContext) -> Result<bool, String> {
    el.bindings
        .iter()
        .find(|b| b.prop == "disabled")
        .map(|b| presence_boolean(&b.value, ctx))
        .unwrap_or(Ok(false))
}

pub(crate) fn apply_bindings(
    mut d: Stateful<Div>,
    el: &Element,
    ctx: &TemplateContext,
) -> Result<Stateful<Div>, String> {
    for binding in &el.bindings {
        let expr = &binding.value;
        d = match binding.prop.as_str() {
            "id" | "key" => d, // Consumed by identity scoping before construction.
            "src" if matches!(el.tag.as_str(), "img" | "image" | "svg") => d,
            "path" if matches!(el.tag.as_str(), "img" | "image") => d,
            "disabled" => if presence_boolean(expr, ctx)? { d.cursor_not_allowed().opacity(0.5) } else { d },
            "focusable" => if boolean(expr, ctx)? && !disabled(el, ctx)? { d.focusable() } else { d },
            "role" => d.role(role(text(expr, ctx)?.as_ref())?),
            "aria-label" | "alt" => d.aria_label(text(expr, ctx)?),
            "aria-description" => d.aria_description(text(expr, ctx)?),
            "aria-keyshortcuts" => d.aria_keyshortcuts(text(expr, ctx)?),
            "aria-value" => d.aria_value(text(expr, ctx)?),
            "aria-placeholder" => d.aria_placeholder(text(expr, ctx)?),
            "aria-selected" => d.aria_selected(boolean(expr, ctx)?),
            "aria-expanded" => d.aria_expanded(boolean(expr, ctx)?),
            "checked" => d.aria_toggled(if presence_boolean(expr, ctx)? { accesskit::Toggled::True } else { accesskit::Toggled::False }),
            prop => return Err(format!("Unsupported GPUI binding `:{prop}` on {}; register a native element for typed behavior", el.tag)),
        };
    }
    Ok(d)
}

fn role(name: &str) -> Result<accesskit::Role, String> {
    Ok(match name {
        "button" => accesskit::Role::Button,
        "checkbox" => accesskit::Role::CheckBox,
        "radio" => accesskit::Role::RadioButton,
        "switch" => accesskit::Role::Switch,
        "img" | "image" => accesskit::Role::Image,
        "link" => accesskit::Role::Link,
        "heading" => accesskit::Role::Heading,
        "list" => accesskit::Role::List,
        "listitem" => accesskit::Role::ListItem,
        "dialog" => accesskit::Role::Dialog,
        "group" => accesskit::Role::Group,
        "slider" => accesskit::Role::Slider,
        "tab" => accesskit::Role::Tab,
        "tablist" => accesskit::Role::TabList,
        "tabpanel" => accesskit::Role::TabPanel,
        "menu" => accesskit::Role::Menu,
        "menuitem" => accesskit::Role::MenuItem,
        _ => {
            return Err(format!(
                "Unsupported GPUI role `{name}`; use a native element for other AccessKit roles"
            ))
        }
    })
}

#[derive(Debug, PartialEq)]
struct KeyFilter {
    key: String,
    control: bool,
    command: bool,
    alt: bool,
    shift: bool,
}

impl KeyFilter {
    fn parse(chord: &str) -> Result<Self, String> {
        let lower = chord.to_lowercase();
        let mut parts: Vec<_> = lower.split('+').collect();
        let key = parts.pop().unwrap_or("");
        if key.is_empty() {
            return Err(format!("Empty key in GPUI chord `{chord}`"));
        }
        let key = match key {
            "esc" => "escape",
            "arrowup" => "up",
            "arrowdown" => "down",
            "arrowleft" => "left",
            "arrowright" => "right",
            other => other,
        }
        .to_owned();
        let mut result = Self {
            key,
            control: false,
            command: false,
            alt: false,
            shift: false,
        };
        for part in parts {
            match part {
                "ctrl" | "control" => result.control = true,
                "cmd" | "command" | "meta" => result.command = true,
                "alt" | "opt" => result.alt = true,
                "shift" => result.shift = true,
                _ => return Err(format!("Unknown modifier `{part}` in GPUI chord `{chord}`")),
            }
        }
        Ok(result)
    }

    fn matches(&self, key: &gpui::Keystroke) -> bool {
        self.key == key.key
            && self.control == key.modifiers.control
            && self.command == key.modifiers.platform
            && self.alt == key.modifiers.alt
            && self.shift == key.modifiers.shift
            && !key.modifiers.function
    }
}

pub(crate) fn apply_events(
    mut d: Stateful<Div>,
    el: &Element,
    ctx: &TemplateContext,
    id: &str,
    bindings: &RuntimeBindings,
) -> Result<Stateful<Div>, String> {
    let is_disabled = disabled(el, ctx)?;
    let mut seen = std::collections::HashSet::new();
    for handler in &el.event_handlers {
        if !seen.insert(handler.event.as_str()) {
            return Err(format!("Duplicate GPUI event `@{}`", handler.event));
        }
        let callback = bindings.handler(&handler.handler)?;
        let filters = if matches!(handler.event.as_str(), "keydown" | "keyup") {
            handler
                .modifiers
                .iter()
                .map(|m| KeyFilter::parse(m))
                .collect::<Result<Vec<_>, _>>()?
        } else if !handler.modifiers.is_empty() {
            return Err(format!(
                "GPUI event modifiers are only supported for keydown/keyup, not {}",
                handler.event
            ));
        } else {
            Vec::new()
        };
        if !matches!(
            handler.event.as_str(),
            "click"
                | "mousedown"
                | "mouseup"
                | "mousemove"
                | "mouseexit"
                | "scroll"
                | "pinch"
                | "hover"
                | "modifierschanged"
                | "drop"
                | "keydown"
                | "keyup"
        ) {
            return Err(format!("Unsupported GPUI event `@{}`; register a native element for typed actions, drags or input", handler.event));
        }
        if is_disabled {
            continue;
        }
        let context = ctx.clone();
        let id = id.to_owned();
        macro_rules! dispatch {
            ($kind:expr, $window:expr, $cx:expr) => {
                if !is_disabled {
                    callback(
                        RuntimeEvent {
                            element_id: &id,
                            context: &context,
                            kind: $kind,
                        },
                        $window,
                        $cx,
                    )
                }
            };
        }
        d = match handler.event.as_str() {
            "click" => d.on_click(move |e, w, cx| dispatch!(RuntimeEventKind::Click(e), w, cx)),
            "mousedown" => d.on_mouse_down(gpui::MouseButton::Left, move |e, w, cx| dispatch!(RuntimeEventKind::MouseDown(e), w, cx)),
            "mouseup" => d.on_mouse_up(gpui::MouseButton::Left, move |e, w, cx| dispatch!(RuntimeEventKind::MouseUp(e), w, cx)),
            "mousemove" => d.on_mouse_move(move |e, w, cx| dispatch!(RuntimeEventKind::MouseMove(e), w, cx)),
            "mouseexit" => d.on_mouse_exit(move |e, w, cx| dispatch!(RuntimeEventKind::MouseExit(e), w, cx)),
            "scroll" => d.on_scroll_wheel(move |e, w, cx| dispatch!(RuntimeEventKind::Scroll(e), w, cx)),
            "pinch" => d.on_pinch(move |e, w, cx| dispatch!(RuntimeEventKind::Pinch(e), w, cx)),
            "hover" => d.on_hover(move |e, w, cx| dispatch!(RuntimeEventKind::Hover(*e), w, cx)),
            "modifierschanged" => d.on_modifiers_changed(move |e, w, cx| dispatch!(RuntimeEventKind::ModifiersChanged(e), w, cx)),
            "drop" => d.on_drop::<gpui::ExternalPaths>(move |e, w, cx| dispatch!(RuntimeEventKind::ExternalDrop(e), w, cx)),
            "keydown" => d.on_key_down(move |e, w, cx| {
                if filters.is_empty() || filters.iter().any(|f| f.matches(&e.keystroke)) {
                    dispatch!(RuntimeEventKind::KeyDown(e), w, cx);
                }
            }),
            "keyup" => d.on_key_up(move |e, w, cx| {
                if filters.is_empty() || filters.iter().any(|f| f.matches(&e.keystroke)) {
                    dispatch!(RuntimeEventKind::KeyUp(e), w, cx);
                }
            }),
            event => return Err(format!("Unsupported GPUI event `@{event}`; register a native element for typed actions, drags or input")),
        };
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_filters_distinguish_control_command_and_reject_typos() {
        assert_ne!(
            KeyFilter::parse("ctrl+s").unwrap(),
            KeyFilter::parse("cmd+s").unwrap()
        );
        assert!(KeyFilter::parse("contrl+s").is_err());
        assert!(KeyFilter::parse("ctrl+").is_err());
        assert_eq!(KeyFilter::parse("shift+ArrowUp").unwrap().key, "up");
    }

    #[test]
    fn binding_values_are_typed_and_invalid_roles_are_reported() {
        let ctx = TemplateContext::default();
        assert!(boolean("true", &ctx).unwrap());
        assert!(boolean("\"false\"", &ctx).is_err());
        assert!(role("buton").is_err());
    }
}
