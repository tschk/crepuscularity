//! Retained single-line text input for the pinned GPUI-CE backend.
//! Runtime `input` nodes use keyed window state. Compiled templates may insert
//! an Entity<TextInput> as a native expression without changing their GPUI alias.
use std::{cell::Cell, ops::Range, rc::Rc};

use crepuscularity_core::{ast, TemplateContext};
use gpui::{
    accesskit, point, prelude::*, px, rems, size, App, AppContext, Bounds, Context, Div, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, InspectorElementId, LayoutId, Pixels, Point, Render, RenderOnce, ShapedLine,
    SharedString, Stateful, Subscription, TextRun, UTF16Selection, Window,
};

use crate::{
    bindings::{EventCallback, RuntimeBindings, RuntimeEvent, RuntimeEventKind},
    input_model::{byte_to_utf16, floor_boundary, range_from_utf16, single_line, InputModel},
    styler::apply_element_classes,
};

/// The complete draft emitted by input/change/submit handlers. Host state is
/// updated only by its callback; a context clone is never a two-way store.
#[derive(Clone, Debug)]
pub struct TextInputEvent {
    pub text: String,
    pub selection_utf16: Range<usize>,
    pub reversed: bool,
    pub composing: bool,
}

#[derive(Clone)]
struct InputConfig {
    id: String,
    value: Option<String>,
    placeholder: SharedString,
    label: SharedString,
    description: SharedString,
    disabled: bool,
    readonly: bool,
    element: ast::Element,
    context: TemplateContext,
    bindings: RuntimeBindings,
    input: Option<EventCallback>,
    change: Option<EventCallback>,
    submit: Option<EventCallback>,
}

impl InputConfig {
    fn parse(
        element: &ast::Element,
        context: &TemplateContext,
        id: &str,
        bindings: &RuntimeBindings,
    ) -> Result<Self, String> {
        use crate::runtime_element::{presence_boolean, text};
        if !element.children.is_empty() || !element.animations.is_empty() {
            return Err("GPUI input does not accept children or animate attributes".into());
        }
        let mut config = Self {
            id: id.into(),
            value: None,
            placeholder: "".into(),
            label: "".into(),
            description: "".into(),
            disabled: false,
            readonly: false,
            element: element.clone(),
            context: context.clone(),
            bindings: bindings.handlers_only(),
            input: None,
            change: None,
            submit: None,
        };
        for binding in &element.bindings {
            match binding.prop.as_str() {
                "id" | "key" => {},
                "type" if text(&binding.value, context)?.as_ref() == "text" => {},
                "value" => config.value = Some(text(&binding.value, context)?.to_string()),
                "placeholder" => config.placeholder = single_line(&text(&binding.value, context)?).into(),
                "aria-label" => config.label = text(&binding.value, context)?,
                "aria-description" => config.description = text(&binding.value, context)?,
                "disabled" => config.disabled = presence_boolean(&binding.value, context)?,
                "readonly" => config.readonly = presence_boolean(&binding.value, context)?,
                prop => return Err(format!("Unsupported GPUI input binding `{prop}`; only single-line type=text is implemented")),
            }
        }
        let mut seen = std::collections::HashSet::new();
        for handler in &element.event_handlers {
            if !seen.insert(handler.event.as_str()) {
                return Err(format!("Duplicate GPUI event @{}", handler.event));
            }
            let slot = match handler.event.as_str() {
                "input" => &mut config.input,
                "change" => &mut config.change,
                "submit" => &mut config.submit,
                _ => continue,
            };
            if !handler.modifiers.is_empty() {
                return Err("Input/change/submit events do not accept key modifiers".into());
            }
            *slot = Some(bindings.handler(&handler.handler)?);
        }
        config
            .element
            .event_handlers
            .retain(|h| !matches!(h.event.as_str(), "input" | "change" | "submit"));
        // Validate other listeners before delaying construction until native render.
        crate::runtime_element::apply_events(
            gpui::div().id("validate"),
            &config.element,
            context,
            id,
            &config.bindings,
        )?;
        Ok(config)
    }
}

#[derive(gpui::IntoElement)]
struct RuntimeInput {
    config: InputConfig,
}

pub(crate) fn runtime_input(
    element: &ast::Element,
    context: &TemplateContext,
    id: &str,
    bindings: &RuntimeBindings,
) -> Result<gpui::AnyElement, String> {
    Ok(RuntimeInput {
        config: InputConfig::parse(element, context, id, bindings)?,
    }
    .into_any_element())
}

