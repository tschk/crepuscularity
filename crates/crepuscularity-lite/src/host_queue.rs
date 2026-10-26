//! Deferred work that must run on the GPUI thread with a live window (e.g. native title APIs).

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum HostDeferred {
    SetWindowTitle(String),
    /// Content size in **logical pixels** (GPUI `px`).
    SetContentSize {
        width: f32,
        height: f32,
    },
    ToggleFullscreen,
    Minimize,
    Zoom,
    /// [`gpui::Window::show_character_palette`].
    ShowCharacterPalette,
    /// macOS “document edited” dot; maps to [`gpui::Window::set_window_edited`].
    SetWindowEdited(bool),
    /// [`gpui::Window::start_window_move`].
    StartWindowMove,
    /// [`gpui::Window::refresh`].
    RefreshWindow,
    /// [`gpui::Window::request_decorations`].
    RequestDecorations(DeferredWindowDecorations),
}

/// Decoration style for [`HostDeferred::RequestDecorations`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredWindowDecorations {
    Server,
    Client,
}

pub struct HostCommandQueue {
    inner: Mutex<Vec<HostDeferred>>,
}

impl HostCommandQueue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Vec::new()),
        })
    }

    pub fn push(&self, cmd: HostDeferred) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(cmd);
    }

    pub fn drain(&self) -> Vec<HostDeferred> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *g)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_host_command_queue_basic() {
        let queue = HostCommandQueue::new();
        assert_eq!(queue.drain().len(), 0);

        queue.push(HostDeferred::ToggleFullscreen);
        queue.push(HostDeferred::Minimize);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(matches!(drained[0], HostDeferred::ToggleFullscreen));
        assert!(matches!(drained[1], HostDeferred::Minimize));

        assert_eq!(queue.drain().len(), 0);
    }

    #[test]
    fn test_host_command_queue_concurrency() {
        let queue = HostCommandQueue::new();
        let queue_clone = Arc::clone(&queue);

        let producer = thread::spawn(move || {
            for i in 0..1000 {
                queue_clone.push(HostDeferred::SetWindowTitle(format!("Title {}", i)));
            }
        });

        let mut all_drained = Vec::new();
        while !producer.is_finished() {
            all_drained.extend(queue.drain());
            // Yield to encourage context switching and actual concurrent access
            thread::yield_now();
        }
        // Final drain after the producer thread is done
        all_drained.extend(queue.drain());

        producer.join().unwrap();

        assert_eq!(all_drained.len(), 1000);
        for (i, cmd) in all_drained.into_iter().enumerate() {
            if let HostDeferred::SetWindowTitle(title) = cmd {
                assert_eq!(title, format!("Title {}", i));
            } else {
                panic!("Unexpected command");
            }
        }
    }
}
