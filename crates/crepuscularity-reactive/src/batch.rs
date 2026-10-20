use crate::effect::run_effect;
use crate::runtime::RUNTIME;

/// Begin a nested batch of memo and effect updates. Memo reads remain current.
pub fn batch_begin() {
    RUNTIME.with(|rt| rt.borrow_mut().batch_depth += 1);
}

/// End one batch level, flushing queued updates when the outermost batch ends.
pub fn batch_end() {
    let should = RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        rt.batch_depth = rt.batch_depth.saturating_sub(1);
        rt.batch_depth == 0
    });
    if should {
        flush();
    }
}

pub(crate) fn maybe_flush() {
    let should = RUNTIME.with(|rt| rt.borrow().batch_depth == 0);
    if should {
        flush();
    }
}

struct FlushGuard;

impl Drop for FlushGuard {
    fn drop(&mut self) {
        RUNTIME.with(|rt| rt.borrow_mut().flushing = false);
    }
}

pub fn flush() {
    let started = RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        if rt.flushing || rt.current_observer.is_some() {
            return false;
        }
        rt.flushing = true;
        true
    });
    if !started {
        return;
    }
    let _guard = FlushGuard;
    loop {
        let memo = RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            while let Some(id) = rt.pending_memos.pop_front() {
                if rt.queued_memos.remove(&id) {
                    return Some(id);
                }
            }
            None
        });
        if let Some(id) = memo {
            crate::memo::run_memo_if_needed(id);
            continue;
        }
        let next = RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            while let Some(id) = rt.pending_effects.pop_front() {
                if rt.queued_effects.remove(&id) {
                    return Some(id);
                }
            }
            None
        });
        let Some(id) = next else {
            break;
        };
        run_effect(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::Effect;
    use crate::runtime::{alloc_id, AnyNode, EffectNode, State, NODES, RUNTIME};
    use crate::signal::Signal;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[test]
    fn signal_writes_do_not_reenter_running_effects() {
        let signal = Signal::new(0);
        let active = Rc::new(Cell::new(false));
        let observed = Rc::new(RefCell::new(Vec::new()));
        let effect = Effect::new({
            let signal = signal.clone();
            let active = active.clone();
            let observed = observed.clone();
            move || {
                assert!(!active.replace(true), "effect reentered");
                let value = signal.get();
                observed.borrow_mut().push(value);
                if value < 2 {
                    signal.set(value + 1);
                }
                active.set(false);
            }
        });
        assert_eq!(*observed.borrow(), vec![0, 1, 2]);
        signal.set(0);
        assert_eq!(*observed.borrow(), vec![0, 1, 2, 0, 1, 2]);
        assert!(RUNTIME.with(|rt| rt.borrow().queued_effects.is_empty()));
        effect.dispose();
    }

    #[test]
    fn callback_panic_does_not_disable_future_flushes() {
        let signal = Signal::new(0);
        let observed = Rc::new(Cell::new(0));
        let effect = Effect::new({
            let signal = signal.clone();
            let observed = observed.clone();
            move || {
                let value = signal.get();
                assert_ne!(value, 1, "callback failed");
                observed.set(value);
            }
        });
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| signal.set(1))).is_err());
        assert!(!RUNTIME.with(|rt| rt.borrow().queued_effects.contains(&effect.id)));
        signal.set(2);
        assert_eq!(observed.get(), 2);
        assert!(RUNTIME.with(|rt| rt.borrow().queued_effects.is_empty()));
        effect.dispose();
    }

    #[test]
    fn callback_panic_preserves_other_queued_effects() {
        let first = Signal::new(0);
        let second = Signal::new(0);
        let observed = Rc::new(Cell::new(0));
        let failing = Effect::new({
            let first = first.clone();
            move || assert_ne!(first.get(), 1, "callback failed")
        });
        let surviving = Effect::new({
            let second = second.clone();
            let observed = observed.clone();
            move || observed.set(second.get())
        });
        batch_begin();
        first.set(1);
        second.set(2);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(batch_end)).is_err());
        flush();
        assert_eq!(observed.get(), 2);
        failing.dispose();
        surviving.dispose();
    }

    #[test]
    fn test_batch_begin_increments_depth() {
        let initial_depth = RUNTIME.with(|rt| rt.borrow().batch_depth);
        batch_begin();
        assert_eq!(
            RUNTIME.with(|rt| rt.borrow().batch_depth),
            initial_depth + 1
        );
        batch_end();
        assert_eq!(RUNTIME.with(|rt| rt.borrow().batch_depth), initial_depth);
    }

    #[test]
    fn test_nested_batches() {
        let initial_depth = RUNTIME.with(|rt| rt.borrow().batch_depth);
        batch_begin();
        batch_begin();
        assert_eq!(
            RUNTIME.with(|rt| rt.borrow().batch_depth),
            initial_depth + 2
        );
        batch_end();
        assert_eq!(
            RUNTIME.with(|rt| rt.borrow().batch_depth),
            initial_depth + 1
        );
        batch_end();
        assert_eq!(RUNTIME.with(|rt| rt.borrow().batch_depth), initial_depth);
    }

    #[test]
    fn test_batch_end_saturating_sub() {
        RUNTIME.with(|rt| rt.borrow_mut().batch_depth = 0);
        batch_end();
        assert_eq!(RUNTIME.with(|rt| rt.borrow().batch_depth), 0);
    }

    #[test]
    fn test_batch_defers_effects() {
        let signal = Signal::new(0);
        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();
        let signal_clone = signal.clone();
        let _effect = Effect::new(move || {
            signal_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });
        assert_eq!(run_count.get(), 1);
        batch_begin();
        signal.set(1);
        signal.set(2);
        assert_eq!(run_count.get(), 1);
        batch_end();
        assert_eq!(run_count.get(), 2);
    }

    #[test]
    fn test_flush_mock_effects() {
        let run_count = Rc::new(RefCell::new(0));
        let rc1 = Rc::clone(&run_count);
        let id1 = alloc_id();
        NODES.with(|n| {
            n.borrow_mut().insert(
                id1,
                AnyNode::Effect(EffectNode {
                    state: State::Dirty,
                    sources: vec![],
                    run: Rc::new(move || *rc1.borrow_mut() += 1),
                }),
            )
        });
        let rc2 = Rc::clone(&run_count);
        let id2 = alloc_id();
        NODES.with(|n| {
            n.borrow_mut().insert(
                id2,
                AnyNode::Effect(EffectNode {
                    state: State::Dirty,
                    sources: vec![],
                    run: Rc::new(move || *rc2.borrow_mut() += 1),
                }),
            )
        });
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.pending_effects.push_back(id1);
            rt.pending_effects.push_back(id2);
            rt.queued_effects.extend([id1, id2]);
        });
        flush();
        assert_eq!(*run_count.borrow(), 2);
        assert!(RUNTIME.with(|rt| rt.borrow().pending_effects.is_empty()));
        NODES.with(|n| {
            let mut n = n.borrow_mut();
            n.remove(&id1);
            n.remove(&id2);
        });
    }

    #[test]
    fn test_flush_nested_effects() {
        let run_count = Rc::new(RefCell::new(0));
        let id2 = alloc_id();
        let rc2 = Rc::clone(&run_count);
        NODES.with(|n| {
            n.borrow_mut().insert(
                id2,
                AnyNode::Effect(EffectNode {
                    state: State::Dirty,
                    sources: vec![],
                    run: Rc::new(move || *rc2.borrow_mut() += 1),
                }),
            )
        });
        let id1 = alloc_id();
        let rc1 = Rc::clone(&run_count);
        NODES.with(|n| {
            n.borrow_mut().insert(
                id1,
                AnyNode::Effect(EffectNode {
                    state: State::Dirty,
                    sources: vec![],
                    run: Rc::new(move || {
                        *rc1.borrow_mut() += 1;
                        RUNTIME.with(|rt| {
                            let mut rt = rt.borrow_mut();
                            if rt.queued_effects.insert(id2) {
                                rt.pending_effects.push_back(id2);
                            }
                        });
                    }),
                }),
            )
        });
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.pending_effects.push_back(id1);
            rt.queued_effects.insert(id1);
        });
        flush();
        assert_eq!(*run_count.borrow(), 2);
        assert!(RUNTIME.with(|rt| rt.borrow().pending_effects.is_empty()));
        NODES.with(|n| {
            let mut n = n.borrow_mut();
            n.remove(&id1);
            n.remove(&id2);
        });
    }

    #[test]
    fn effect_queue_deduplicates_preserves_fifo_and_unlinks_disposed_jobs() {
        let signals = [Signal::new(0), Signal::new(0), Signal::new(0)];
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut effects: Vec<_> = signals
            .iter()
            .enumerate()
            .map(|(index, signal)| {
                let signal = signal.clone();
                let seen = seen.clone();
                Effect::new(move || {
                    signal.get();
                    seen.borrow_mut().push(index);
                })
            })
            .collect();
        seen.borrow_mut().clear();
        batch_begin();
        signals[1].set(1);
        signals[0].set(1);
        signals[2].set(1);
        for value in 2..=100 {
            for signal in &signals {
                signal.set(value);
            }
        }
        let expected = [effects[1].id, effects[0].id, effects[2].id];
        RUNTIME.with(|rt| {
            let rt = rt.borrow();
            assert_eq!(
                rt.pending_effects.iter().copied().collect::<Vec<_>>(),
                expected
            );
            assert_eq!(rt.queued_effects.len(), 3);
        });
        effects.remove(0).dispose();
        RUNTIME.with(|rt| {
            let rt = rt.borrow();
            assert_eq!(
                rt.pending_effects.iter().copied().collect::<Vec<_>>(),
                expected
            );
            assert!(!rt.queued_effects.contains(&expected[1]));
        });
        batch_end();
        assert_eq!(*seen.borrow(), [1, 2]);
        assert!(RUNTIME.with(|rt| rt.borrow().queued_effects.is_empty()));
        signals[1].set(101);
        assert_eq!(*seen.borrow(), [1, 2, 1]);
        for effect in effects {
            effect.dispose();
        }
    }
    #[test]
    fn bulk_disposal_defers_fifo_cleanup_until_flush() {
        use crate::memo::Memo;
        let signal = Signal::new(0);
        let memos: Vec<_> = (0..256)
            .map(|_| {
                let signal = signal.clone();
                let memo = Memo::new(move || signal.get());
                memo.get();
                memo
            })
            .collect();
        let effects: Vec<_> = (0..256)
            .map(|_| {
                let signal = signal.clone();
                Effect::new(move || {
                    signal.get();
                })
            })
            .collect();
        batch_begin();
        signal.set(1);
        let queued = RUNTIME.with(|rt| {
            let rt = rt.borrow();
            (rt.pending_memos.clone(), rt.pending_effects.clone())
        });
        for memo in memos {
            memo.dispose();
        }
        for effect in effects {
            effect.dispose();
        }
        RUNTIME.with(|rt| {
            let rt = rt.borrow();
            assert!(rt.queued_memos.is_empty());
            assert!(rt.queued_effects.is_empty());
            // Cancel in constant time; bulk teardown must not repeatedly scan FIFOs.
            assert_eq!(rt.pending_memos, queued.0);
            assert_eq!(rt.pending_effects, queued.1);
        });
        let source = Signal::new(0);
        let memo = Memo::new({
            let source = source.clone();
            move || source.get()
        });
        let seen = Rc::new(RefCell::new(Vec::new()));
        let effect = Effect::new({
            let memo = memo.clone();
            let seen = Rc::clone(&seen);
            move || seen.borrow_mut().push(memo.get())
        });
        source.set(1);
        batch_end();
        assert_eq!(*seen.borrow(), [0, 1]);
        effect.dispose();
        memo.dispose();
        RUNTIME.with(|rt| {
            let rt = rt.borrow();
            assert!(rt.pending_memos.is_empty());
            assert!(rt.pending_effects.is_empty());
        });
    }
    #[test]
    fn flushing_skips_jobs_disposed_by_an_earlier_callback() {
        let signals = [Signal::new(0), Signal::new(0), Signal::new(0)];
        let canceled = Rc::new(RefCell::new(None::<Effect>));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let first = Effect::new({
            let signal = signals[0].clone();
            let canceled = Rc::clone(&canceled);
            let seen = Rc::clone(&seen);
            move || {
                if signal.get() != 0 {
                    seen.borrow_mut().push(0);
                    canceled.borrow_mut().take().unwrap().dispose();
                }
            }
        });
        *canceled.borrow_mut() = Some(Effect::new({
            let signal = signals[1].clone();
            let seen = Rc::clone(&seen);
            move || {
                if signal.get() != 0 {
                    seen.borrow_mut().push(1);
                }
            }
        }));
        let last = Effect::new({
            let signal = signals[2].clone();
            let seen = Rc::clone(&seen);
            move || {
                if signal.get() != 0 {
                    seen.borrow_mut().push(2);
                }
            }
        });
        batch_begin();
        for signal in &signals {
            signal.set(1);
        }
        batch_end();
        assert_eq!(*seen.borrow(), [0, 2]);
        first.dispose();
        last.dispose();
    }
}
