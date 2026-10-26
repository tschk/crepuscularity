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

use crate::renderer::{prepare_includes, render_nodes_prepared, PreparedIncludes};
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
    #[cfg(test)]
    preparation_gate: Option<Arc<PreparationGate>>,

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
    #[cfg(test)]
    gate: Option<Arc<PreparationGate>>,
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
                        #[cfg(test)]
                        gate: state.preparation_gate.take(),
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
                                // Gate the worker's actual include-preparation boundary after
                                // root/context loading, never the UI dispatch path.
                                #[cfg(test)]
                                if let Some(gate) = &request.gate {
                                    gate.wait().await;
                                }
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
            #[cfg(test)]
            preparation_gate: None,
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
                Some(snapshot) => snapshot.render(&self.context),
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

// An asynchronous gate models slow worker preparation without blocking GPUI's
// deterministic test executor or adding sleeps/OS scheduling assumptions.
#[cfg(test)]
#[derive(Default)]
struct PreparationGate {
    entered: AtomicBool,
    released: AtomicBool,
    waker: Mutex<Option<std::task::Waker>>,
}
#[cfg(test)]
impl PreparationGate {
    async fn wait(&self) {
        std::future::poll_fn(|cx| {
            self.entered.store(true, Ordering::SeqCst);
            let mut waker = self.waker.lock().unwrap();
            if self.released.load(Ordering::SeqCst) {
                std::task::Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        })
        .await
    }
    fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;

    fn poll(cx: &mut gpui::TestAppContext) {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
    }
    fn tag(state: &Entity<HotReloadState>, cx: &mut gpui::TestAppContext) -> String {
        state.read_with(cx, |state, _| {
            match &state.prepared.as_ref().unwrap().nodes[0] {
                Node::Element(el) => el.tag.clone(),
                _ => panic!("expected element"),
            }
        })
    }
    #[gpui::test]
    fn gated_worker_keeps_ui_responsive_retains_snapshot_rejects_stale_and_disposes(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "div").unwrap();
        let state = cx.new(|cx| HotReloadState::new(path.clone(), TemplateContext::new(), cx));
        cx.run_until_parked();
        let previous = state.read_with(cx, |state, _| {
            Arc::clone(&state.prepared.as_ref().unwrap().nodes)
        });
        let gate = Arc::new(PreparationGate::default());
        std::fs::write(&path, "nav").unwrap();
        state.update(cx, |state, _| {
            state.preparation_gate = Some(Arc::clone(&gate));
            state.reload();
        });
        poll(cx);
        assert!(gate.entered.load(Ordering::SeqCst));
        // A UI callback completes while the worker remains pending. Rendering
        // uses the previous prepared graph, without falling back to file IO.
        state.update(cx, |state, _| {
            assert!(!gate.released.load(Ordering::SeqCst));
            assert!(Arc::ptr_eq(
                &previous,
                &state.prepared.as_ref().unwrap().nodes
            ));
            let _ = state.render();
            state.context.set("ui_callback", "completed");
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("ui_callback")),
            "completed"
        );
        std::fs::write(&path, "main").unwrap();
        state.update(cx, |state, _| state.reload());
        gate.release();
        cx.run_until_parked();
        assert_eq!(tag(&state, cx), "main");

        let gate = Arc::new(PreparationGate::default());
        let cancel = state.read_with(cx, |state, _| Arc::clone(&state.cancel));
        let weak = state.downgrade();
        state.update(cx, |state, _| {
            state.preparation_gate = Some(Arc::clone(&gate));
            state.reload();
        });
        poll(cx);
        assert!(gate.entered.load(Ordering::SeqCst));
        drop(state);
        cx.update(|_| {});
        assert!(cancel.load(Ordering::Relaxed));
        gate.release();
        cx.run_until_parked();
        assert!(weak.upgrade().is_none());
    }

    #[gpui::test]
    fn environment_changes_keep_old_graph_until_prepared_and_errors_recover(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "include child.crepus").unwrap();
        let mut context = TemplateContext::new();
        Arc::make_mut(&mut context.virtual_files).insert("child.crepus".into(), "div".into());
        let state = cx.new(|cx| HotReloadState::new(path.clone(), context, cx));
        cx.run_until_parked();
        let previous = state.read_with(cx, |state, _| {
            Arc::clone(&state.prepared.as_ref().unwrap().includes)
        });
        state.update(cx, |state, _| {
            Arc::make_mut(&mut state.context.virtual_files)
                .insert("child.crepus".into(), "nav".into());
            let snapshot = state.prepared.as_ref().unwrap();
            let retained = snapshot.render_context(&state.context);
            assert!(same_environment(&snapshot.environment, &retained));
            assert!(!same_environment(&snapshot.environment, &state.context));
            let _ = state.render();
        });
        poll(cx);
        assert!(!state.read_with(cx, |state, _| Arc::ptr_eq(
            &previous,
            &state.prepared.as_ref().unwrap().includes
        )));
        std::fs::remove_file(&path).unwrap();
        state.update(cx, |state, _| state.reload());
        poll(cx);
        assert!(state.read_with(cx, |state, _| state.template.is_err()));
        std::fs::write(&path, "main").unwrap();
        state.update(cx, |state, _| state.reload());
        poll(cx);
        assert_eq!(tag(&state, cx), "main");
    }

    #[gpui::test]
    fn initial_load_is_deferred_and_context_loader_replaces_snapshot(
        cx: &mut gpui::TestAppContext,
    ) {
        fn loader(path: &std::path::Path) -> TemplateContext {
            let mut ctx = TemplateContext::new();
            ctx.set(
                "value",
                std::fs::read_to_string(path.with_extension("context")).unwrap(),
            );
            ctx
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("view.crepus");
        let state = cx.new(|cx| HotReloadState::new_with_context_loader(path.clone(), loader, cx));
        assert!(state.read_with(cx, |state, _| state.template.as_ref().unwrap().is_empty()));
        std::fs::write(&path, "div").unwrap();
        std::fs::write(path.with_extension("context"), "before").unwrap();
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("value")),
            "before"
        );
        std::fs::write(path.with_extension("context"), "after").unwrap();
        state.update(cx, |state, _| state.reload());
        poll(cx);
        assert_eq!(
            state.read_with(cx, |state, _| state.context.get_str("value")),
            "after"
        );
    }
}