impl RenderOnce for RuntimeInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.config.value.clone().unwrap_or_default();
        let entity = window.use_keyed_state(
            SharedString::from(self.config.id.clone()),
            cx,
            |window, cx| TextInput::new(value, window, cx),
        );
        entity.update(cx, |input, cx| {
            input.update_config(self.config, window, cx);
        });
        entity
    }
}

#[derive(Clone)]
struct TextGeometry {
    line: ShapedLine,
    bounds: Bounds<Pixels>,
    origin: Point<Pixels>,
    line_height: Pixels,
    text: String,
    run: TextRun,
    font_size: Pixels,
    alignment: gpui::TextAlign,
}

/// A host-owned input entity may also be embedded directly in compiled templates.
pub struct TextInput {
    model: InputModel,
    focus: FocusHandle,
    config: Option<InputConfig>,
    geometry: Option<TextGeometry>,
    scroll_x: Pixels,
    dragging: bool,
    committed_text: String,
    a11y_run_id: Rc<Cell<Option<accesskit::NodeId>>>,
    _blur: Subscription,
}

impl TextInput {
    fn update_config(&mut self, config: InputConfig, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(value) = &config.value {
            if self.model.sync_external(value) {
                self.committed_text = self.model.text().to_owned();
            }
        }
        let was_composing = self.model.is_composing();
        self.model.set_permissions(config.disabled, config.readonly);
        if config.disabled && self.focus.is_focused(window) {
            window.blur();
        }
        self.config = Some(config);
        if was_composing && !self.model.is_composing() {
            // A render may be borrowing the host view. Deliver the terminal
            // composition snapshot after rendering before invoking its handler.
            let event = self.event();
            let config = self.config.clone();
            cx.defer_in(window, move |_, window, cx| {
                cx.emit(event.clone());
                Self::emit_config(config.as_ref(), "input", &event, window, cx);
            });
        }
    }

    pub fn new(value: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model = InputModel::new(value);
        let focus = cx.focus_handle().tab_stop(true);
        let blur = cx.on_blur(&focus, window, |this, window, cx| {
            this.edit(window, cx, |m| m.commit_composition());
            this.dragging = false;
            this.emit_change(window, cx);
            cx.notify();
        });
        Self {
            committed_text: model.text().into(),
            model,
            focus,
            config: None,
            geometry: None,
            scroll_x: px(0.),
            dragging: false,
            a11y_run_id: Rc::new(Cell::new(None)),
            _blur: blur,
        }
    }

