use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) type NodeId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
    Clean,
    Check,
    Dirty,
}

pub(crate) struct ReactiveRuntime {
    pub current_observer: Option<NodeId>,
    pub pending_effects: VecDeque<NodeId>,
    pub queued_effects: HashSet<NodeId>,
    pub pending_memos: VecDeque<NodeId>,
    // FIFO membership: insert on enqueue, remove before execution or disposal.
    pub queued_memos: HashSet<NodeId>,
    pub batch_depth: u32,
    pub flushing: bool,
    pub next_id: NodeId,
    active_memos: usize,
}

impl ReactiveRuntime {
    fn new() -> Self {
        ReactiveRuntime {
            current_observer: None,
            pending_effects: VecDeque::new(),
            queued_effects: HashSet::new(),
            pending_memos: VecDeque::new(),
            queued_memos: HashSet::new(),
            batch_depth: 0,
            flushing: false,
            next_id: 1,
            active_memos: 0,
        }
    }
}

pub(crate) struct SignalNode {
    pub subscribers: HashSet<NodeId>,
}

pub(crate) type MemoRunFn = std::rc::Rc<dyn Fn() -> std::rc::Rc<dyn std::any::Any>>;
pub(crate) type MemoEqFn = std::rc::Rc<dyn Fn(&dyn std::any::Any, &dyn std::any::Any) -> bool>;

pub(crate) struct MemoNode {
    pub state: State,
    pub revision: u64,
    pub sources: Vec<NodeId>,
    pub subscribers: HashSet<NodeId>,
    /// Returns a shared snapshot so user code can run outside graph borrows.
    pub run: MemoRunFn,
    pub cached: Option<std::rc::Rc<dyn std::any::Any>>,
    /// Compare old and new values; returns true if they are equal (no change)
    pub eq_fn: MemoEqFn,
}

pub(crate) struct EffectNode {
    pub state: State,
    pub sources: Vec<NodeId>,
    pub run: std::rc::Rc<dyn Fn()>,
}

pub(crate) enum AnyNode {
    Signal(SignalNode),
    Memo(MemoNode),
    Effect(EffectNode),
}

impl AnyNode {
    pub(crate) fn subscribers(&self) -> Option<&HashSet<NodeId>> {
        match self {
            AnyNode::Signal(s) => Some(&s.subscribers),
            AnyNode::Memo(m) => Some(&m.subscribers),
            AnyNode::Effect(_) => None,
        }
    }

    pub(crate) fn subscribers_mut(&mut self) -> Option<&mut HashSet<NodeId>> {
        match self {
            AnyNode::Signal(s) => Some(&mut s.subscribers),
            AnyNode::Memo(m) => Some(&mut m.subscribers),
            AnyNode::Effect(_) => None,
        }
    }

    pub(crate) fn sources(&self) -> Option<&[NodeId]> {
        match self {
            AnyNode::Memo(m) => Some(&m.sources),
            AnyNode::Effect(e) => Some(&e.sources),
            AnyNode::Signal(_) => None,
        }
    }

    pub(crate) fn sources_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        match self {
            AnyNode::Memo(m) => Some(&mut m.sources),
            AnyNode::Effect(e) => Some(&mut e.sources),
            AnyNode::Signal(_) => None,
        }
    }
}

thread_local! {
    pub(crate) static RUNTIME: std::cell::RefCell<ReactiveRuntime> =
        std::cell::RefCell::new(ReactiveRuntime::new());

    pub(crate) static NODES: std::cell::RefCell<HashMap<NodeId, AnyNode>> =
        std::cell::RefCell::new(HashMap::new());
}

pub(crate) fn alloc_id() -> NodeId {
    RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        let id = rt.next_id;
        rt.next_id += 1;
        id
    })
}

pub(crate) struct ObserverGuard {
    previous: Option<NodeId>,
    source_backup: Option<(NodeId, Vec<NodeId>)>,
}

impl Drop for ObserverGuard {
    fn drop(&mut self) {
        RUNTIME.with(|rt| rt.borrow_mut().current_observer = self.previous);
        if let Some((observer, sources)) = self.source_backup.take() {
            for source in sources {
                track_source(source, observer);
            }
        }
    }
}

pub(crate) fn enter_observer(id: NodeId) -> ObserverGuard {
    let previous = RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        let previous = rt.current_observer;
        rt.current_observer = Some(id);
        previous
    });
    ObserverGuard {
        previous,
        source_backup: None,
    }
}

// A failed callback keeps both previous and newly read dependencies for retries.
pub(crate) fn run_observer<T>(id: NodeId, run: impl FnOnce() -> T) -> T {
    let sources = clear_observer_sources(id);
    let mut observer = enter_observer(id);
    observer.source_backup = Some((id, sources));
    let result = run();
    observer.source_backup = None;
    result
}

