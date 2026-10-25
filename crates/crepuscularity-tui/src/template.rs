use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::{CompletedFrame, Frame, Terminal};

use crate::component::ComponentRegistry;
use crate::diff::DiffTracker;
use crate::event::{DispatchResult, Event, EventDispatcher, FocusManager};
use crate::render::{collect_focusable_ids_with_context, current_unix_ms, BufferRenderer};
use crate::TemplateContext;

pub struct Template {
    path: PathBuf,
    source: String,
    parsed_nodes: OnceLock<Arc<Vec<crepuscularity_core::ast::Node>>>,
    ctx: TemplateContext,
    focus: FocusManager,
    automatic_focus: bool,
    dispatcher: EventDispatcher,
    diff: DiffTracker,
    last_draw_area: Option<Rect>,
    cached_draw: Option<ratatui::buffer::Buffer>,
    cached_time_window: Option<std::ops::Range<u64>>,
    components: ComponentRegistry,
}

impl Clone for Template {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            source: self.source.clone(),
            parsed_nodes: self.parsed_nodes.clone(),
            ctx: self.ctx.clone(),
            focus: self.focus.clone(),
            automatic_focus: self.automatic_focus,
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            last_draw_area: None,
            cached_draw: None,
            cached_time_window: None,
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
            parsed_nodes: OnceLock::new(),
            ctx: TemplateContext::new(),
            focus: FocusManager::new(),
            automatic_focus: true,
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            last_draw_area: None,
            cached_draw: None,
            cached_time_window: None,
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
            parsed_nodes: OnceLock::new(),
            ctx,
            focus: FocusManager::new(),
            automatic_focus: true,
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            last_draw_area: None,
            cached_draw: None,
            cached_time_window: None,
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
            parsed_nodes: OnceLock::new(),
            ctx,
            focus: FocusManager::new(),
            automatic_focus: true,
            dispatcher: EventDispatcher::new(),
            diff: DiffTracker::new(),
            last_draw_area: None,
            cached_draw: None,
            cached_time_window: None,
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

    /// Mutate inputs tracked by the render snapshot, including virtual files and slots.
    /// Equivalent replacements keep cached cells; external disk changes need [`Self::invalidate`].
    pub fn context_mut(&mut self) -> &mut TemplateContext {
        &mut self.ctx
    }

    pub fn register_component(&mut self, name: &str, source: &str) -> &mut Self {
        self.components.register(name, source);
        std::sync::Arc::make_mut(&mut self.ctx.virtual_files)
            .insert(name.to_string(), source.to_string());
        self.invalidate_render();
        self
    }

    pub fn components(&self) -> &ComponentRegistry {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut ComponentRegistry {
        self.invalidate_render();
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
            self.parsed_nodes.take();
            self.invalidate_render();
            if self.automatic_focus {
                self.refresh_focus();
            }
        }
        self
    }

    /// Invalidate cached output after a resource outside the context changes.
    /// The last coherent cells remain available if the next render fails.
    pub fn invalidate(&mut self) {
        self.invalidate_render();
    }

    pub(crate) fn invalidate_render(&mut self) {
        self.diff = DiffTracker::new();
    }

