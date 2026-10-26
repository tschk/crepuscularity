use std::rc::Rc;

use crate::runtime::{
    alloc_id, remove_node, run_observer, AnyNode, EffectNode, NodeId, State, NODES,
};

/// Reactive side effect that re-runs when signals or memos read inside it change.
///
/// Effect handles belong to the thread that created their reactive graph.
/// They cannot be moved to another thread or shared across threads.
///
/// ```compile_fail
/// use crepuscularity_reactive::Effect;
/// let effect = Effect::new(|| {});
/// std::thread::spawn(move || effect.dispose());
/// ```
///
/// ```compile_fail
/// use crepuscularity_reactive::Effect;
/// let effect = Effect::new(|| {});
/// std::thread::scope(|scope| {
///     let shared = &effect;
///     scope.spawn(move || { std::hint::black_box(shared); });
/// });
/// ```
pub struct Effect {
    pub(crate) id: NodeId,
    _marker: std::marker::PhantomData<Rc<()>>,
}

// A failed first run never returns a public handle that could dispose the node.
// Keep this guard constructor-only: dropping an ordinary Effect handle is still
// intentionally independent of its graph lifetime.
struct PendingEffect(Option<NodeId>);

impl Drop for PendingEffect {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            remove_node(id);
        }
    }
}

impl Effect {
    /// Create an effect and run it once immediately to establish subscriptions.
    pub fn new(f: impl Fn() + 'static) -> Self {
        let id = alloc_id();
        NODES.with(|n| {
            n.borrow_mut().insert(
                id,
                AnyNode::Effect(EffectNode {
                    state: State::Dirty,
                    sources: vec![],
                    run: Rc::new(f),
                }),
            )
        });
        let mut pending = PendingEffect(Some(id));
        run_effect(id);
        pending.0 = None;
        Effect {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    /// Remove this effect from the reactive graph.
    pub fn dispose(self) {
        remove_node(self.id);
    }
}

pub(crate) fn run_effect(id: NodeId) {
    let run = NODES.with(|nodes| {
        let nodes = nodes.borrow();
        match nodes.get(&id) {
            Some(AnyNode::Effect(e)) => Some(Rc::clone(&e.run)),
            _ => None,
        }
    });

    let Some(run) = run else {
        return;
    };

    run_observer(id, || run());

    NODES.with(|nodes| {
        if let Some(AnyNode::Effect(e)) = nodes.borrow_mut().get_mut(&id) {
            e.state = State::Clean;
        }
    });
    crate::batch::maybe_flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::NODES;

    #[test]
    fn constructor_panic_reclaims_capture_and_subscriptions() {
        use crate::Signal;
        use std::cell::Cell;
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        let signal = Signal::new(0);
        let capture = Rc::new(());
        let weak = Rc::downgrade(&capture);
        let runs = Rc::new(Cell::new(0));
        let count = runs.clone();
        let read = signal.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Effect::new(move || {
                let _keep_alive = &capture;
                read.get();
                count.set(count.get() + 1);
                // Queue this active effect before unwinding as well.
                read.set(1);
                std::panic::panic_any(37usize);
            });
        }));
        assert_eq!(*result.unwrap_err().downcast::<usize>().unwrap(), 37);
        assert!(
            weak.upgrade().is_none(),
            "failed constructor retained its closure"
        );
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline + 1);
        signal.set(2);
        assert_eq!(runs.get(), 1, "failed constructor left a subscribed orphan");
        drop(signal);
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn failed_nested_constructor_restores_the_parent_observer() {
        use crate::runtime::RUNTIME;
        use crate::Signal;
        use std::cell::Cell;
        let baseline = NODES.with(|nodes| nodes.borrow().len());
        let signal = Signal::new(0);
        let runs = Rc::new(Cell::new(0));
        let count = runs.clone();
        let read = signal.clone();
        let parent = Effect::new(move || {
            let observer = RUNTIME.with(|runtime| runtime.borrow().current_observer);
            let result = std::panic::catch_unwind(|| {
                Effect::new(|| panic!("nested initialization"));
            });
            assert!(result.is_err());
            assert_eq!(
                RUNTIME.with(|runtime| runtime.borrow().current_observer),
                observer
            );
            read.get();
            count.set(count.get() + 1);
        });
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline + 2);
        signal.set(1);
        assert_eq!(runs.get(), 2);
        parent.dispose();
        drop(signal);
        assert_eq!(NODES.with(|nodes| nodes.borrow().len()), baseline);
    }

    #[test]
    fn test_effect_dispose() {
        let effect = Effect::new(|| {});
        let id = effect.id;

        // Verify it was added to NODES
        let exists_before = NODES.with(|nodes| nodes.borrow().contains_key(&id));
        assert!(exists_before, "Effect should exist in NODES after creation");

        effect.dispose();

        // Verify it was removed from NODES
        let exists_after = NODES.with(|nodes| nodes.borrow().contains_key(&id));
        assert!(
            !exists_after,
            "Effect should be removed from NODES after dispose"
        );
    }
}
