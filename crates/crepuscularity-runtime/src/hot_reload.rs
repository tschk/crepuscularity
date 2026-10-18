//! Hot-reload GPUI view component: reads a template file, renders it, and
//! watches the logical entry directory and resolved entry target via `notify`.
//!
//! # What triggers a reload
//!
//! The watcher monitors those directories, reconciling symlink changes in the background.
//! A reload fires when:
//!
//! - the entry template itself is modified, created, or removed (covers
//!   editor atomic-save patterns that rename a temp file over the original),
//! - any other supported template file in those subtrees changes (so editing an
//!   `include`d component reloads the entry), or
//! - a `context.toml` next to the template is updated.
//!
//! Watcher setup, template/include reads, path resolution and parsing run on GPUI's background executor.
//! Only completed snapshots are published on the UI thread; reloads retain the previous snapshot.
//!
//! Renderer parse errors render an in-window red error block instead of
//! panicking, so an in-progress edit cannot crash the dev window.
//!
//! # Lifetime
//!
//! Each [`HotReloadState`] owns its `notify` watcher and a cancellation flag
//! shared with its background poll task. Dropping the entity signals watcher
//! cleanup without joining on the UI thread, and signals the poll task to
//! exit on its next tick — so reusing the same process to load a series of
//! templates does not pile up dead watchers and zombie polling tasks.
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gpui::{
    div, rems, rgb, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window,
};
use notify::Watcher;

use crate::renderer::{
    prepare_includes, render_nodes_prepared, with_gpui_target_in_place, PreparedIncludes,
};
use crepuscularity_core::ast::Node;
use crepuscularity_core::context::TemplateContext;

/// State model for the hot-reload view.
pub struct HotReloadState {
    pub path: PathBuf,
    /// Last completed snapshot; empty until the initial background load finishes.
    pub template: Result<Arc<Vec<Node>>, String>,
    pub context: TemplateContext,
    pub changed: Arc<Mutex<bool>>,

    prepared: Option<PreparedSnapshot>,

    /// Owning watcher handle; dropping it signals background cleanup.
    _watcher: Option<Box<dyn Watcher + Send>>,
    watcher_path: Option<PathBuf>,

    /// Set to `true` in [`Drop`] so the spawned poll task exits on its next
    /// tick instead of running forever after the entity is gone.
    cancel: Arc<AtomicBool>,
}

struct PreparedSnapshot {
    nodes: Arc<Vec<Node>>,
    environment: TemplateContext,
    includes: Arc<PreparedIncludes>,
}

// Include paths and captured slots do not depend on ordinary variable values.
fn include_environment(context: &TemplateContext) -> TemplateContext {
    TemplateContext {
        vars: Default::default(),
        base_dir: context.base_dir.clone(),
        virtual_files: Arc::clone(&context.virtual_files),
        slot: context.slot.clone(),
    }
}

fn same_environment(left: &TemplateContext, right: &TemplateContext) -> bool {
    left.base_dir == right.base_dir
        && Arc::ptr_eq(&left.virtual_files, &right.virtual_files)
        && match (&left.slot, &right.slot) {
            (None, None) => true,
            (Some((left, left_context)), Some((right, right_context))) => {
                left == right && Arc::ptr_eq(left_context, right_context)
            }
            _ => false,
        }
}

fn same_template(
    left: &Result<Arc<Vec<Node>>, String>,
    right: &Result<Arc<Vec<Node>>, String>,
) -> bool {
    match (left, right) {
        (Ok(left), Ok(right)) => Arc::ptr_eq(left, right),
        (Err(left), Err(right)) => left == right,
        _ => false,
    }
}

impl PreparedSnapshot {
    fn matches(&self, nodes: &Arc<Vec<Node>>, context: &TemplateContext) -> bool {
        Arc::ptr_eq(&self.nodes, nodes) && same_environment(&self.environment, context)
    }

    fn render(&self, current: &TemplateContext) -> gpui::AnyElement {
        render_nodes_prepared(
            &self.nodes,
            &self.render_context(current),
            Arc::clone(&self.includes),
        )
    }