    /// Re-read the template source from [`Template::path`].
    ///
    /// Returns an error if this template was constructed without a path
    /// ([`Template::from_source`]) or if reading or parsing fails. On error the previous
    /// source is left intact so the next [`Template::draw`] still renders.
    pub fn reload(&mut self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Err("template has no path; reload requires `from_path`".to_string());
        }
        let source = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("template error: {:?}: {}", self.path, e))?;
        crepuscularity_core::parse_template_with_path(&source, Some(&self.path))
            .map_err(|e| e.to_string())?;
        self.set_source(source);
        Ok(())
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) -> Result<(), String> {
        self.draw_at(frame, area, current_unix_ms()).map(|_| ())
    }

    fn draw_at(&self, frame: &mut Frame, area: Rect, now_ms: u64) -> Result<Option<u64>, String> {
        let nodes = self.parse_nodes().map_err(|e| e.to_string())?;
        let mut renderer = BufferRenderer::new();
        renderer
            .render_nodes_at(&nodes, &self.ctx, frame.buffer_mut(), area, now_ms)
            .map_err(|e| e.to_string())?;
        Ok(renderer.next_redraw_ms())
    }

    fn parse_nodes(
        &self,
    ) -> Result<Arc<Vec<crepuscularity_core::ast::Node>>, crepuscularity_core::CrepusError> {
        if let Some(nodes) = self.parsed_nodes.get() {
            return Ok(Arc::clone(nodes));
        }
        let nodes = Arc::new(crepuscularity_core::parse_template_with_path(
            &self.source,
            Some(&self.path),
        )?);
        let _ = self.parsed_nodes.set(Arc::clone(&nodes));
        Ok(nodes)
    }

    pub fn draw_full(&self, frame: &mut Frame) -> Result<(), String> {
        self.draw(frame, frame.area())
    }

    /// Render changed content or copy cached cells into the new frame.
    /// Returns whether the template was rendered again; cached draws still paint.
    /// Failed renders return their error and repaint the last successful cached frame.
    /// The cache covers only the visible intersection of `area` and the frame.
    pub fn draw_if_changed(&mut self, frame: &mut Frame, area: Rect) -> Result<bool, String> {
        self.draw_if_changed_at(frame, area, current_unix_ms())
    }

    fn draw_if_changed_at(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        now_ms: u64,
    ) -> Result<bool, String> {
        if self.diff.has_changed(&self.ctx)
            || self
                .cached_time_window
                .as_ref()
                .is_some_and(|window| !window.contains(&now_ms))
            || self.last_draw_area != Some(area)
            || self
                .cached_draw
                .as_ref()
                .is_some_and(|cached| cached.area != area.intersection(frame.area()))
        {
            let next_redraw_ms = match self.draw_at(frame, area, now_ms) {
                Ok(next) => next,
                Err(error) => {
                    self.paint_cached(frame, area);
                    return Err(error);
                }
            };
            self.cached_time_window = next_redraw_ms.map(|next| now_ms..next);
            self.diff.update(&self.ctx);
            self.last_draw_area = Some(area);
            let buffer = frame.buffer_mut();
            let mut cached = ratatui::buffer::Buffer::empty(area.intersection(buffer.area));
            for y in cached.area.top()..cached.area.bottom() {
                for x in cached.area.left()..cached.area.right() {
                    cached[(x, y)] = buffer[(x, y)].clone();
                }
            }
            self.cached_draw = Some(cached);
            Ok(true)
        } else {
            self.paint_cached(frame, area);
            Ok(false)
        }
    }

    fn paint_cached(&self, frame: &mut Frame, area: Rect) {
        if let Some(cached) = &self.cached_draw {
            let buffer = frame.buffer_mut();
            let visible = cached.area.intersection(buffer.area).intersection(area);
            for y in visible.top()..visible.bottom() {
                for x in visible.left()..visible.right() {
                    buffer[(x, y)] = cached[(x, y)].clone();
                }
            }
        }
    }

    pub fn draw_full_if_changed(&mut self, frame: &mut Frame) -> Result<bool, String> {
        self.draw_if_changed(frame, frame.area())
    }

    pub fn changed_keys(&self) -> Vec<String> {
        self.diff.changed_keys(&self.ctx)
    }

    /// Acknowledge current variables and resources without validating cached cells.
    /// Changed inputs still require a redraw before the cell cache can be reused.
    pub fn mark_rendered(&mut self) {
        if self.diff.has_changed(&self.ctx) {
            self.last_draw_area = None;
        }
        self.diff.update(&self.ctx);
    }

    /// Returns a reference to the focus manager.
    pub fn focus(&self) -> &FocusManager {
        &self.focus
    }

    /// Returns the focus manager for manual control, including an empty focus list.
    /// Events and source changes preserve manual control. Call `refresh_focus` to
    /// resume automatic discovery from the template.
    pub fn focus_mut(&mut self) -> &mut FocusManager {
        self.automatic_focus = false;
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
        self.automatic_focus = true;
        let ids = self
            .parse_nodes()
            .map(|nodes| collect_focusable_ids_with_context(&nodes, &self.ctx))
            .unwrap_or_default();
        self.focus.set_focusable_ids(ids);
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
        if self.automatic_focus {
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
    fn manual_empty_focus_stays_empty_until_explicit_refresh() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut template = Template::from_source("button #automatic focusable");
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        template.dispatcher_mut().on("automatic", move |_| {
            callback_calls.fetch_add(1, Ordering::Relaxed);
            true
        });
        template.focus_mut().set_focusable_ids(Vec::new());
        assert_eq!(
            template.handle_tui_event(&Event::Paste("private input".into())),
            EventResult::Unhandled
        );
        assert!(template.focus().focusable_ids.is_empty());
        assert_eq!(template.focus().focused_id, None);
        assert_eq!(template.context().get_str("focused_id"), "");
        assert_eq!(calls.load(Ordering::Relaxed), 0);

        template.refresh_focus();
        assert_eq!(template.focus().focusable_ids, ["automatic"]);
        assert_eq!(
            template.handle_tui_event(&Event::Paste("dispatched input".into())),
            EventResult::Handled
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn source_replacement_preserves_manual_focus_until_explicit_refresh() {
        for ids in [Vec::new(), vec!["manual".to_string()]] {
            let mut template = Template::from_source("button #old focusable");
            template.focus_mut().set_focusable_ids(ids.clone());
            let focused = template.focus().focused_id.clone();
            template.set_source("button #replacement focusable");
            assert_eq!(template.focus().focusable_ids, ids);
            assert_eq!(template.focus().focused_id, focused);
            template.handle_tui_event(&Event::FocusGained);
            assert_eq!(template.focus().focusable_ids, ids);
            assert_eq!(template.focus().focused_id, focused);

            template.refresh_focus();
            assert_eq!(template.focus().focusable_ids, ["replacement"]);
            assert_eq!(template.focus().focused_id.as_deref(), Some("replacement"));
        }
    }

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

#[cfg(test)]
mod rotation_timing_tests {
    use super::*;
    use ratatui::backend::TestBackend;

    const ROTATOR: &str = "slot-rotate interval=20\n  \"First\"\n  \"Second\"";

    fn draw_at(template: &mut Template, terminal: &mut Terminal<TestBackend>, now: u64) -> bool {
        let mut redrawn = false;
        terminal
            .draw(|frame| {
                let area = frame.area();
                redrawn = template.draw_if_changed_at(frame, area, now).unwrap();
            })
            .unwrap();
        redrawn
    }

    fn text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn rotation_boundaries_skipped_periods_and_clock_rollback_invalidate_cache() {
        let mut template = Template::from_source(ROTATOR);
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        assert!(draw_at(&mut template, &mut terminal, 0));
        assert!(text(&terminal).contains("First"));
        assert!(!draw_at(&mut template, &mut terminal, 19));
        assert!(draw_at(&mut template, &mut terminal, 20));
        assert!(text(&terminal).contains("Second"));
        assert!(!draw_at(&mut template, &mut terminal, 39));
        assert!(draw_at(&mut template, &mut terminal, 100));
        assert!(text(&terminal).contains("Second"));
        assert!(!draw_at(&mut template, &mut terminal, 101));
        assert!(draw_at(&mut template, &mut terminal, 10));
        assert!(text(&terminal).contains("First"));
        assert!(!draw_at(&mut template, &mut terminal, 11));
    }

    #[test]
    fn multiple_rotators_use_the_earliest_boundary() {
        let mut template = Template::from_source("div\n  slot-rotate interval=100\n    \"A\"\n    \"B\"\n  slot-rotate interval=30\n    \"C\"\n    \"D\"");
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        assert!(draw_at(&mut template, &mut terminal, 0));
        assert!(!draw_at(&mut template, &mut terminal, 29));
        for boundary in [30, 60, 90, 100] {
            assert!(draw_at(&mut template, &mut terminal, boundary));
            assert!(!draw_at(&mut template, &mut terminal, boundary + 1));
        }
        assert!(text(&terminal).contains('B'));
        assert!(text(&terminal).contains('D'));
    }

    #[test]
    fn conditional_includes_only_schedule_visible_rotators() {
        let mut template =
            Template::from_source("if enabled\n  include child.crepus\nelse\n  span \"Static\"");
        template.register_component("child.crepus", ROTATOR);
        template.set("enabled", false);
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        assert!(draw_at(&mut template, &mut terminal, 0));
        assert!(!draw_at(&mut template, &mut terminal, 30));
        template.set("enabled", true);
        assert!(draw_at(&mut template, &mut terminal, 35));
        assert!(text(&terminal).contains("Second"));
        assert!(!draw_at(&mut template, &mut terminal, 39));
        assert!(draw_at(&mut template, &mut terminal, 40));
        assert!(text(&terminal).contains("First"));
        template.set("enabled", false);
        assert!(draw_at(&mut template, &mut terminal, 45));
        assert!(!draw_at(&mut template, &mut terminal, 100));
    }

    #[test]
    fn static_and_single_phrase_content_keep_their_cached_cells() {
        for source in [
            "span \"Static\"",
            "slot-rotate interval=0\n  \"Only\"",
            "slot-rotate interval=0",
        ] {
            let mut template = Template::from_source(source);
            let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
            assert!(draw_at(&mut template, &mut terminal, 0));
            assert!(!draw_at(&mut template, &mut terminal, 1000));
        }
    }

    #[test]
    fn zero_interval_and_extreme_clock_values_do_not_overflow() {
        let mut template =
            Template::from_source("slot-rotate interval=0\n  \"First\"\n  \"Second\"");
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        assert!(draw_at(&mut template, &mut terminal, 0));
        assert!(!draw_at(&mut template, &mut terminal, 0));
        assert!(draw_at(&mut template, &mut terminal, 1));
        assert!(text(&terminal).contains("Second"));
        assert!(draw_at(&mut template, &mut terminal, u64::MAX));
    }

    #[test]
    fn failed_draws_preserve_cached_cells_and_allow_static_recovery() {
        let mut template = Template::from_source(ROTATOR);
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        assert!(draw_at(&mut template, &mut terminal, 0));
        let previous = terminal.backend().buffer().clone();
        template.set_source("if {");
        for now in [25, 26] {
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    assert!(template.draw_if_changed_at(frame, area, now).is_err());
                })
                .unwrap();
            assert_eq!(terminal.backend().buffer(), &previous);
        }
        template.set_source("span \"Static\"");
        assert!(draw_at(&mut template, &mut terminal, 30));
        assert!(!draw_at(&mut template, &mut terminal, 100));
    }
}

#[cfg(test)]
mod parse_cache_tests {
    use super::*;

    #[test]
    fn drawing_and_events_reuse_root_ast_with_live_context() {
        let mut template = Template::from_source(
            "if enabled\n  button #first focusable\nelse\n  button #second focusable",
        );
        template.set("enabled", true);
        let nodes = template.parse_nodes().unwrap();
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| template.draw_full(frame).unwrap())
            .unwrap();
        template.handle_tui_event(&Event::FocusGained);
        assert_eq!(template.focus().focusable_ids, ["first"]);
        template.set("enabled", false);
        template.handle_tui_event(&Event::FocusGained);
        assert_eq!(template.focus().focusable_ids, ["second"]);
        assert!(Arc::ptr_eq(&nodes, &template.parse_nodes().unwrap()));
    }

    #[test]
    fn source_replacement_detaches_a_cloned_templates_ast() {
        let mut template = Template::from_source("button #old focusable");
        let old_nodes = template.parse_nodes().unwrap();
        let cloned = template.clone();
        assert!(Arc::ptr_eq(&old_nodes, &cloned.parse_nodes().unwrap()));
        template.set_source("button #new focusable");
        assert_eq!(template.focus().focusable_ids, ["new"]);
        assert!(!Arc::ptr_eq(&old_nodes, &template.parse_nodes().unwrap()));
        assert!(Arc::ptr_eq(&old_nodes, &cloned.parse_nodes().unwrap()));
        template.set_source("<div>");
        assert!(template.parse_nodes().is_err());
        assert!(template.parsed_nodes.get().is_none());
        template.set_source("button #recovered focusable");
        assert_eq!(template.focus().focusable_ids, ["recovered"]);
    }
}
