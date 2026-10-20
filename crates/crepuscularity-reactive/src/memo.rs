use std::rc::Rc;

use crate::runtime::{
    alloc_id, enter_memo, mark_subscribers_dirty, remove_node, run_observer, track_read, AnyNode,
    MemoEqFn, MemoNode, MemoRunFn, NodeId, State, NODES,
};

/// Cached reactive computation that notifies dependents only when its value changes.
///
/// Memo handles belong to the thread that created their reactive graph.
/// They cannot be moved to another thread or shared across threads.
///
/// ```compile_fail
/// use crepuscularity_reactive::Memo;
/// let memo = Memo::new(|| 1);
/// std::thread::spawn(move || { std::hint::black_box(memo.get()); });
/// ```
///
/// ```compile_fail
/// use crepuscularity_reactive::Memo;
/// let memo = Memo::new(|| 1);
/// std::thread::scope(|scope| {
///     let shared = &memo;
///     scope.spawn(move || { std::hint::black_box(shared.get()); });
/// });
/// ```
pub struct Memo<T: Clone + PartialEq + 'static> {
    pub(crate) id: NodeId,
    _marker: std::marker::PhantomData<(T, Rc<()>)>,
}

impl<T: Clone + PartialEq + 'static> Memo<T> {
    /// Create a lazy memo. The closure runs on the first [`Memo::get`] call.
    pub fn new(f: impl Fn() -> T + 'static) -> Self {
        let id = alloc_id();
        let run: MemoRunFn = Rc::new(move || Rc::new(f()) as Rc<dyn std::any::Any>);
        let eq_fn: MemoEqFn = Rc::new(|a: &dyn std::any::Any, b: &dyn std::any::Any| {
            match (a.downcast_ref::<T>(), b.downcast_ref::<T>()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
        });
        NODES.with(|n| {
            n.borrow_mut().insert(
                id,
                AnyNode::Memo(MemoNode {
                    state: State::Dirty,
                    revision: 0,
                    sources: vec![],
                    subscribers: std::collections::HashSet::new(),
                    run,
                    cached: None,
                    eq_fn,
                }),
            )
        });
        Memo {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    /// Return the current value, recomputing if an upstream dependency changed.
    pub fn get(&self) -> T {
        track_read(self.id);
        run_memo_if_needed(self.id);
        let cached = NODES.with(|nodes| {
            let nodes = nodes.borrow();
            match nodes.get(&self.id) {
                Some(AnyNode::Memo(m)) => m.cached.as_ref().map(Rc::clone),
                _ => panic!("memo node not found"),
            }
        });
        cached
            .as_ref()
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
            .expect("memo cached value must be T")
    }

    /// Remove this memo from the reactive graph.
    ///
    /// All clones share the same graph node. Disposing one clone invalidates the others.
    pub fn dispose(self) {
        remove_node(self.id);
    }
}

impl<T: Clone + PartialEq + 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Memo {
            id: self.id,
            _marker: std::marker::PhantomData,
        }
    }
}

/// Recompute memo if its state is not Clean.
pub(crate) fn run_memo_if_needed(id: NodeId) {
    loop {
        let snapshot = NODES.with(|nodes| {
            let nodes = nodes.borrow();
            match nodes.get(&id) {
                Some(AnyNode::Memo(m)) => Some((m.state, m.revision)),
                _ => None,
            }
        });
        match snapshot {
            None | Some((State::Clean, _)) => return,
            Some((State::Dirty, _)) => run_memo(id),
            Some((State::Check, _)) => validate_memo_sources(id),
        }
        // Recheck after user comparisons/destructors, which may invalidate us or
        // install a newer cache through a nested computation.
    }
}