    fn render_context<'a>(&self, current: &'a TemplateContext) -> Cow<'a, TemplateContext> {
        if same_environment(&self.environment, current) {
            return Cow::Borrowed(current);
        }
        // Retain the previous include graph until preparation catches up, while
        // keeping caller-managed variables live. Captured slot scopes stay owned.
        let mut context = current.clone();
        context.base_dir = self.environment.base_dir.clone();
        context.virtual_files = Arc::clone(&self.environment.virtual_files);
        context.slot = self.environment.slot.clone();
        Cow::Owned(context)
    }
}

struct LoadRequest {
    path: PathBuf,
    template: Result<Arc<Vec<Node>>, String>,
    environment: TemplateContext,
    reload_entry: bool,
    needs_watcher: bool,
}

impl HotReloadState {
    pub fn new(path: PathBuf, context: TemplateContext, cx: &mut Context<Self>) -> Self {
        Self::new_inner(path, context, None, cx)
    }

    /// Reload an owned context alongside the template on the background worker.
    ///
    /// The loader replaces the entire context on each load. Apply any caller
    /// overrides inside the loader; use [`Self::new`] for caller-managed context.
    pub fn new_with_context_loader(
        path: PathBuf,
        loader: fn(&std::path::Path) -> TemplateContext,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_inner(path, TemplateContext::new(), Some(loader), cx)
    }