// Track computation/validation through comparisons and cache destruction as well.
// This lets normal invalidation keep its pruning while reentrant writes reach
// active descendants even when an intermediate memo is already dirty.
pub(crate) struct ActiveMemoGuard;

pub(crate) fn enter_memo() -> ActiveMemoGuard {
    RUNTIME.with(|rt| rt.borrow_mut().active_memos += 1);
    ActiveMemoGuard
}

impl Drop for ActiveMemoGuard {
    fn drop(&mut self) {
        RUNTIME.with(|rt| {
            rt.borrow_mut().active_memos -= 1;
        });
    }
}

pub(crate) fn clear_observer_sources(id: NodeId) -> Vec<NodeId> {
    let old_sources = NODES.with(|nodes| {
        let nodes = nodes.borrow();
        nodes
            .get(&id)
            .and_then(|n| n.sources())
            .map(|s| s.to_vec())
            .unwrap_or_default()
    });

    for source_id in &old_sources {
        NODES.with(|nodes| {
            let mut nodes = nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(source_id) {
                if let Some(subs) = node.subscribers_mut() {
                    subs.remove(&id);
                }
            }
        });
    }

    NODES.with(|nodes| {
        let mut nodes = nodes.borrow_mut();
        if let Some(node) = nodes.get_mut(&id) {
            if let Some(sources) = node.sources_mut() {
                sources.clear();
            }
        }
    });
    old_sources
}

pub(crate) fn remove_node(id: NodeId) {
    clear_observer_sources(id);
    RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        // Leave canceled numeric IDs for one linear drain instead of scanning per disposal.
        rt.queued_effects.remove(&id);
        rt.queued_memos.remove(&id);
        if rt.current_observer == Some(id) {
            rt.current_observer = None;
        }
    });
    let old_subscribers = NODES.with(|nodes| {
        nodes
            .borrow()
            .get(&id)
            .and_then(|node| node.subscribers())
            .map(|subscribers| subscribers.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    });

    for sub_id in &old_subscribers {
        NODES.with(|nodes| {
            let mut nodes = nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(sub_id) {
                if let Some(sources) = node.sources_mut() {
                    sources.retain(|&x| x != id);
                }
            }
        });
    }

    let removed = NODES.with(|nodes| nodes.borrow_mut().remove(&id));
    drop(removed);
}

/// Signals have no upstream sources or pending jobs; unlink their subscribers only.
pub(crate) fn remove_signal(id: NodeId) {
    // The graph may already be destroying captured signals during thread shutdown.
    let removed = NODES.try_with(|nodes| {
        let mut nodes = nodes.borrow_mut();
        let removed = nodes.remove(&id);
        if let Some(AnyNode::Signal(signal)) = &removed {
            for subscriber in &signal.subscribers {
                if let Some(sources) = nodes.get_mut(subscriber).and_then(AnyNode::sources_mut) {
                    sources.retain(|source| *source != id);
                }
            }
        }
        removed
    });
    drop(removed);
}

/// Record that `source_id` was read by the current observer.
pub(crate) fn track_read(source_id: NodeId) {
    let observer = RUNTIME.with(|rt| rt.borrow().current_observer);
    let Some(observer_id) = observer else {
        return;
    };
    track_source(source_id, observer_id);
}

fn track_source(source_id: NodeId, observer_id: NodeId) {
    NODES.with(|nodes| {
        let mut nodes = nodes.borrow_mut();

        if nodes.get(&observer_id).and_then(AnyNode::sources).is_none() {
            return;
        }
        // The reciprocal subscriber set already deduplicates this edge.
        let inserted = nodes
            .get_mut(&source_id)
            .and_then(AnyNode::subscribers_mut)
            .is_some_and(|subscribers| subscribers.insert(observer_id));
        if inserted {
            if let Some(sources) = nodes.get_mut(&observer_id).and_then(AnyNode::sources_mut) {
                sources.push(source_id);
            }
        }
    });
}