// Known dependency traversal uses heap frames.
// shortcut: user computations still recurse; revisit for deep cold memo chains.
fn validate_memo_sources(id: NodeId) {
    enum Frame {
        Visit(NodeId),
        Finish(NodeId, u64, crate::runtime::ActiveMemoGuard),
    }
    let mut pending = vec![Frame::Visit(id)];
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Visit(id) => {
                let snapshot = NODES.with(|nodes| match nodes.borrow().get(&id) {
                    Some(AnyNode::Memo(memo)) => Some((memo.state, memo.revision)),
                    _ => None,
                });
                match snapshot {
                    None | Some((State::Clean, _)) => {}
                    Some((State::Dirty, _)) => run_memo_if_needed(id),
                    Some((State::Check, revision)) => {
                        let active = enter_memo();
                        let sources = NODES.with(|nodes| {
                            nodes
                                .borrow()
                                .get(&id)
                                .and_then(AnyNode::sources)
                                .map(<[NodeId]>::to_vec)
                                .unwrap_or_default()
                        });
                        pending.push(Frame::Finish(id, revision, active));
                        pending.extend(sources.into_iter().rev().map(Frame::Visit));
                    }
                }
            }
            Frame::Finish(id, revision, active) => {
                NODES.with(|nodes| {
                    if let Some(AnyNode::Memo(memo)) = nodes.borrow_mut().get_mut(&id) {
                        if memo.state == State::Check && memo.revision == revision {
                            memo.state = State::Clean;
                        }
                    }
                });
                drop(active);
                pending.push(Frame::Visit(id));
            }
        }
    }
}