    fn new_inner(
        path: PathBuf,
        mut context: TemplateContext,
        loader: Option<fn(&std::path::Path) -> TemplateContext>,
        cx: &mut Context<Self>,
    ) -> Self {
        if context.base_dir.is_none() {
            context.base_dir = path.parent().map(|p| p.to_path_buf());
        }

        // Keep construction responsive; the first poll loads on a worker.
        let template = Ok(Arc::new(Vec::new()));
        let changed = Arc::new(Mutex::new(true));

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_task = Arc::clone(&cancel);
        let changed_poll = Arc::clone(&changed);

        cx.spawn(
            async move |this: WeakEntity<HotReloadState>, cx: &mut AsyncApp| loop {
                if cancel_for_task.load(Ordering::Relaxed) {
                    break;
                }

                let is_changed = changed_poll
                    .lock()
                    .map(|mut g| {
                        let v = *g;
                        *g = false;
                        v
                    })
                    .unwrap_or(false);

                let Ok(request) = this.update(cx, |state, _| {
                    let needs_preparation = state.template.as_ref().is_ok_and(|nodes| {
                        !state
                            .prepared
                            .as_ref()
                            .is_some_and(|snapshot| snapshot.matches(nodes, &state.context))
                    });
                    (is_changed || needs_preparation).then(|| LoadRequest {
                        path: state.path.clone(),
                        template: state.template.clone(),
                        environment: include_environment(&state.context),
                        reload_entry: is_changed,
                        needs_watcher: state._watcher.is_none()
                            || state.watcher_path.as_ref() != Some(&state.path),
                    })
                }) else {
                    break;
                };
                if let Some(request) = request {
                    let watcher_changed = Arc::clone(&changed_poll);
                    let load_cancel = Arc::clone(&cancel_for_task);
                    let (request, template, context, prepared, watcher) =
                        cx.background_executor()
                            .spawn(async move {
                                let watcher = if request.needs_watcher
                                    && !load_cancel.load(Ordering::Relaxed)
                                {
                                    match crepuscularity_core::watch::create_watcher(
                                        request.path.clone(),
                                        watcher_changed,
                                        "runtime",
                                    ) {
                                        Ok(watcher) => Some(watcher),
                                        Err(error) => {
                                            eprintln!(
                                            "[crepuscularity-runtime] hot reload disabled: {error}"
                                        );
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };
                                let template = if request.reload_entry {
                                    load_template(&request.path)
                                } else {
                                    request.template.clone()
                                };
                                let context = request.reload_entry.then_some(loader).flatten().map(
                                    |loader| {
                                        let mut context = loader(&request.path);
                                        if context.base_dir.is_none() {
                                            context.base_dir =
                                                request.path.parent().map(|p| p.to_path_buf());
                                        }
                                        context
                                    },
                                );
                                let preparation_context =
                                    context.as_ref().unwrap_or(&request.environment);
                                let prepared =
                                    template.as_ref().ok().map(|nodes| PreparedSnapshot {
                                        nodes: Arc::clone(nodes),
                                        environment: include_environment(preparation_context),
                                        includes: prepare_includes(nodes, preparation_context),
                                    });
                                (request, template, context, prepared, watcher)
                            })
                            .await;
                    if cancel_for_task.load(Ordering::Relaxed) {
                        break;
                    }
                    let published = this.update(cx, |state, cx| {
                        if state.path != request.path {
                            state.reload();
                            return true;
                        }
                        // Retain watcher setup even when newer edits invalidate this load.
                        if request.needs_watcher {
                            state._watcher = watcher;
                            state.watcher_path = Some(request.path);
                        }
                        if state
                            .changed
                            .lock()
                            .map(|changed| *changed)
                            .unwrap_or(false)
                        {
                            return true;
                        }
                        if !same_template(&state.template, &request.template) {
                            return true;
                        }
                        if !same_environment(&state.context, &request.environment) {
                            if request.reload_entry {
                                state.reload();
                            }
                            return true;
                        }
                        state.template = template;
                        if let Some(context) = context {
                            state.context = context;
                        }
                        state.prepared = prepared;
                        cx.notify();
                        false
                    });
                    match published {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(_) => break,
                    }
                }

                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
            },
        )
        .detach();

        Self {
            path,
            template,
            context,
            changed,
            prepared: None,
            _watcher: None,
            watcher_path: None,
            cancel,
        }
    }

    /// Queue a background reload, retaining the current snapshot until it completes.
    pub fn reload(&mut self) {
        if let Ok(mut changed) = self.changed.lock() {
            *changed = true;
        }
    }
}

impl Drop for HotReloadState {
    fn drop(&mut self) {
        // Watcher teardown is signaled separately when its handle is dropped.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn load_template(path: &std::path::Path) -> Result<Arc<Vec<Node>>, String> {
    // Explicit reloads must see edits even when file metadata is unchanged.
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    crepuscularity_core::ast_cache::parse_content_with_path(&content, Some(path))
        .map_err(|e| e.to_string())
}

/// A GPUI view that renders a hot-reloaded template.
pub struct HotReloadView {
    pub state: Entity<HotReloadState>,
}

impl HotReloadView {
    pub fn new(state: Entity<HotReloadState>) -> Self {
        Self { state }
    }
}

impl HotReloadState {
    fn render(&mut self) -> gpui::AnyElement {
        match &self.template {
            Ok(_) => match &self.prepared {
                Some(snapshot) => {
                    with_gpui_target_in_place(&mut self.context, |context| snapshot.render(context))
                }
                None => div().into_any_element(),
            },
            Err(err) => {
                let msg = format!("Parse error:\n\n{}", err);
                div()
                    .w_full()
                    .h_full()
                    .bg(rgb(0x1a0000))
                    .p(rems(2.))
                    .flex()
                    .flex_col()
                    .gap(rems(1.))
                    .child(
                        div()
                            .text_color(rgb(0xff4444))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(rems(1.125))
                            .child("Template Error"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xff8888))
                            .text_size(rems(0.875))
                            .child(msg),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for HotReloadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Return the entity lease before propagating a renderer panic.
        self.state
            .update(cx, |state, _| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.render()))
            })
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;

    fn snapshot_id(
        state: &Entity<HotReloadState>,
        cx: &mut gpui::TestAppContext,
    ) -> Option<gpui::ElementId> {
        state.read_with(cx, |state, _| {
            let mut element = state.prepared.as_ref().unwrap().render(&state.context);
            element
                .downcast_mut::<gpui::Stateful<gpui::Div>>()
                .and_then(|element| gpui::Element::id(element))
        })
    }

    fn named_id(name: &'static str) -> Option<gpui::ElementId> {
        Some(gpui::ElementId::Name(name.into()))
    }

    fn poll(cx: &mut gpui::TestAppContext) {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn rendering_restores_target_defaults_and_observes_caller_overrides(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "if is_gui\n  div #gui\nelse\n  div #custom").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path, TemplateContext::new(), cx));
        cx.run_until_parked();
        for (override_value, expected) in [(None, "gui"), (Some(false), "custom")] {
            state.update(cx, |state, _| {
                if let Some(value) = override_value {
                    state.context.set("is_gui", value);
                }
                let mut element = state.render();
                let id = element
                    .downcast_mut::<gpui::Stateful<gpui::Div>>()
                    .and_then(|element| gpui::Element::id(element));
                assert_eq!(id, named_id(expected));
                assert!(state.context.get("crepus_target").is_none());
                assert_eq!(
                    state.context.get("is_gui").is_some(),
                    override_value.is_some()
                );
            });
        }
    }

    #[gpui::test]
    fn includes_load_on_the_worker_and_remain_a_snapshot_until_reload(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        let child = dir.path().join("child.crepus");
        std::fs::write(&path, "include child.crepus").unwrap();
        std::fs::write(&child, "div #initial").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path, TemplateContext::new(), cx));
        assert!(state.read_with(cx, |state, _| state.prepared.is_none()));
        std::fs::write(&child, "div #worker").unwrap();
        cx.run_until_parked();
        assert_eq!(snapshot_id(&state, cx), named_id("worker"));
        std::fs::remove_file(&child).unwrap();
        assert_eq!(snapshot_id(&state, cx), named_id("worker"));
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("outside.crepus"), "div #outside").unwrap();
            std::os::unix::fs::symlink(outside.path().join("outside.crepus"), &child).unwrap();
            // Re-resolving would reject this escape; the prepared renderer uses only its snapshot.
            assert_eq!(snapshot_id(&state, cx), named_id("worker"));
            std::fs::remove_file(&child).unwrap();
        }
        std::fs::write(&child, "div #reloaded").unwrap();
        state.update(cx, |state, _| state.reload());
        assert_eq!(snapshot_id(&state, cx), named_id("worker"));
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("reloaded"));
    }

    #[gpui::test]
    fn variable_edits_use_the_current_scope_without_preparing_again(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "include child.crepus label={name}").unwrap();
        std::fs::write(dir.path().join("child.crepus"), "div id={label}").unwrap();
        let mut context = TemplateContext::new();
        context.set("name", "first");
        let state = cx.new(|cx| HotReloadState::new(path, context, cx));
        cx.run_until_parked();
        let includes = state.read_with(cx, |state, _| {
            Arc::clone(&state.prepared.as_ref().unwrap().includes)
        });
        assert_eq!(snapshot_id(&state, cx), named_id("first"));
        state.update(cx, |state, _| {
            state.context.set("name", "second");
        });
        assert_eq!(snapshot_id(&state, cx), named_id("second"));
        poll(cx);
        assert!(state.read_with(cx, |state, _| Arc::ptr_eq(
            &includes,
            &state.prepared.as_ref().unwrap().includes
        )));
    }

    static STRUCTURAL_LOADER_CALLS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    fn counted_context(entry: &std::path::Path) -> TemplateContext {
        STRUCTURAL_LOADER_CALLS.fetch_add(1, Ordering::Relaxed);
        read_test_context(entry)
    }

    #[gpui::test]
    fn structural_refresh_preserves_manual_root_and_skips_the_context_loader(
        cx: &mut gpui::TestAppContext,
    ) {
        STRUCTURAL_LOADER_CALLS.store(0, Ordering::Relaxed);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "div #entry").unwrap();
        let state = cx.new(|cx| HotReloadState::new_with_context_loader(path, counted_context, cx));
        cx.run_until_parked();
        let manual =
            Arc::new(crepuscularity_core::parse_template("include virtual.crepus").unwrap());
        state.update(cx, |state, _| {
            state.template = Ok(Arc::clone(&manual));
            Arc::make_mut(&mut state.context.virtual_files)
                .insert("virtual.crepus".into(), "div #virtual_one".into());
            state.context.set("name", "caller");
        });
        assert_eq!(snapshot_id(&state, cx), named_id("entry"));
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("virtual_one"));
        state.update(cx, |state, _| {
            Arc::make_mut(&mut state.context.virtual_files)
                .insert("virtual.crepus".into(), "div #virtual_two".into());
        });
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("virtual_two"));
        assert_eq!(STRUCTURAL_LOADER_CALLS.load(Ordering::Relaxed), 1);
        assert!(state.read_with(cx, |state, _| Arc::ptr_eq(
            state.template.as_ref().unwrap(),
            &manual
        ) && state.context.get_str("name")
            == "caller"));
    }

    #[gpui::test]
    fn base_and_in_place_slot_changes_prepare_without_explicit_reload(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "include child.crepus").unwrap();
        std::fs::write(dir.path().join("child.crepus"), "div #one").unwrap();
        std::fs::write(other.path().join("child.crepus"), "div #two").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path, TemplateContext::new(), cx));
        cx.run_until_parked();
        state.update(cx, |state, _| {
            state.context.base_dir = Some(other.path().into())
        });
        assert_eq!(snapshot_id(&state, cx), named_id("one"));
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("two"));
        let mut slot_context = TemplateContext::new();
        Arc::make_mut(&mut slot_context.virtual_files).extend([
            ("foo.crepus".into(), "div #slot_one".into()),
            ("bar.crepus".into(), "div #slot_two".into()),
        ]);
        state.update(cx, |state, _| {
            state.template = Ok(Arc::new(
                crepuscularity_core::parse_template("slot").unwrap(),
            ));
            state.context.slot = Some((
                crepuscularity_core::parse_template("include foo.crepus").unwrap(),
                Arc::new(slot_context),
            ));
        });
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("slot_one"));
        state.update(cx, |state, _| {
            let (nodes, _) = state.context.slot.as_mut().unwrap();
            let Node::Include(include) = &mut nodes[0] else {
                panic!("include")
            };
            include.path = "bar.crepus".into();
        });
        assert_eq!(snapshot_id(&state, cx), named_id("slot_one"));
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("slot_two"));
    }

    #[gpui::test]
    fn manual_template_error_recovers_without_reloading_the_entry(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "div #entry").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path, TemplateContext::new(), cx));
        cx.run_until_parked();
        state.update(cx, |state, _| state.template = Err("manual error".into()));
        poll(cx);
        assert!(state.read_with(cx, |state, _| state.template.is_err()));
        state.update(cx, |state, _| {
            state.template = Ok(Arc::new(
                crepuscularity_core::parse_template("div #manual").unwrap(),
            ))
        });
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("manual"));
    }

    #[gpui::test]
    fn included_file_notifications_refresh_the_prepared_snapshot(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        let child = dir.path().join("child.crepus");
        std::fs::write(&path, "include child.crepus").unwrap();
        std::fs::write(&child, "div #one").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path, TemplateContext::new(), cx));
        cx.run_until_parked();
        let changed = state.read_with(cx, |state, _| Arc::clone(&state.changed));
        *changed.lock().unwrap() = false;
        std::fs::write(&child, "div #two").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !*changed.lock().unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "include watcher notification missing"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(snapshot_id(&state, cx), named_id("one"));
        poll(cx);
        assert_eq!(snapshot_id(&state, cx), named_id("two"));
    }

    static STALE_LOAD_HOOK: std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>> =
        std::sync::Mutex::new(None);
    fn editing_context(entry: &std::path::Path) -> TemplateContext {
        let context = read_test_context(entry);
        let hook = STALE_LOAD_HOOK.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        context
    }

    #[gpui::test]
    fn edits_during_worker_loading_cannot_publish_mixed_root_context_and_includes(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "include first.crepus").unwrap();
        std::fs::write(dir.path().join("first.crepus"), "div #one").unwrap();
        std::fs::write(dir.path().join("second.crepus"), "div #two").unwrap();
        std::fs::write(dir.path().join("context.toml"), "one").unwrap();
        let state =
            cx.new(|cx| HotReloadState::new_with_context_loader(path.clone(), editing_context, cx));
        let changed = state.read_with(cx, |state, _| Arc::clone(&state.changed));
        *STALE_LOAD_HOOK.lock().unwrap() = Some(Box::new(move || {
            std::fs::write(&path, "include second.crepus").unwrap();
            std::fs::write(path.with_file_name("context.toml"), "two").unwrap();
            *changed.lock().unwrap() = true;
        }));
        cx.run_until_parked();
        assert_eq!(snapshot_id(&state, cx), named_id("two"));
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("name")),
            "two"
        );
    }

    #[test]
    fn reload_reads_same_length_edits_with_preserved_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "span \"old\"").unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let old = load_template(&path).unwrap();
        assert!(Arc::ptr_eq(&old, &load_template(&path).unwrap()));

        for replace in [false, true] {
            let updated = if replace {
                "span \"end\""
            } else {
                "span \"new\""
            };
            let write_path = if replace {
                dir.path().join("replacement.crepus")
            } else {
                path.clone()
            };
            std::fs::write(&write_path, updated).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&write_path)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(modified))
                .unwrap();
            if replace {
                std::fs::rename(&write_path, &path).unwrap();
            }
            let current = load_template(&path).unwrap();
            assert!(!Arc::ptr_eq(&old, &current));
            assert_eq!(
                *current,
                crepuscularity_core::parser::parse_template_with_path(updated, Some(&path))
                    .unwrap()
            );
            assert!(Arc::ptr_eq(&current, &load_template(&path).unwrap()));
        }
    }

    fn read_test_context(entry: &std::path::Path) -> TemplateContext {
        let mut context = TemplateContext::new();
        if let Ok(name) = std::fs::read_to_string(entry.with_file_name("context.toml")) {
            context.set("name", name);
        }
        context
    }

    #[gpui::test]
    fn context_loader_is_deferred_and_replaces_context_with_the_template(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        let context_path = dir.path().join("context.toml");
        std::fs::write(&path, "div").unwrap();
        std::fs::write(&context_path, "startup").unwrap();
        let state = cx
            .new(|cx| HotReloadState::new_with_context_loader(path.clone(), read_test_context, cx));
        assert!(state.read_with(cx, |state, _| state.context.vars.is_empty()));
        std::fs::write(&context_path, "loaded").unwrap();
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("name")),
            "loaded"
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.context.base_dir.clone()),
            Some(dir.path().to_path_buf())
        );

        std::fs::write(&path, "nav").unwrap();
        std::fs::write(&context_path, "updated").unwrap();
        state.update(cx, |state, _| state.reload());
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("name")),
            "loaded"
        );
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(state.read_with(cx, |state, _| {
            state.context.get_str("name") == "updated" && matches!(
                &state.template.as_ref().unwrap()[0], Node::Element(element) if element.tag == "nav"
            )
        }));

        std::fs::remove_file(context_path).unwrap();
        state.update(cx, |state, _| state.reload());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(state.read_with(cx, |state, _| !state.context.vars.contains_key("name")));
    }

    #[gpui::test]
    fn caller_managed_context_survives_reloads(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "div").unwrap();
        std::fs::write(dir.path().join("context.toml"), "file value").unwrap();
        let mut context = TemplateContext::new();
        context.set("name", "caller");
        context.base_dir = Some(dir.path().join("explicit"));
        let state = cx.new(|cx| HotReloadState::new(path.clone(), context, cx));
        cx.run_until_parked();
        state.update(cx, |state, _| {
            state.context.set("name", "override");
            state.reload();
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("name")),
            "override"
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.context.base_dir.clone()),
            Some(dir.path().join("explicit"))
        );
    }

    #[gpui::test]
    fn watcher_is_reused_until_the_entry_path_changes(cx: &mut gpui::TestAppContext) {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let first = one.path().join("view.crepus");
        let second = two.path().join("view.crepus");
        std::fs::write(&first, "div").unwrap();
        std::fs::write(&second, "section").unwrap();
        let state = cx.new(|cx| {
            HotReloadState::new_with_context_loader(first.clone(), read_test_context, cx)
        });
        cx.run_until_parked();
        let address = state.read_with(cx, |state, _| {
            state._watcher.as_deref().unwrap() as *const dyn Watcher as *const ()
        });
        state.update(cx, |state, _| state.reload());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(
            state.read_with(
                cx,
                |state, _| state._watcher.as_deref().unwrap() as *const dyn Watcher as *const ()
            ),
            address
        );
        state.update(cx, |state, _| {
            state.path = second.clone();
            state.reload();
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.watcher_path.clone()),
            Some(second)
        );
        assert_ne!(
            state.read_with(
                cx,
                |state, _| state._watcher.as_deref().unwrap() as *const dyn Watcher as *const ()
            ),
            address
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.context.base_dir.clone()),
            Some(two.path().to_path_buf())
        );
        assert!(state.read_with(cx, |state, _| matches!(&state.template.as_ref().unwrap()[0], Node::Element(element) if element.tag == "section")));
    }

    #[gpui::test]
    fn failed_watcher_setup_retries_on_reload(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing/view.crepus");
        let state = cx.new(|cx| HotReloadState::new(path.clone(), TemplateContext::new(), cx));
        cx.run_until_parked();
        assert!(state.read_with(cx, |state, _| state._watcher.is_none()
            && state.template.is_err()));
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "div").unwrap();
        state.update(cx, |state, _| state.reload());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(state.read_with(cx, |state, _| state._watcher.is_some()
            && state.template.is_ok()));
    }

    #[gpui::test]
    fn dropping_before_initial_poll_releases_pending_setup(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let state = cx.new(|cx| {
            HotReloadState::new(dir.path().join("view.crepus"), TemplateContext::new(), cx)
        });
        let (cancel, changed) = state.read_with(cx, |state, _| {
            (Arc::clone(&state.cancel), Arc::downgrade(&state.changed))
        });
        drop(state);
        cx.update(|_| {});
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(changed.upgrade().is_none());
    }

    #[gpui::test]
    fn initial_load_is_deferred_until_the_worker_runs(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        let state = cx.new(|cx| HotReloadState::new(path.clone(), TemplateContext::new(), cx));
        assert!(state.read_with(cx, |state, _| state.template.as_ref().unwrap().is_empty()));
        assert!(state.read_with(cx, |state, _| state._watcher.is_none()));
        std::fs::write(&path, "div\n  text \"loaded\"").unwrap();
        cx.run_until_parked();
        assert!(!state.read_with(cx, |state, _| state.template.as_ref().unwrap().is_empty()));
    }

    #[gpui::test]
    fn reload_queues_work_and_keeps_the_last_snapshot(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "div").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path.clone(), TemplateContext::new(), cx));
        cx.run_until_parked();
        let previous = state.read_with(cx, |state, _| Arc::clone(state.template.as_ref().unwrap()));
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, "nav").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        state.update(cx, |state, _| state.reload());
        assert!(state.read_with(cx, |state, _| Arc::ptr_eq(
            &previous,
            state.template.as_ref().unwrap()
        )));
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(!state.read_with(cx, |state, _| Arc::ptr_eq(
            &previous,
            state.template.as_ref().unwrap()
        )));
        assert!(state.read_with(cx, |state, _| matches!(
            &state.template.as_ref().unwrap()[0],
            Node::Element(element) if element.tag == "nav"
        )));
    }
}
