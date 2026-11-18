use std::cell::RefCell;
use std::rc::Rc;

use crate::batch::maybe_flush;
use crate::runtime::{
    alloc_id, mark_subscribers_dirty, remove_signal, track_read, AnyNode, NodeId, SignalNode, NODES,
};

/// Reactive value that notifies memos and effects when it changes.
///
/// Clones share one value and graph node; dropping the last handle removes that node.
/// Memo and effect closures retain their captured handles until explicitly disposed.
pub struct Signal<T: Clone + PartialEq + 'static> {
    pub(crate) id: NodeId,
    value: Rc<RefCell<Rc<T>>>,
}

impl<T: Clone + PartialEq + 'static> Signal<T> {
    /// Create a signal with an initial value.
    pub fn new(value: T) -> Self {
        let id = alloc_id();
        NODES.with(|n| {
            n.borrow_mut().insert(
                id,
                AnyNode::Signal(SignalNode {
                    subscribers: std::collections::HashSet::new(),
                }),
            )
        });
        Signal {
            id,
            value: Rc::new(RefCell::new(Rc::new(value))),
        }
    }

    /// Read the current value and subscribe the active memo or effect.
    pub fn get(&self) -> T {
        track_read(self.id);
        let snapshot = Rc::clone(&self.value.borrow());
        snapshot.as_ref().clone()
    }

    /// Replace the value and flush dependent effects unless a batch is active.
    /// If comparison writes this signal, this call commits its replacement afterward.
    pub fn set(&self, val: T) {
        let snapshot = Rc::clone(&self.value.borrow());
        let equal = snapshot.as_ref() == &val;
        if equal && Rc::ptr_eq(&snapshot, &self.value.borrow()) {
            return;
        }
        drop(snapshot);
        let displaced = {
            let mut current = self.value.borrow_mut();
            // Reuse unique storage; an outstanding read snapshot must retain its value.
            match Rc::get_mut(&mut current) {
                Some(current) => (Some(std::mem::replace(current, val)), None),
                None => (None, Some(std::mem::replace(&mut *current, Rc::new(val)))),
            }
        };
        mark_subscribers_dirty(self.id);
        drop(displaced);
        maybe_flush();
    }

    /// Compute a replacement value from the previous value.
    pub fn update(&self, f: impl FnOnce(T) -> T) {
        let previous = {
            let snapshot = Rc::clone(&self.value.borrow());
            snapshot.as_ref().clone()
        };
        let val = f(previous);
        self.set(val);
    }
}

impl<T: Clone + PartialEq + 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Signal {
            id: self.id,
            value: Rc::clone(&self.value),
        }
    }
}

impl<T: Clone + PartialEq + 'static> Drop for Signal<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.value) == 1 {
            remove_signal(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::Effect;
    use crate::runtime::{alloc_id, enter_observer, AnyNode, EffectNode, State, NODES};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn signal_lifecycle_ephemeral_nodes_return_to_baseline() {
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        for value in 0..1000 {
            let signal = Signal::new(value);
            assert_eq!(signal.get(), value);
            drop(signal);
        }
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn signal_lifecycle_only_the_last_clone_reclaims_the_node() {
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        let first = Signal::new(1);
        let id = first.id;
        let second = first.clone();
        let last = second.clone();
        drop(first);
        second.set(2);
        assert_eq!(last.get(), 2);
        last.update(|value| value + 1);
        assert_eq!(second.get(), 3);
        drop(second);
        assert!(NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        last.set(4);
        assert_eq!(last.get(), 4);
        drop(last);
        assert!(!NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn signal_lifecycle_effect_capture_lives_until_explicit_disposal() {
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        let signal = Signal::new(1);
        let id = signal.id;
        let captured = signal.clone();
        let observed = Rc::new(RefCell::new(0));
        let output = Rc::clone(&observed);
        let effect = Effect::new(move || *output.borrow_mut() = captured.get());
        signal.set(2);
        assert_eq!(*observed.borrow(), 2);
        drop(signal);
        assert!(NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline + 2);
        effect.dispose();
        assert!(!NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn signal_lifecycle_memo_capture_lives_until_explicit_disposal() {
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        let signal = Signal::new(2);
        let id = signal.id;
        let captured = signal.clone();
        let memo = crate::Memo::new(move || captured.get() * 2);
        assert_eq!(memo.get(), 4);
        signal.set(3);
        assert_eq!(memo.get(), 6);
        drop(signal);
        assert!(NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        assert_eq!(memo.get(), 6);
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline + 2);
        memo.dispose();
        assert!(!NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn test_signal_new() {
        let sig = Signal::new(42);
        NODES.with(|nodes| {
            let nodes = nodes.borrow();
            match nodes.get(&sig.id).expect("node") {
                AnyNode::Signal(s) => assert!(s.subscribers.is_empty()),
                _ => panic!("not a signal"),
            }
        });
        assert_eq!(**sig.value.borrow(), 42);
    }

    #[test]
    fn test_signal_get() {
        assert_eq!(Signal::new(42).get(), 42);
    }

    #[test]
    fn test_signal_get_tracks_read() {
        let signal = Signal::new("hello".to_string());
        let observer_id = alloc_id();
        NODES.with(|n| {
            n.borrow_mut().insert(
                observer_id,
                AnyNode::Effect(EffectNode {
                    state: State::Clean,
                    sources: vec![],
                    run: Rc::new(|| {}),
                }),
            )
        });
        {
            let _guard = enter_observer(observer_id);
            assert_eq!(signal.get(), "hello");
        }
        let has_subscriber = NODES.with(|nodes| match nodes.borrow().get(&signal.id) {
            Some(AnyNode::Signal(s)) => s.subscribers.contains(&observer_id),
            _ => false,
        });
        assert!(has_subscriber);
        let has_source = NODES.with(|nodes| match nodes.borrow().get(&observer_id) {
            Some(AnyNode::Effect(e)) => e.sources.contains(&signal.id),
            _ => false,
        });
        assert!(has_source);
    }

    #[test]
    fn test_signal_set() {
        let sig = Signal::new(0);
        let counter = Rc::new(RefCell::new(0));
        let c_clone = counter.clone();
        let sig_clone = sig.clone();
        let _effect = Effect::new(move || {
            sig_clone.get();
            *c_clone.borrow_mut() += 1;
        });
        assert_eq!(*counter.borrow(), 1);
        sig.set(0);
        assert_eq!(*counter.borrow(), 1);
        sig.set(1);
        assert_eq!(*counter.borrow(), 2);
        assert_eq!(sig.get(), 1);
    }

    #[test]
    fn test_signal_update() {
        let signal = Signal::new(5);
        signal.update(|v| v + 10);
        assert_eq!(signal.get(), 15);
    }

    #[test]
    fn test_signal_update_multiple_times() {
        let signal = Signal::new(0);
        signal.update(|v| v + 1);
        signal.update(|v| v + 2);
        assert_eq!(signal.get(), 3);
    }

    #[test]
    fn test_signal_update_callback_can_write_to_same_signal() {
        let signal = Signal::new(5);
        let signal_clone = signal.clone();
        signal.update(|_| {
            signal_clone.set(10);
            signal_clone.get() + 1
        });
        assert_eq!(signal.get(), 11);
    }
}