    pub fn model(&self) -> &InputModel {
        &self.model
    }
    pub fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        if self.model.sync_external(value) {
            self.committed_text = self.model.text().to_owned();
        }
        cx.notify();
    }
    pub fn set_permissions(
        &mut self,
        disabled: bool,
        readonly: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(window, cx, |m| m.set_permissions(disabled, readonly));
        if disabled && self.focus.is_focused(window) {
            window.blur();
        }
        cx.notify();
    }

    fn event(&self) -> TextInputEvent {
        TextInputEvent {
            text: self.model.text().into(),
            selection_utf16: self.model.selection_utf16(),
            reversed: self.model.reversed(),
            composing: self.model.is_composing(),
        }
    }

    fn emit(&self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        Self::emit_config(self.config.as_ref(), name, &self.event(), window, cx);
    }

    fn emit_config(
        config: Option<&InputConfig>,
        name: &str,
        event: &TextInputEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(config) = config else {
            return;
        };
        let callback = match name {
            "input" => &config.input,
            "change" => &config.change,
            _ => &config.submit,
        };
        if let Some(callback) = callback {
            let kind = match name {
                "input" => RuntimeEventKind::Input(event),
                "change" => RuntimeEventKind::Change(event),
                _ => RuntimeEventKind::Submit(event),
            };
            callback(
                RuntimeEvent {
                    element_id: &config.id,
                    context: &config.context,
                    kind,
                },
                window,
                cx,
            );
        }
    }

    fn emit_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.committed_text != self.model.text() {
            self.committed_text = self.model.text().into();
            self.emit("change", window, cx);
        }
    }

    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut InputModel),
    ) {
        let before = (self.model.text().to_owned(), self.model.is_composing());
        edit(&mut self.model);
        if before != (self.model.text().to_owned(), self.model.is_composing()) {
            cx.emit(self.event());
            self.emit("input", window, cx);
        }
        cx.notify();
    }

    fn key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.disabled() {
            return;
        }
        let mods = event.keystroke.modifiers;
        let primary = if cfg!(target_os = "macos") {
            mods.platform && !mods.control
        } else {
            mods.control && !mods.platform
        };
        let key = event.keystroke.key.as_str();
        let plain = !mods.platform && !mods.control && !mods.alt && !mods.function;
        let word = if cfg!(target_os = "macos") {
            mods.alt && !mods.control && !mods.platform
        } else {
            mods.control && !mods.alt && !mods.platform
        };
        let mut handled = true;
        if word && !mods.function && matches!(key, "left" | "right") {
            self.edit(window, cx, |m| m.move_word(key == "right", mods.shift));
        } else if primary && !mods.alt && !mods.function {
            match key {
                "a" => self.edit(window, cx, |m| {
                    m.commit_composition();
                    m.select_all();
                }),
                "c" | "x" => {
                    let selected = self.model.selection();
                    if !selected.is_empty() {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            self.model.text()[selected].into(),
                        ));
                        if key == "x" {
                            self.edit(window, cx, |m| {
                                m.replace_utf16(Some(m.selection_utf16()), "")
                            });
                        }
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                        self.edit(window, cx, |m| m.replace_utf16(None, &text));
                    }
                }
                "z" => self.edit(window, cx, |m| if mods.shift { m.redo() } else { m.undo() }),
                "y" if !cfg!(target_os = "macos") => self.edit(window, cx, |m| m.redo()),
                "left" if cfg!(target_os = "macos") => {
                    self.edit(window, cx, |m| m.move_to(0, mods.shift))
                }
                "right" if cfg!(target_os = "macos") => {
                    self.edit(window, cx, |m| m.move_to(m.text().len(), mods.shift))
                }
                _ => handled = false,
            }
        } else if plain {
            match key {
                "left" | "right" => {
                    self.edit(window, cx, |m| m.move_grapheme(key == "right", mods.shift))
                }
                "home" => self.edit(window, cx, |m| m.move_to(0, mods.shift)),
                "end" => self.edit(window, cx, |m| m.move_to(m.text().len(), mods.shift)),
                "backspace" | "delete" => self.edit(window, cx, |m| m.delete(key == "delete")),
                "escape" if self.model.is_composing() => {
                    self.edit(window, cx, |m| m.cancel_composition())
                }
                "enter" if !self.model.is_composing() => {
                    self.emit_change(window, cx);
                    self.emit("submit", window, cx);
                }
                _ => handled = false,
            }
        } else {
            handled = false;
        }
        if handled {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn refresh_geometry(&mut self, window: &mut Window) {
        let Some(geometry) = self.geometry.as_mut() else {
            return;
        };
        if geometry.text != self.model.text() {
            geometry.text = self.model.text().to_owned();
            geometry.run.len = geometry.text.len();
            geometry.line = window.text_system().shape_line(
                SharedString::from(geometry.text.clone()),
                geometry.font_size,
                &[geometry.run.clone()],
                None,
            );
        }
        let caret = geometry.line.x_for_index(self.model.cursor());
        let width = (geometry.bounds.size.width - px(2.)).max(px(1.));
        self.scroll_x = self
            .scroll_x
            .min((geometry.line.width() - width).max(px(0.)));
        if caret < self.scroll_x {
            self.scroll_x = caret;
        }
        if caret > self.scroll_x + width {
            self.scroll_x = caret - width;
        }
        geometry.origin.x = geometry.bounds.left()
            + alignment_offset(geometry.alignment, width, geometry.line.width())
            - self.scroll_x;
    }

    fn index_at(&self, point: Point<Pixels>) -> Option<usize> {
        let geometry = self.geometry.as_ref()?;
        if geometry.text != self.model.text() {
            return None;
        }
        Some(floor_boundary(
            self.model.text(),
            geometry
                .line
                .closest_index_for_x(point.x - geometry.origin.x),
        ))
    }
}

