use std::path::{Path, PathBuf};

use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::{CompletedFrame, Frame, Terminal};

use crate::component::ComponentRegistry;
use crate::diff::DiffTracker;
use crate::event::{DispatchResult, Event, EventDispatcher, FocusManager};
use crate::render::collect_focusable_ids_with_context;
use crate::{render_template, TemplateContext};

pub struct Template {
    path: PathBuf,
    source: String,
    ctx: TemplateContext,
    focus: FocusManager,
    dispatcher: EventDispatcher,
    diff: DiffTracker,
    rendered: Option<(Rect, Buffer)>,
    components: ComponentRegistry,
}

impl Clone for Template {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            source: self.source.clone(),
            ctx: self.ctx.clone(),
            focus: self.focus.clone(),
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            rendered: None,
            components: self.components.clone(),
        }
    }
}

/// The outcome of [`Template::handle_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// The event was handled (focus moved or a callback consumed it).
    Handled,
    /// The event was not handled.
    Unhandled,
}

#[derive(Clone, Debug)]
pub struct ElementRef {
    pub id: &'static str,
    pub content: String,
}

impl ElementRef {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            content: String::new(),
        }
    }

    pub fn set_content(&mut self, content: impl Into<String>) -> &mut Self {
        self.content = content.into();
        self
    }

    pub fn content(&mut self, content: impl Into<String>) -> &mut Self {
        self.set_content(content)
    }

    pub fn text(&mut self, content: impl Into<String>) -> &mut Self {
        self.set_content(content)
    }

    pub fn val(&mut self, content: impl Into<String>) -> &mut Self {
        self.set_content(content)
    }

    pub fn clear(&mut self) -> &mut Self {
        self.content.clear();
        self
    }
}

impl Template {
    pub fn from_source(source: impl Into<String>) -> Self {
        Self {
            path: PathBuf::new(),
            source: source.into(),
            ctx: TemplateContext::new(),
            focus: FocusManager::new(),
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            rendered: None,
            components: ComponentRegistry::new(),
        }
    }

    pub fn from_source_with_path(source: impl Into<String>, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut ctx = TemplateContext::new();
        ctx.base_dir = path.parent().map(Path::to_path_buf);
        Self {
            path,
            source: source.into(),
            ctx,
            focus: FocusManager::new(),
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            rendered: None,
            components: ComponentRegistry::new(),
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("template error: {:?}: {}", path, e))?;
        let mut ctx = TemplateContext::new();
        ctx.base_dir = path.parent().map(Path::to_path_buf);
        Ok(Self {
            path,
            source,
            ctx,
            focus: FocusManager::new(),
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            rendered: None,
            components: ComponentRegistry::new(),
        })
    }

    pub fn set(
        &mut self,
        key: impl Into<String>,
        value: impl Into<crate::TemplateValue>,
    ) -> &mut Self {
        self.ctx.set(key, value);
        self
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<crate::TemplateValue>) -> Self {
        self.set(key, value);
        self
    }

    pub fn context(&self) -> &TemplateContext {
        &self.ctx
    }

    pub fn context_mut(&mut self) -> &mut TemplateContext {
        self.invalidate_render_cache();
        &mut self.ctx
    }

    pub fn register_component(&mut self, name: &str, source: &str) -> &mut Self {
        self.components.register(name, source);
        std::sync::Arc::make_mut(&mut self.ctx.virtual_files)
            .insert(name.to_string(), source.to_string());
        self.invalidate_render_cache();
        self
    }

    pub fn components(&self) -> &ComponentRegistry {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut ComponentRegistry {
        self.invalidate_render_cache();
        &mut self.components
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Replace the in-memory template source.
    ///
    /// Useful for live-reload pipelines that want to keep the same [`TemplateContext`]
    /// (variables, `base_dir`) but swap the rendered DSL.
    pub fn set_source(&mut self, source: impl Into<String>) -> &mut Self {
        let source = source.into();
        if self.source != source {
            self.source = source;
            self.invalidate_render_cache();
            self.refresh_focus();
        }
        self
    }

    /// Re-read the template source from [`Template::path`].
    ///
    /// Returns an error if this template was constructed without a path
    /// ([`Template::from_source`]) or if reading fails. On error the previous
    /// source is left intact so the next [`Template::draw`] still renders.
    pub fn reload(&mut self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Err("template has no path; reload requires `from_path`".to_string());
        }
        let source = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("template error: {:?}: {}", self.path, e))?;
        self.set_source(source);
        Ok(())
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) -> Result<(), String> {
        render_template(&self.source, &self.ctx, frame, area).map_err(|e| e.to_string())
    }

    pub fn draw_full(&self, frame: &mut Frame) -> Result<(), String> {
        self.draw(frame, frame.area())
    }

    /// Draw changed template output and replay the cached buffer on skipped renders.
    ///
    /// Returns `true` when the template was re-rendered and `false` when the previous
    /// buffer was reused. Replaying the buffer keeps Ratatui frames complete after its
    /// back buffer is reset between draw calls.
    pub fn draw_if_changed(&mut self, frame: &mut Frame, area: Rect) -> Result<bool, String> {
        let needs_render = self.diff.has_changed(&self.ctx)
            || self
                .rendered
                .as_ref()
                .is_none_or(|(rendered_area, _)| *rendered_area != area);

        if needs_render {
            let mut buffer = Buffer::empty(area);
            crate::render_template_to_buffer(&self.source, &self.ctx, &mut buffer, area)
                .map_err(|e| e.to_string())?;
            self.diff.update(&self.ctx);
            self.rendered = Some((area, buffer));
        }

        if let Some((_, buffer)) = &self.rendered {
            frame.buffer_mut().merge(buffer);
        }

        Ok(needs_render)
    }