/// Queue directly changed subscribers and invalidate downstream memo caches first.
pub(crate) fn mark_subscribers_dirty(source_id: NodeId) {
    let subscribers = NODES.with(|nodes| {
        let nodes = nodes.borrow();
        nodes
            .get(&source_id)
            .and_then(|n| n.subscribers())
            .map(|s| s.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    });

    // Signal writes during user callbacks need a complete revision wave. Memo
    // commits already descend from that wave; forcing them would repeatedly walk
    // active Check chains during lazy reads inside a batch.
    let signal_source =
        NODES.with(|nodes| matches!(nodes.borrow().get(&source_id), Some(AnyNode::Signal(_))));
    let force = signal_source && RUNTIME.with(|rt| rt.borrow().active_memos != 0);
    for sub_id in subscribers {
        let is_effect = NODES.with(|nodes| {
            let nodes = nodes.borrow();
            matches!(nodes.get(&sub_id), Some(AnyNode::Effect(_)))
        });

        if is_effect {
            NODES.with(|nodes| {
                if let Some(AnyNode::Effect(e)) = nodes.borrow_mut().get_mut(&sub_id) {
                    e.state = State::Dirty;
                }
            });
            RUNTIME.with(|rt| {
                let mut rt = rt.borrow_mut();
                if rt.queued_effects.insert(sub_id) {
                    rt.pending_effects.push_back(sub_id);
                }
            });
        } else {
            let invalidated = NODES.with(|nodes| {
                let mut nodes = nodes.borrow_mut();
                let Some(AnyNode::Memo(memo)) = nodes.get_mut(&sub_id) else {
                    return false;
                };
                let changed = memo.state != State::Dirty;
                memo.revision = memo.revision.wrapping_add(1);
                memo.state = State::Dirty;
                changed
            });
            if invalidated || force {
                mark_descendant_memos_check(sub_id, force);
            }
            RUNTIME.with(|rt| {
                let mut rt = rt.borrow_mut();
                if rt.queued_memos.insert(sub_id) {
                    rt.pending_memos.push_back(sub_id);
                }
            });
        }
    }
}

/// Make all downstream caches suspect before any computation can observe a diamond.
/// Effects are queued only when a direct source actually changes its value.
fn mark_descendant_memos_check(source: NodeId, force: bool) {
    let mut pending = vec![source];
    let mut visited = force.then(|| HashSet::from([source]));
    while let Some(id) = pending.pop() {
        let subscribers = NODES.with(|nodes| {
            nodes
                .borrow()
                .get(&id)
                .and_then(AnyNode::subscribers)
                .map(|s| s.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
        });
        for subscriber in subscribers {
            if visited
                .as_mut()
                .is_some_and(|visited| !visited.insert(subscriber))
            {
                continue;
            }
            let changed = NODES.with(|nodes| {
                let mut nodes = nodes.borrow_mut();
                let Some(AnyNode::Memo(memo)) = nodes.get_mut(&subscriber) else {
                    return false;
                };
                if !force && memo.state != State::Clean {
                    return false;
                }
                memo.revision = memo.revision.wrapping_add(1);
                if memo.state == State::Clean {
                    memo.state = State::Check;
                }
                true
            });
            if changed {
                pending.push(subscriber);
            }
        }
    }
}

#[cfg(test)]
mod tracking_tests {
    use super::*;
    use crate::{Effect, Signal};

    #[test]
    fn tracking_keeps_order_and_reciprocal_edges_through_cleanup() {
        let first = Signal::new(1);
        let second = Signal::new(2);
        let observer = Effect::new(|| {});
        {
            let _guard = enter_observer(observer.id);
            for _ in 0..3 {
                second.get();
                first.get();
            }
        }
        NODES.with(|nodes| {
            let nodes = nodes.borrow();
            assert_eq!(
                nodes[&observer.id].sources().unwrap(),
                &[second.id, first.id]
            );
            for source in [&first, &second] {
                assert_eq!(nodes[&source.id].subscribers().unwrap().len(), 1);
            }
        });
        clear_observer_sources(observer.id);
        NODES.with(|nodes| {
            let nodes = nodes.borrow();
            assert!(nodes[&observer.id].sources().unwrap().is_empty());
            assert!(nodes[&first.id].subscribers().unwrap().is_empty());
            assert!(nodes[&second.id].subscribers().unwrap().is_empty());
        });
        {
            let _guard = enter_observer(observer.id);
            first.get();
        }
        drop(first);
        assert!(NODES.with(|nodes| nodes.borrow()[&observer.id].sources().unwrap().is_empty()));
        observer.dispose();
    }

    #[test]
    fn tracking_ignores_disposed_sources_and_restored_disposed_observers() {
        let source = Signal::new(1);
        let observer = Effect::new(|| {});
        let observer_id = observer.id;
        {
            let _guard = enter_observer(observer_id);
            track_read(NodeId::MAX);
            assert!(NODES.with(|nodes| nodes.borrow()[&observer_id].sources().unwrap().is_empty()));
        }
        observer.dispose();
        {
            let _guard = enter_observer(observer_id);
            source.get();
        }
        assert!(NODES.with(|nodes| nodes.borrow()[&source.id].subscribers().unwrap().is_empty()));
    }
}