impl gpui::EventEmitter<TextInputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, range) = self.model.text_for_utf16_range(range);
        *adjusted = Some(range);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        ignore_disabled: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.model.disabled() && !ignore_disabled {
            return None;
        }
        Some(UTF16Selection {
            range: self.model.selection_utf16(),
            reversed: self.model.reversed(),
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.model.marked_utf16()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |m| m.commit_composition());
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(window, cx, |m| m.replace_utf16(range, text));
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(window, cx, |m| {
            m.replace_and_mark_utf16(range, text, selected)
        });
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _bounds: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.refresh_geometry(window);
        let geometry = self.geometry.as_ref()?;
        if geometry.text != self.model.text() {
            return None;
        }
        let range = range_from_utf16(self.model.text(), range);
        let start = geometry.line.x_for_index(range.start);
        let end = geometry.line.x_for_index(range.end);
        let x = (geometry.origin.x + start.min(end))
            .max(geometry.bounds.left())
            .min(geometry.bounds.right());
        Some(Bounds::new(
            point(x, geometry.bounds.top()),
            size((end - start).abs().max(px(1.)), geometry.line_height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.refresh_geometry(window);
        self.index_at(point)
            .map(|i| byte_to_utf16(self.model.text(), i))
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.select_utf16(range);
        cx.notify();
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.model.text().encode_utf16().count())
    }
    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.model.accepts_input()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut d = gpui::div()
            .id("input")
            .w(rems(16.))
            .min_h(window.line_height())
            .overflow_hidden()
            .cursor_text()
            .role(accesskit::Role::TextInput);
        if let Some(config) = &self.config {
            let classes = config.element.classes.iter().map(String::as_str).chain(
                config
                    .element
                    .conditional_classes
                    .iter()
                    .filter(|c| config.context.eval_condition(&c.condition).unwrap_or(false))
                    .map(|c| c.class.as_str()),
            );
            d = apply_element_classes(d, classes, Some(&config.context));
            d = crate::runtime_element::apply_events(
                d,
                &config.element,
                &config.context,
                &config.id,
                &config.bindings,
            )
            .expect("RuntimeInput validates event bindings before construction");
            d = d
                .aria_label(config.label.clone())
                .aria_description(config.description.clone())
                .aria_placeholder(config.placeholder.clone());
        }
        d = d.aria_value(SharedString::from(self.model.text().to_owned()));
        if !self.model.disabled() {
            d = d
                .track_focus(&self.focus)
                .on_key_down(cx.listener(Self::key_down))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                        this.focus.focus(window, cx);
                        this.edit(window, cx, |m| m.commit_composition());
                        this.refresh_geometry(window);
                        if event.click_count >= 3 {
                            this.model.select_all();
                        } else if let Some(index) = this.index_at(event.position) {
                            this.model.move_to(index, event.modifiers.shift);
                        }
                        this.dragging = true;
                        cx.notify();
                    }),
                )
                .on_mouse_move(
                    cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                        this.dragging =
                            this.dragging && event.pressed_button == Some(gpui::MouseButton::Left);
                        if this.dragging {
                            this.refresh_geometry(window);
                            if let Some(index) = this.index_at(event.position) {
                                this.model.move_to(index, true);
                                cx.notify();
                            }
                        }
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, _| this.dragging = false),
                )
                .on_mouse_up_out(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, _| this.dragging = false),
                );
            let entity = cx.entity().downgrade();
            d = d.on_a11y_action(
                accesskit::Action::SetTextSelection,
                move |data, _window, cx| {
                    let Some(accesskit::ActionData::SetTextSelection(selection)) = data else {
                        return;
                    };
                    let _ = entity.update(cx, |this, cx| {
                        let Some(run) = this.a11y_run_id.get() else {
                            return;
                        };
                        if selection.anchor.node != run || selection.focus.node != run {
                            return;
                        }
                        let byte = |index| {
                            this.model
                                .text()
                                .char_indices()
                                .nth(index)
                                .map(|(i, _)| i)
                                .unwrap_or(this.model.text().len())
                        };
                        let anchor = byte(selection.anchor.character_index);
                        let cursor = byte(selection.focus.character_index);
                        this.model.select_bytes(anchor, cursor);
                        cx.notify();
                    });
                },
            );
            if !self.model.readonly() {
                let entity = cx.entity().downgrade();
                d = d.on_a11y_action(accesskit::Action::SetValue, move |data, window, cx| {
                    if let Some(accesskit::ActionData::Value(value)) = data {
                        let _ = entity.update(cx, |this, cx| {
                            let end = this.model.text().encode_utf16().count();
                            this.edit(window, cx, |m| m.replace_utf16(Some(0..end), value));
                        });
                    }
                });
            }
        } else {
            d = d.cursor_not_allowed().opacity(0.5);
        }
        let semantics = InputSemantics {
            text: self.model.text().to_owned(),
            anchor: self.model.anchor(),
            cursor: self.model.cursor(),
            disabled: self.model.disabled(),
            readonly: self.model.readonly(),
            run_id: self.a11y_run_id.clone(),
        };
        InputContainer {
            inner: d.child(InputText { input: cx.entity() }),
            semantics,
        }
    }
}

