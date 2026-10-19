//! Event system and focus management for the Crepuscularity TUI backend.
//!
//! Provides a lightweight event enum (re-exporting crossterm's [`KeyEvent`] and
//! [`MouseEvent`]), a [`FocusManager`] that tracks the currently focused element
//! and cycles through focusable IDs in tab order, and an [`EventDispatcher`] that
//! routes events to the focused element's callback with support for event
//! bubbling to parent elements.

use std::collections::{HashMap, HashSet};

pub use crossterm::event::{KeyEvent, MouseEvent};

/// A terminal event, re-exporting crossterm's key and mouse types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    FocusGained,
    FocusLost,
}

impl From<crossterm::event::Event> for Event {
    fn from(ev: crossterm::event::Event) -> Self {
        match ev {
            crossterm::event::Event::Key(k) => Event::Key(k),
            crossterm::event::Event::Mouse(m) => Event::Mouse(m),
            crossterm::event::Event::Resize(w, h) => Event::Resize(w, h),
            crossterm::event::Event::FocusGained => Event::FocusGained,
            crossterm::event::Event::FocusLost => Event::FocusLost,
            _ => Event::FocusLost,
        }
    }
}

/// Tracks which element currently has focus and the ordered list of focusable
/// element IDs (tab order).
#[derive(Debug, Clone, Default)]
pub struct FocusManager {
    /// The currently focused element ID, if any.
    pub focused_id: Option<String>,
    /// Focusable element IDs in tab order.
    pub focusable_ids: Vec<String>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the focusable IDs, keeping the first occurrence of each ID.
    /// Preserve existing focus when present; otherwise select the first ID,
    /// or clear focus when the list is empty.
    pub fn set_focusable_ids(&mut self, ids: Vec<String>) {
        let mut seen = HashSet::new();
        self.focusable_ids = ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        if let Some(ref focused) = self.focused_id {
            if !self.focusable_ids.iter().any(|id| id == focused) {
                self.focused_id = None;
            }
        }
        if self.focused_id.is_none() && !self.focusable_ids.is_empty() {
            self.focused_id = Some(self.focusable_ids[0].clone());
        }
    }

    /// Move focus to the next focusable element (wraps around).
    /// Returns the newly focused ID, if any.
    pub fn focus_next(&mut self) -> Option<&str> {
        if self.focusable_ids.is_empty() {
            self.focused_id = None;
            return None;
        }
        let idx = self
            .focused_id
            .as_ref()
            .and_then(|id| self.focusable_ids.iter().position(|x| x == id))
            .map(|i| (i + 1) % self.focusable_ids.len())
            .unwrap_or(0);
        self.focused_id = Some(self.focusable_ids[idx].clone());
        self.focused_id.as_deref()
    }

    /// Move focus to the previous focusable element (wraps around).
    /// Returns the newly focused ID, if any.
    pub fn focus_prev(&mut self) -> Option<&str> {
        if self.focusable_ids.is_empty() {
            self.focused_id = None;
            return None;
        }
        let len = self.focusable_ids.len();
        let idx = self
            .focused_id
            .as_ref()
            .and_then(|id| self.focusable_ids.iter().position(|x| x == id))
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(0);
        self.focused_id = Some(self.focusable_ids[idx].clone());
        self.focused_id.as_deref()
    }

    /// Focus the element with the given ID. Returns `true` if the ID is in the
    /// focusable list.
    pub fn focus(&mut self, id: &str) -> bool {
        if self.focusable_ids.iter().any(|x| x == id) {
            self.focused_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Returns `true` if the given ID is the currently focused element.
    pub fn is_focused(&self, id: &str) -> bool {
        self.focused_id.as_deref() == Some(id)
    }
}

/// The result of dispatching an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    /// A callback handled the event.
    Handled,
    /// No callback handled the event.
    NotHandled,
}

/// Routes events to element callbacks, with support for event bubbling.
///
/// Each element ID may register a callback that receives an [`Event`] and
/// returns `true` if it handled the event. When [`EventDispatcher::dispatch`]
/// is called, the event is first sent to the focused element's callback; if
/// that callback is absent or returns `false`, the event bubbles up to the
/// element's parent (if registered via [`EventDispatcher::set_parent`]) and so
/// on until a callback handles it or the root is reached.
pub struct EventDispatcher {
    callbacks: HashMap<String, EventCallback>,
    parents: HashMap<String, String>,
}

type EventCallback = Box<dyn Fn(&Event) -> bool + Send + Sync>;

impl std::fmt::Debug for EventDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDispatcher")
            .field("callback_count", &self.callbacks.len())
            .field("parents", &self.parents)
            .finish()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    /// Register a callback for an element ID. The callback receives an
    /// [`Event`] and returns `true` if it handled the event.
    pub fn on<F>(&mut self, id: &str, callback: F)
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        self.callbacks.insert(id.to_string(), Box::new(callback));
    }

    /// Register a parent relationship for event bubbling.
    pub fn set_parent(&mut self, child: &str, parent: &str) {
        self.parents.insert(child.to_string(), parent.to_string());
    }

    /// Dispatch an event to the focused element's callback, bubbling to
    /// parents if not handled. Each element is visited at most once, even if
    /// the registered parent relationships contain a cycle.
    pub fn dispatch(&self, event: &Event, focused_id: Option<&str>) -> DispatchResult {
        let mut current = focused_id;
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                break;
            }
            if let Some(cb) = self.callbacks.get(id) {
                if cb(event) {
                    return DispatchResult::Handled;
                }
            }
            current = self.parents.get(id).map(String::as_str);
        }
        DispatchResult::NotHandled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_focused() {
        let mut focus_manager = FocusManager::new();
        focus_manager.set_focusable_ids(vec!["button1".to_string(), "button2".to_string()]);

        assert!(focus_manager.is_focused("button1"));
        assert!(!focus_manager.is_focused("button2"));
        assert!(!focus_manager.is_focused("button3"));

        assert!(focus_manager.focus("button2"));
        assert!(!focus_manager.is_focused("button1"));
        assert!(focus_manager.is_focused("button2"));

        focus_manager.set_focusable_ids(vec![]);
        assert!(!focus_manager.is_focused("button1"));
        assert!(!focus_manager.is_focused("button2"));
    }
}