/// Run the memo computation, update cached value, and notify subscribers if the value changed.
pub(crate) fn run_memo(id: NodeId) {
    let _active = enter_memo();
    let run = NODES.with(|nodes| {
        let nodes = nodes.borrow();
        match nodes.get(&id) {
            Some(AnyNode::Memo(m)) => Some((Rc::clone(&m.run), m.revision)),
            _ => None,
        }
    });

    let Some((run, revision)) = run else {
        return;
    };
    let new_val = run_observer(id, || run());

    {
        let snapshot = NODES.with(|nodes| {
            let nodes = nodes.borrow();
            match nodes.get(&id) {
                Some(AnyNode::Memo(m)) => {
                    Some((m.cached.as_ref().map(Rc::clone), Rc::clone(&m.eq_fn)))
                }
                _ => None,
            }
        });
        let (had_cached, changed) = match &snapshot {
            Some((Some(old), equal)) => (true, !equal(old.as_ref(), new_val.as_ref())),
            _ => (false, true),
        };
        let mut pending_value = Some(new_val);
        let (committed, previous) = NODES.with(|nodes| {
            if let Some(AnyNode::Memo(m)) = nodes.borrow_mut().get_mut(&id) {
                if m.revision == revision {
                    let previous = m
                        .cached
                        .replace(pending_value.take().expect("new memo value"));
                    m.state = State::Clean;
                    return (true, previous);
                }
            }
            (false, None)
        });

        if committed && had_cached && changed {
            mark_subscribers_dirty(id);
        }
        drop(previous);
        drop(snapshot);
        drop(pending_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{AnyNode, State, NODES};
    use crate::signal::Signal;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn test_memo_new() {
        let memo = Memo::new(|| 42);
        NODES.with(|nodes| {
            let nodes = nodes.borrow();
            let node = nodes.get(&memo.id).expect("node");
            if let AnyNode::Memo(m) = node {
                assert_eq!(m.state, State::Dirty);
                assert!(m.cached.is_none());
                assert!(m.sources.is_empty());
                assert!(m.subscribers.is_empty());
            } else {
                panic!("not a memo");
            }
        });
    }

    #[test]
    fn test_memo_dispose() {
        let memo = Memo::new(|| 42);
        let id = memo.id;
        NODES.with(|nodes| assert!(nodes.borrow().contains_key(&id)));
        memo.dispose();
        NODES.with(|nodes| assert!(!nodes.borrow().contains_key(&id)));
    }

    #[test]
    fn test_memo_get_computes_lazily_and_caches() {
        let runs = Rc::new(Cell::new(0));
        let runs_for_memo = Rc::clone(&runs);
        let memo = Memo::new(move || {
            runs_for_memo.set(runs_for_memo.get() + 1);
            42
        });

        assert_eq!(runs.get(), 0);
        assert_eq!(memo.get(), 42);
        assert_eq!(memo.get(), 42);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn test_memo_get_reflects_signal_updates() {
        let signal = Signal::new(1);
        let source = signal.clone();
        let memo = Memo::new(move || source.get() * 2);

        assert_eq!(memo.get(), 2);
        signal.set(5);
        assert_eq!(memo.get(), 10);
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CallbackPhase {
        Clone,
        Equal,
        Drop,
    }

    struct ReentrantValue {
        value: i32,
        callback: Rc<dyn Fn(CallbackPhase)>,
    }

    impl Clone for ReentrantValue {
        fn clone(&self) -> Self {
            (self.callback)(CallbackPhase::Clone);
            Self {
                value: self.value,
                callback: Rc::clone(&self.callback),
            }
        }
    }

    impl PartialEq for ReentrantValue {
        fn eq(&self, other: &Self) -> bool {
            (self.callback)(CallbackPhase::Equal);
            self.value == other.value
        }
    }

    impl Drop for ReentrantValue {
        fn drop(&mut self) {
            (self.callback)(CallbackPhase::Drop);
        }
    }

    fn callback_can_drop_last_signal(phase: CallbackPhase) {
        let signal = Signal::new(42);
        let id = signal.id;
        let owned = Rc::new(std::cell::RefCell::new(Some(signal)));
        let callback: Rc<dyn Fn(CallbackPhase)> = Rc::new(move |current| {
            if current == phase {
                drop(owned.borrow_mut().take());
            }
        });
        let source = Signal::new(0);
        let memo = Memo::new({
            let source = source.clone();
            move || ReentrantValue {
                value: source.get(),
                callback: Rc::clone(&callback),
            }
        });
        run_memo(memo.id);
        if phase == CallbackPhase::Clone {
            drop(memo.get());
        } else {
            source.set(1);
        }
        assert!(!NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        memo.dispose();
    }

    #[test]
    fn cloned_memo_values_can_release_the_last_signal_handle() {
        callback_can_drop_last_signal(CallbackPhase::Clone);
    }

    #[test]
    fn memo_equality_can_release_the_last_signal_handle() {
        callback_can_drop_last_signal(CallbackPhase::Equal);
    }

    #[test]
    fn replaced_cached_values_can_release_the_last_signal_handle() {
        callback_can_drop_last_signal(CallbackPhase::Drop);
    }

    #[test]
    fn equality_can_dispose_its_memo_before_new_value_installation() {
        let source = Signal::new(0);
        let owner = Rc::new(std::cell::RefCell::new(None::<Memo<ReentrantValue>>));
        let released = Rc::new(std::cell::RefCell::new(Vec::new()));
        let memo = Memo::new({
            let source = source.clone();
            let owner = owner.clone();
            let released = released.clone();
            move || {
                let signal = Signal::new(source.get());
                released.borrow_mut().push(signal.id);
                let signal = std::cell::RefCell::new(Some(signal));
                let owner = owner.clone();
                ReentrantValue {
                    value: source.get(),
                    callback: Rc::new(move |phase| {
                        if phase == CallbackPhase::Equal {
                            if let Some(memo) = owner.borrow_mut().take() {
                                memo.dispose();
                            }
                        }
                        if phase == CallbackPhase::Drop {
                            drop(signal.borrow_mut().take());
                        }
                    }),
                }
            }
        });
        let id = memo.id;
        run_memo(id);
        *owner.borrow_mut() = Some(memo.clone());
        source.set(1);
        assert!(!NODES.with(|nodes| nodes.borrow().contains_key(&id)));
        for signal in released.borrow().iter() {
            assert!(!NODES.with(|nodes| nodes.borrow().contains_key(signal)));
        }
    }

    #[test]
    fn signal_handles_can_outlive_graph_thread_local_teardown() {
        std::thread::spawn(|| {
            thread_local! {
                static LATE: std::cell::RefCell<Option<Signal<i32>>> = const { std::cell::RefCell::new(None) };
            }
            LATE.with(|late| {
                let signal = Signal::new(1);
                let captured = signal.clone();
                let memo = Memo::new(move || captured.get());
                assert_eq!(memo.get(), 1);
                *late.borrow_mut() = Some(signal);
                // Memo graph retention is intentional; its closure drops during TLS teardown.
            });
        }).join().unwrap();
    }
}