struct InputText {
    input: Entity<TextInput>,
}
impl IntoElement for InputText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
struct InputPaint {
    geometry: TextGeometry,
    display_line: ShapedLine,
    display_origin: Point<Pixels>,
    cursor: Option<gpui::PaintQuad>,
    selection: Option<gpui::PaintQuad>,
    composition: Option<gpui::PaintQuad>,
}

impl Element for InputText {
    type RequestLayoutState = ();
    type PrepaintState = InputPaint;
    fn id(&self) -> Option<ElementId> {
        Some("text".into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> InputPaint {
        let input = self.input.read(cx);
        let text = input.model.text().to_owned();
        let placeholder = text.is_empty();
        let display: SharedString = if placeholder {
            input
                .config
                .as_ref()
                .map(|c| c.placeholder.clone())
                .unwrap_or_default()
        } else {
            text.clone().into()
        };
        let style = window.text_style();
        let line_height = window.line_height();
        let color = style.color;
        let run = style.to_run(text.len());
        let font_size = style.font_size.to_pixels(window.rem_size());
        let display_line = window.text_system().shape_line(
            display.clone(),
            font_size,
            &[style.to_run(display.len())],
            None,
        );
        // Hints are paint-only. Empty text has its own alignment point, caret
        // and IME geometry independent of the placeholder's width.
        let line = if placeholder {
            window
                .text_system()
                .shape_line("".into(), font_size, &[run.clone()], None)
        } else {
            display_line.clone()
        };
        let caret_x = line.x_for_index(input.model.cursor());
        let available = (bounds.size.width - px(2.)).max(px(1.));
        let mut scroll_x = input.scroll_x.min((line.width() - available).max(px(0.)));
        if caret_x < scroll_x {
            scroll_x = caret_x;
        }
        if caret_x > scroll_x + available {
            scroll_x = caret_x - available;
        }
        let origin = point(
            bounds.left() + alignment_offset(style.text_align, available, line.width()) - scroll_x,
            bounds.top(),
        );
        let display_origin = if placeholder {
            point(
                bounds.left() + alignment_offset(style.text_align, available, display_line.width()),
                bounds.top(),
            )
        } else {
            origin
        };
        let focused = input.focus.is_focused(window) && !input.model.disabled();
        let rect = |range: Range<usize>, height: Pixels, y: Pixels| {
            let a = line.x_for_index(range.start);
            let b = line.x_for_index(range.end);
            Bounds::new(
                point(origin.x + a.min(b), y),
                size((b - a).abs().max(px(1.)), height),
            )
        };
        let selection = if focused && !input.model.selection().is_empty() {
            Some(gpui::fill(
                rect(input.model.selection(), line_height, origin.y),
                gpui::rgba(0x4488cc66),
            ))
        } else {
            None
        };
        let composition = input
            .model
            .marked()
            .map(|r| gpui::fill(rect(r, px(1.), origin.y + line_height - px(1.)), color));
        let cursor = if focused {
            Some(gpui::fill(
                Bounds::new(
                    point(origin.x + caret_x, origin.y),
                    size(px(1.5), line_height),
                ),
                color,
            ))
        } else {
            None
        };
        let geometry = TextGeometry {
            line,
            bounds,
            origin,
            line_height,
            text,
            run,
            font_size,
            alignment: style.text_align,
        };
        self.input.update(cx, |input, _| {
            input.geometry = Some(geometry.clone());
            input.scroll_x = scroll_x;
        });
        InputPaint {
            geometry,
            display_line,
            display_origin,
            cursor,
            selection,
            composition,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        paint: &mut InputPaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = {
            let input = self.input.read(cx);
            (!input.model.disabled()).then(|| input.focus.clone())
        };
        if let Some(focus) = focus {
            window.handle_input(
                &focus,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
        }
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            let _ = paint.display_line.paint_background(
                paint.display_origin,
                paint.geometry.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(selection) = paint.selection.take() {
                window.paint_quad(selection);
            }
            let _ = paint.display_line.paint(
                paint.display_origin,
                paint.geometry.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(composition) = paint.composition.take() {
                window.paint_quad(composition);
            }
            if let Some(cursor) = paint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
    }
}

fn alignment_offset(alignment: gpui::TextAlign, available: Pixels, line_width: Pixels) -> Pixels {
    let space = (available - line_width).max(px(0.));
    match alignment {
        gpui::TextAlign::Left => px(0.),
        gpui::TextAlign::Center => space / 2.,
        gpui::TextAlign::Right => space,
    }
}

struct InputSemantics {
    text: String,
    anchor: usize,
    cursor: usize,
    disabled: bool,
    readonly: bool,
    run_id: Rc<Cell<Option<accesskit::NodeId>>>,
}
struct InputContainer {
    inner: Stateful<Div>,
    semantics: InputSemantics,
}
impl IntoElement for InputContainer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for InputContainer {
    type RequestLayoutState = <Stateful<Div> as Element>::RequestLayoutState;
    type PrepaintState = <Stateful<Div> as Element>::PrepaintState;
    fn id(&self) -> Option<ElementId> {
        gpui::Element::id(&self.inner)
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::TextInput)
    }
    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node);
        if self.semantics.disabled {
            node.set_disabled();
        }
        if self.semantics.readonly {
            node.set_read_only();
        }
    }
    fn a11y_synthetic_children(
        &mut self,
        _: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        let mut run = accesskit::Node::new(accesskit::Role::TextRun);
        run.set_value(self.semantics.text.clone());
        run.set_character_lengths(
            self.semantics
                .text
                .chars()
                .map(|c| c.len_utf8() as u8)
                .collect::<Vec<_>>(),
        );
        let id = builder.synthetic_node_id("text");
        self.semantics.run_id.set(Some(id));
        builder.push_child(id, run);
        let position = |byte| accesskit::TextPosition {
            node: id,
            character_index: self.semantics.text[..byte].chars().count(),
        };
        builder
            .parent_node()
            .set_text_selection(accesskit::TextSelection {
                anchor: position(self.semantics.anchor),
                focus: position(self.semantics.cursor),
            });
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector, bounds, state, window, cx)
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        paint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector, bounds, state, paint, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_input_rejects_unsupported_types_and_resolves_named_callbacks() {
        let bindings = RuntimeBindings::new().on("edited", |_, _, _| {});
        let context = TemplateContext::new();
        let nodes = crepuscularity_core::parser::parse_template(
            "input value={\"hello\"} readonly @input=edited",
        )
        .unwrap();
        let ast::Node::Element(element) = &nodes[0] else {
            panic!()
        };
        let config = InputConfig::parse(element, &context, "field", &bindings).unwrap();
        assert_eq!(config.value.as_deref(), Some("hello"));
        assert!(config.readonly);
        assert!(config.input.is_some());
        for source in [
            "input type=\"password\"",
            "input type=\"number\"",
            "input @input=missing",
            "input @input|Enter=edited",
        ] {
            let nodes = crepuscularity_core::parser::parse_template(source).unwrap();
            let ast::Node::Element(element) = &nodes[0] else {
                panic!()
            };
            assert!(
                InputConfig::parse(element, &context, "field", &bindings).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn accessibility_flags_are_written_on_the_actual_text_input_node() {
        let container = InputContainer {
            inner: gpui::div()
                .id("field")
                .role(accesskit::Role::TextInput)
                .aria_label("Name")
                .aria_value("abc"),
            semantics: InputSemantics {
                text: "abc".into(),
                anchor: 3,
                cursor: 0,
                disabled: true,
                readonly: true,
                run_id: Rc::new(Cell::new(None)),
            },
        };
        let mut node = accesskit::Node::new(container.a11y_role().unwrap());
        container.write_a11y_info(&mut node);
        assert_eq!(node.role(), accesskit::Role::TextInput);
        assert_eq!(node.label(), Some("Name"));
        assert_eq!(node.value(), Some("abc"));
        assert!(node.is_disabled());
        assert!(node.is_read_only());
    }

    #[test]
    fn runtime_placeholder_is_normalized_before_single_line_shaping() {
        let mut context = TemplateContext::new();
        context.set("hint", "one\r\ntwo\nthree\rfour");
        let nodes =
            crepuscularity_core::parser::parse_template("input placeholder={hint}").unwrap();
        let ast::Node::Element(element) = &nodes[0] else {
            panic!()
        };
        let config =
            InputConfig::parse(element, &context, "field", &RuntimeBindings::new()).unwrap();
        assert_eq!(config.placeholder.as_ref(), "one two three four");
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn runtime_permission_update_emits_terminal_composition_snapshot(
        cx: &mut gpui::TestAppContext,
    ) {
        let events = Rc::new(std::cell::RefCell::new(Vec::new()));
        let output = events.clone();
        let bindings = RuntimeBindings::new().on("edited", move |event, _, _| {
            if let RuntimeEventKind::Input(input) = event.kind {
                output
                    .borrow_mut()
                    .push((input.text.clone(), input.composing));
            }
        });
        let nodes =
            crepuscularity_core::parser::parse_template("input readonly={locked} @input=edited")
                .unwrap();
        let ast::Node::Element(element) = &nodes[0] else {
            panic!()
        };
        let mut context = TemplateContext::new();
        context.set("locked", false);
        let initial = InputConfig::parse(element, &context, "field", &bindings).unwrap();
        context.set("locked", true);
        let locked = InputConfig::parse(element, &context, "field", &bindings).unwrap();
        let (field, cx) = cx.add_window_view(|window, cx| TextInput::new("", window, cx));
        cx.update(|window, cx| {
            field.update(cx, |input, cx| {
                input.update_config(initial, window, cx);
                input.replace_and_mark_text_in_range(None, "拼", Some(1..1), window, cx);
                input.update_config(locked, window, cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(
            &*events.borrow(),
            &[("拼".to_owned(), true), ("拼".to_owned(), false)]
        );
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn empty_input_geometry_is_independent_of_aligned_hint_width(cx: &mut gpui::TestAppContext) {
        for alignment in ["center", "right"] {
            let nodes = crepuscularity_core::parser::parse_template(&format!(
                "input w-[200px] text-{alignment} placeholder={{hint}}"
            ))
            .unwrap();
            let ast::Node::Element(element) = &nodes[0] else {
                panic!()
            };
            let (field, view_cx) = cx.add_window_view(|window, cx| TextInput::new("", window, cx));
            let expected_x = |bounds: Bounds<Pixels>| {
                let width = (bounds.size.width - px(2.)).max(px(1.));
                bounds.left()
                    + if alignment == "center" {
                        width / 2.
                    } else {
                        width
                    }
            };
            for hint in ["a", "a much longer hint"] {
                let mut context = TemplateContext::new();
                context.set("hint", hint);
                let config =
                    InputConfig::parse(element, &context, "field", &RuntimeBindings::new())
                        .unwrap();
                view_cx.update(|window, cx| {
                    field.update(cx, |input, cx| input.update_config(config, window, cx));
                    window.draw(cx).clear(cx);
                    field.update(cx, |input, cx| {
                        let geometry = input.geometry.clone().unwrap();
                        assert_eq!(geometry.line.text.as_ref(), "");
                        assert_eq!(geometry.origin.x, expected_x(geometry.bounds));
                        assert_eq!(
                            input
                                .bounds_for_range(0..0, geometry.bounds, window, cx)
                                .unwrap()
                                .origin
                                .x,
                            expected_x(geometry.bounds)
                        );
                        input.replace_text_in_range(None, "filled", window, cx);
                    });
                    window.draw(cx).clear(cx);
                    field.update(cx, |input, cx| {
                        input.replace_text_in_range(Some(0..6), "", window, cx);
                        let bounds = input.geometry.as_ref().unwrap().bounds;
                        let before_paint =
                            input.bounds_for_range(0..0, bounds, window, cx).unwrap();
                        assert_eq!(before_paint.origin.x, expected_x(bounds));
                    });
                    window.draw(cx).clear(cx);
                    field.update(cx, |input, _| {
                        let geometry = input.geometry.as_ref().unwrap();
                        assert_eq!(geometry.line.text.as_ref(), "");
                        assert_eq!(geometry.origin.x, expected_x(geometry.bounds));
                    });
                });
            }
        }
    }
}
