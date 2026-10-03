//! Hot-reload GPUI view component: reads a template file, renders it, and
//! watches for changes via the parent-directory `notify` watcher.
//!
//! # What triggers a reload
//!
//! The watcher is rooted at the *directory* that contains the entry template.
//! A reload fires when:
//!
//! - the entry template itself is modified, created, or removed (covers
//!   editor atomic-save patterns that rename a temp file over the original),
//! - any other `.crepus` file in that subtree changes (so editing an
//!   `include`d component reloads the entry), or
//! - a `context.toml` next to the template is updated.
//!
//! Renderer parse errors render an in-window red error block instead of
//! panicking, so an in-progress edit cannot crash the dev window.
//!
//! # Lifetime
//!
//! Each [`HotReloadState`] owns its `notify` watcher and a cancellation flag
//! shared with its background poll task. Dropping the entity drops the
//! watcher (stopping `notify`'s worker thread) and signals the poll task to
//! exit on its next tick — so reusing the same process to load a series of
//! templates does not pile up dead watchers and zombie polling tasks.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gpui::{
    div, rems, rgb, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window,
};
use notify::Watcher;

use crate::{renderer::render_nodes_with_bindings, RuntimeBindings};
use crepuscularity_core::ast::Node;
use crepuscularity_core::context::TemplateContext;

/// State model for the hot-reload view.
pub struct HotReloadState {
    pub path: PathBuf,
    pub template: Result<Arc<Vec<Node>>, String>,
    pub context: TemplateContext,
    pub bindings: RuntimeBindings,
    pub changed: Arc<Mutex<bool>>,

    /// Owning handle to the `notify` watcher. Dropped with `Self`, which
    /// stops the watcher's internal worker thread.
    _watcher: Option<Box<dyn Watcher + Send>>,

    /// Set to `true` in [`Drop`] so the spawned poll task exits on its next
    /// tick instead of running forever after the entity is gone.
    cancel: Arc<AtomicBool>,
}

impl HotReloadState {
    pub fn new(path: PathBuf, mut context: TemplateContext, cx: &mut Context<Self>) -> Self {
        if context.base_dir.is_none() {
            context.base_dir = path.parent().map(|p| p.to_path_buf());
        }

        let template = load_template(&path);
        let changed = Arc::new(Mutex::new(false));

        // The watcher worker keeps running as long as we hold this Box.
        // Surface creation errors as a red template block so a misconfigured
        // path does not silently disable hot reload.
        let watcher = match crepuscularity_core::watch::create_watcher(
            path.clone(),
            Arc::clone(&changed),
            "runtime",
        ) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("[crepuscularity-runtime] hot reload disabled: {e}");
                None
            }
        };

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

                if is_changed
                    && this
                        .update(cx, |state, cx| {
                            state.template = load_template(&state.path);
                            cx.notify();
                        })
                        .is_err()
                {
                    // Entity has been dropped; stop polling.
                    break;
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
            bindings: RuntimeBindings::default(),
            changed,
            _watcher: watcher,
            cancel,
        }
    }

    pub fn reload(&mut self) {
        self.template = load_template(&self.path);
    }
}

impl Drop for HotReloadState {
    fn drop(&mut self) {
        // Stop the polling task ASAP. The watcher box is dropped with the
        // struct itself, which terminates `notify`'s worker thread.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn load_template(path: &std::path::Path) -> Result<Arc<Vec<Node>>, String> {
    crepuscularity_core::ast_cache::parse_file(path).map_err(|e| e.to_string())
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

impl Render for HotReloadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        match &state.template {
            Ok(nodes) => render_nodes_with_bindings(
                &format!("hot-template:{:?}", self.state.entity_id()),
                nodes.as_ref(),
                &state.context,
                &state.bindings,
            ),
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
