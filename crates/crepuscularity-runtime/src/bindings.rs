//! Explicit host bindings for runtime GPUI templates.
//!
//! Templates name callbacks and native element factories. They never execute
//! extracted JavaScript or Rust source. Factories retain access to the original
//! GPUI `Window`/`App`, so typed drags, input entities, canvases and platform APIs
//! do not have to pass through a lossy serialized event protocol.

use std::{collections::HashMap, rc::Rc};

use crepuscularity_core::{ast::Element, TemplateContext};
use gpui::{AnyElement, App, IntoElement, RenderOnce, SharedString, Window};

/// Original native event data, borrowed only for the duration of dispatch.
pub enum RuntimeEventKind<'a> {
    Click(&'a gpui::ClickEvent),
    MouseDown(&'a gpui::MouseDownEvent),
    MouseUp(&'a gpui::MouseUpEvent),
    MouseMove(&'a gpui::MouseMoveEvent),
    MouseExit(&'a gpui::MouseExitEvent),
    Scroll(&'a gpui::ScrollWheelEvent),
    Pinch(&'a gpui::PinchEvent),
    KeyDown(&'a gpui::KeyDownEvent),
    KeyUp(&'a gpui::KeyUpEvent),
    ModifiersChanged(&'a gpui::ModifiersChangedEvent),
    Hover(bool),
    ExternalDrop(&'a gpui::ExternalPaths),
    Input(&'a crate::TextInputEvent),
    Change(&'a crate::TextInputEvent),
    Submit(&'a crate::TextInputEvent),
}

pub struct RuntimeEvent<'a> {
    pub element_id: &'a str,
    /// Evaluated context of the element that registered this callback, including
    /// loop variables and include props. Host state remains owned by the host.
    pub context: &'a TemplateContext,
    pub kind: RuntimeEventKind<'a>,
}

pub(crate) type EventCallback = Rc<dyn Fn(RuntimeEvent<'_>, &mut Window, &mut App)>;
type ElementFactory =
    Rc<dyn Fn(ElementRequest, &mut Window, &mut App) -> Result<AnyElement, String>>;

/// A native factory owns its rendered children and may compose or replace them.
/// Apply template styling/semantics explicitly when implementing a custom tag.
pub struct ElementRequest {
    pub id: SharedString,
    pub element: Element,
    pub context: TemplateContext,
    pub children: Vec<AnyElement>,
}

#[derive(Clone, Default)]
pub struct RuntimeBindings {
    handlers: HashMap<String, EventCallback>,
    elements: HashMap<String, ElementFactory>,
}

impl RuntimeBindings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler referenced by `@click=name` (and other event names).
    /// The callback can update retained GPUI entities and control propagation.
    pub fn on(
        mut self,
        name: impl Into<String>,
        callback: impl Fn(RuntimeEvent<'_>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.insert(name.into(), Rc::new(callback));
        self
    }

    /// Register a native tag. Unlike `Type::new()` shorthand, the factory may
    /// construct any GPUI element and use state, services and typed callbacks.
    pub fn element(
        mut self,
        tag: impl Into<String>,
        factory: impl Fn(ElementRequest, &mut Window, &mut App) -> Result<AnyElement, String> + 'static,
    ) -> Self {
        self.elements.insert(tag.into(), Rc::new(factory));
        self
    }

    pub(crate) fn handler(&self, name: &str) -> Result<EventCallback, String> {
        self.handlers
            .get(name)
            .cloned()
            .ok_or_else(|| format!("GPUI template handler `{name}` is not registered"))
    }

    pub(crate) fn handlers_only(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
            elements: HashMap::new(),
        }
    }

    pub(crate) fn has_element(&self, tag: &str) -> bool {
        self.elements.contains_key(tag)
    }

    pub(crate) fn render_element(&self, request: ElementRequest) -> AnyElement {
        let factory = self.elements[&request.element.tag].clone();
        NativeElement { request, factory }.into_any_element()
    }
}

#[derive(gpui::IntoElement)]
struct NativeElement {
    request: ElementRequest,
    factory: ElementFactory,
}

impl RenderOnce for NativeElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match (self.factory)(self.request, window, cx) {
            Ok(element) => element,
            Err(error) => crate::renderer::render_error(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_handlers_are_errors_and_registrations_survive_cloning() {
        let bindings = RuntimeBindings::new().on("save", |_, _, _| {});
        assert!(bindings.clone().handler("save").is_ok());
        assert!(bindings
            .handler("missing")
            .err()
            .unwrap()
            .contains("missing"));
        assert!(bindings.handler("{ arbitrary_code() }").is_err());
    }
}