    pub fn draw_full_if_changed(&mut self, frame: &mut Frame) -> Result<bool, String> {
        self.draw_if_changed(frame, frame.area())
    }

    pub fn changed_keys(&self) -> Vec<String> {
        self.diff.changed_keys(&self.ctx)
    }

    pub fn mark_rendered(&mut self) {
        self.diff.update(&self.ctx);
    }

    /// Invalidate the cached render output.
    ///
    /// Call this after changing rendering inputs that are not tracked in the
    /// template context, such as an included file on disk.
    pub fn invalidate(&mut self) {
        self.invalidate_render_cache();
    }

    fn invalidate_render_cache(&mut self) {
        self.diff = DiffTracker::new();
        self.rendered = None;
    }

    /// Returns a reference to the focus manager.
    pub fn focus(&self) -> &FocusManager {
        &self.focus
    }

    /// Returns a mutable reference to the focus manager.
    pub fn focus_mut(&mut self) -> &mut FocusManager {
        &mut self.focus
    }

    /// Returns a builder for registering callbacks on the event dispatcher.
    /// Register callbacks before calling [`Template::handle_event`].
    pub fn dispatcher_mut(&mut self) -> EventDispatcherBuilder<'_> {
        EventDispatcherBuilder {
            dispatcher: &mut self.dispatcher,
        }
    }

    /// Refresh the focusable ID list from the current template source and sync
    /// the `focused_id` context variable.
    pub fn refresh_focus(&mut self) {
        if let Ok(ids) = collect_focusable_ids_with_context(&self.source, &self.ctx) {
            self.focus.set_focusable_ids(ids);
        }
        let focused = self.focus.focused_id.clone().unwrap_or_default();
        self.ctx.set("focused_id", focused);
    }

    /// Handle a crossterm [`Event`].
    ///
    /// Tab / Shift-Tab cycle focus through focusable elements. The event is
    /// then dispatched to the focused element's callback (with bubbling). The
    /// `focused_id` context variable is updated so templates can react.
    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> EventResult {
        let ev = Event::from(event.clone());
        self.handle_tui_event(&ev)
    }

    /// Handle a crate-level [`Event`].
    pub fn handle_tui_event(&mut self, event: &Event) -> EventResult {
        if self.focus.focusable_ids.is_empty() {
            self.refresh_focus();
        }

        let mut handled = false;
        if let Event::Key(key) = event {
            use crossterm::event::{KeyCode, KeyEventKind};
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Tab => {
                        self.focus.focus_next();
                        handled = true;
                    }
                    KeyCode::BackTab => {
                        self.focus.focus_prev();
                        handled = true;
                    }
                    _ => {}
                }
            }
        }

        let focused = self.focus.focused_id.clone().unwrap_or_default();
        self.ctx.set("focused_id", focused);

        let dispatch_result = self
            .dispatcher
            .dispatch(event, self.focus.focused_id.as_deref());
        if dispatch_result == DispatchResult::Handled {
            handled = true;
        }

        if handled {
            EventResult::Handled
        } else {
            EventResult::Unhandled
        }
    }
}

/// Builder for registering callbacks on a [`Template`]'s dispatcher.
pub struct EventDispatcherBuilder<'a> {
    dispatcher: &'a mut EventDispatcher,
}

impl<'a> EventDispatcherBuilder<'a> {
    /// Register a callback for an element ID.
    pub fn on<F>(self, id: &str, callback: F) -> Self
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        self.dispatcher.on(id, callback);
        self
    }

    /// Register a parent relationship for event bubbling.
    pub fn parent(self, child: &str, parent: &str) -> Self {
        self.dispatcher.set_parent(child, parent);
        self
    }
}

pub fn template(path: impl AsRef<Path>) -> Result<Template, String> {
    Template::from_path(path)
}

pub fn draw<'a, B, F>(
    terminal: &'a mut Terminal<B>,
    path: impl AsRef<Path>,
    update: F,
) -> Result<CompletedFrame<'a>, String>
where
    B: Backend,
    F: FnOnce(&mut Template),
{
    let mut tpl = template(path)?;
    update(&mut tpl);
    let mut render_err = None;
    let frame = terminal
        .draw(|frame| {
            if let Err(err) = tpl.draw_full(frame) {
                render_err = Some(err);
            }
        })
        .map_err(|e| format!("terminal draw error: {e}"))?;
    if let Some(err) = render_err {
        return Err(err);
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_ref_new_initialization() {
        let el = ElementRef::new("test-id");
        assert_eq!(el.id, "test-id");
        assert_eq!(el.content, "");
    }

    #[test]
    fn element_ref_set_content() {
        let mut el = ElementRef::new("test-id");
        el.set_content("Hello");
        assert_eq!(el.content, "Hello");
    }

    #[test]
    fn element_ref_content_alias() {
        let mut el = ElementRef::new("test-id");
        el.content("World");
        assert_eq!(el.content, "World");
    }

    #[test]
    fn element_ref_text_alias() {
        let mut el = ElementRef::new("test-id");
        el.text("TextContent");
        assert_eq!(el.content, "TextContent");
    }

    #[test]
    fn element_ref_val_alias() {
        let mut el = ElementRef::new("test-id");
        el.val("ValContent");
        assert_eq!(el.content, "ValContent");
    }

    #[test]
    fn element_ref_clear() {
        let mut el = ElementRef::new("test-id");
        el.set_content("Should be cleared");
        assert_eq!(el.content, "Should be cleared");

        el.clear();
        assert_eq!(el.content, "");
    }

    #[test]
    fn element_ref_chaining() {
        let mut el = ElementRef::new("test-id");
        el.set_content("A")
            .content("B")
            .text("C")
            .val("D")
            .clear()
            .text("Final");

        assert_eq!(el.content, "Final");
    }
}
