//! File-watch + reload helpers for `.crepus` templates rendered with Ratatui.
//!
//! [`HotTemplate`] wraps a [`Template`] with a `notify` watcher and a shared
//! "changed" flag. The watcher only flips a `bool`; the *application* decides
//! when to react — typically once per crossterm event-loop tick by calling
//! [`HotTemplate::poll_reload`] (or [`HotTemplate::poll_and_draw`]).
//!
//! This mirrors the GPUI [`HotReloadState`](https://docs.rs/crepuscularity-runtime)
//! design: cheap, lock-light, app-driven. Crepus does not own your terminal
//! event loop, so we do not push reload events at you — we just expose a
//! polled flag and a reload routine.
//!
//! # Lifetime
//!
//! Each [`HotTemplate`] owns its `notify` watcher; dropping the value tears
//! down the watcher's internal worker thread, so a process that opens a
//! sequence of templates does not pile up dead watchers.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use crepuscularity_tui::{HotTemplate, ReloadOutcome};
//! use ratatui::{backend::CrosstermBackend, Terminal};
//! use std::io;
//!
//! let mut hot = HotTemplate::watch("ui/ui.crepus").unwrap();
//! hot.template_mut().set("title", "My App");
//!
//! let backend = CrosstermBackend::new(io::stdout());
//! let mut terminal = Terminal::new(backend).unwrap();
//!
//! loop {
//!     terminal
//!         .draw(|frame| {
//!             let _ = hot.poll_and_draw_full(frame);
//!         })
//!         .unwrap();
//!     // ... read crossterm events, break on Q, etc.
//! #   break;
//! }
//! ```
//!
//! # Reload semantics
//!
//! - Source-only: only the template's [`source`](Template::source) is replaced.
//!   The [`TemplateContext`](crate::TemplateContext) (variables, `base_dir`,
//!   virtual files) is preserved across reloads, so the app's state survives a
//!   `.crepus` save.
//! - Atomic-ish: on read failure the previous source stays active and the
//!   outcome is [`ReloadOutcome::Error`] — the next draw still renders.
//! - Robust against atomic-save editors: we watch the *parent directory*
//!   rather than the single file, so Vim/Helix/JetBrains/VS Code-style
//!   "write to temp, rename over original" saves still trigger reload (a
//!   plain file watch would leave the kernel inotify watch on a deleted
//!   inode and never fire again). Sibling/descendant `.crepus` files in the
//!   same subtree also flip the change flag, so editing an `include`d
//!   component reloads the entry too.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crepuscularity_core::watch;
use notify::Watcher;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::Template;

/// Outcome of a single [`HotTemplate::poll_reload`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadOutcome {
    /// File hadn't changed — no work was done.
    Unchanged,
    /// File changed and the new source replaced the in-memory template.
    Reloaded,
    /// File changed but reading it failed; the previous source is still active.
    Error(String),
}

/// A [`Template`] backed by a file on disk, kept in sync with the file via a
/// `notify` watcher whose lifetime is tied to this struct.
pub struct HotTemplate {
    template: Template,
    changed: Arc<Mutex<bool>>,
    /// Owning handle to the `notify` watcher — dropped with `Self`, which
    /// stops the watcher's worker thread.
    _watcher: Box<dyn Watcher + Send>,
}

impl HotTemplate {
    /// Open `path`, load its source, and start a watcher rooted at the
    /// containing directory.
    ///
    /// Subsequent saves of the file flip an internal flag that is consumed by
    /// [`HotTemplate::poll_reload`]. Dropping the [`HotTemplate`] tears down
    /// the watcher cleanly.
    pub fn watch(path: impl AsRef<Path>) -> Result<Self, String> {
        let template = Template::from_path(path)?;
        let watch_path = template.path().to_path_buf();
        let changed = Arc::new(Mutex::new(false));
        let watcher = create_file_watcher(watch_path, Arc::clone(&changed))?;
        Ok(Self {
            template,
            changed,
            _watcher: watcher,
        })
    }

    /// Borrow the underlying [`Template`] (read-only).
    pub fn template(&self) -> &Template {
        &self.template
    }

    /// Borrow the underlying [`Template`] mutably — useful for setting context
    /// variables (`hot.template_mut().set("user", "alice")`).
    pub fn template_mut(&mut self) -> &mut Template {
        &mut self.template
    }

    /// Returns `true` if the file changed since the last [`HotTemplate::poll_reload`].
    /// Does *not* clear the flag — call [`HotTemplate::poll_reload`] to consume it.
    pub fn has_pending_change(&self) -> bool {
        self.changed.lock().map(|g| *g).unwrap_or(false)
    }

    /// If the watched file changed since the last poll, re-read it and update
    /// the in-memory template source.
    ///
    /// The change flag is consumed regardless of read success so the next poll
    /// returns [`ReloadOutcome::Unchanged`] until the next save.
    pub fn poll_reload(&mut self) -> ReloadOutcome {
        let was_changed = match self.changed.lock() {
            Ok(mut g) => {
                let v = *g;
                *g = false;
                v
            }
            Err(_) => false,
        };
        if !was_changed {
            return ReloadOutcome::Unchanged;
        }
        match self.template.reload() {
            Ok(()) => ReloadOutcome::Reloaded,
            Err(e) => ReloadOutcome::Error(e),
        }
    }

    /// Poll for a change *and* draw into `area` in one call.
    ///
    /// The returned [`ReloadOutcome`] is from the poll, not the draw — use it
    /// to show a "reloaded" toast or surface a parse error in your TUI.
    pub fn poll_and_draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) -> Result<ReloadOutcome, String> {
        let outcome = self.poll_reload();
        self.template.draw(frame, area)?;
        Ok(outcome)
    }

    /// Like [`HotTemplate::poll_and_draw`] but uses the full frame area.
    pub fn poll_and_draw_full(&mut self, frame: &mut Frame) -> Result<ReloadOutcome, String> {
        let area = frame.area();
        self.poll_and_draw(frame, area)
    }

    /// Forwarded for ergonomics: render without polling for changes.
    pub fn draw(&self, frame: &mut Frame, area: Rect) -> Result<(), String> {
        self.template.draw(frame, area)
    }

    /// Forwarded for ergonomics: render the full frame without polling for changes.
    pub fn draw_full(&self, frame: &mut Frame) -> Result<(), String> {
        self.template.draw_full(frame)
    }

    #[cfg(test)]
    pub(crate) fn changed_handle(&self) -> Arc<Mutex<bool>> {
        Arc::clone(&self.changed)
    }
}

/// Create a `notify` watcher — thin wrapper delegating to [`watch::create_watcher`].
fn create_file_watcher(
    path: PathBuf,
    changed: Arc<Mutex<bool>>,
) -> Result<Box<dyn Watcher + Send>, String> {
    watch::create_watcher(path, changed, "tui")
}
