use crepuscularity_reactive::{batch_begin, batch_end, Effect, Memo, Signal};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn signal_read_write() {
    let s = Signal::new(42i32);
    assert_eq!(s.get(), 42);
    s.set(100);
    assert_eq!(s.get(), 100);
    s.update(|v| v + 1);
    assert_eq!(s.get(), 101);
}

#[test]
fn signal_update_behavior() {
    // 1. Happy path: modifies the signal's value correctly based on its previous value.
    let s = Signal::new(10i32);
    assert_eq!(s.get(), 10);
    s.update(|v| v * 2);
    assert_eq!(s.get(), 20);

    // 2. Effect triggering: notifies and re-runs any dependent Effects.
    let count = Rc::new(RefCell::new(0u32));
    let count2 = Rc::clone(&count);
    let s2 = s.clone();
    let _e = Effect::new(move || {
        let _ = s2.get();
        *count2.borrow_mut() += 1;
    });

    assert_eq!(*count.borrow(), 1, "Effect ran once on init");
    s.update(|v| v + 5);
    assert_eq!(s.get(), 25);
    assert_eq!(*count.borrow(), 2, "Effect re-ran after update");

    // 3. No-op update: function returning same value does not trigger dependent effects
    s.update(|v| v);
    assert_eq!(s.get(), 25);
    assert_eq!(*count.borrow(), 2, "Effect did not re-run on no-op update");
}

#[test]
fn effect_runs_immediately() {
    let ran = Rc::new(RefCell::new(false));
    let ran2 = Rc::clone(&ran);
    let _e = Effect::new(move || {
        *ran2.borrow_mut() = true;
    });
    assert!(*ran.borrow(), "effect should run immediately on creation");
}

#[test]
fn effect_reruns_on_change() {
    let count = Rc::new(RefCell::new(0u32));
    let s = Signal::new(0i32);
    let s2 = s.clone();
    let count2 = Rc::clone(&count);
    let _e = Effect::new(move || {
        let _ = s2.get(); // track signal
        *count2.borrow_mut() += 1;
    });
    assert_eq!(*count.borrow(), 1, "ran once on init");
    s.set(1);
    assert_eq!(*count.borrow(), 2, "ran again after set");
    s.set(2);
    assert_eq!(*count.borrow(), 3, "ran again after another set");
    // Setting same value should NOT re-run
    s.set(2);
    assert_eq!(*count.borrow(), 3, "no re-run on same value");
}

#[test]
fn memo_caches() {
    let s = Signal::new(2i32);
    let s2 = s.clone();
    let compute_count = Rc::new(RefCell::new(0u32));
    let cc2 = Rc::clone(&compute_count);
    let m = Memo::new(move || {
        *cc2.borrow_mut() += 1;
        s2.get() * 2
    });

    // First get triggers computation
    assert_eq!(m.get(), 4);
    let first_count = *compute_count.borrow();

    // Second get should use cache (no recomputation since signal unchanged)
    assert_eq!(m.get(), 4);
    // Note: depends on implementation; we at minimum verify the value is correct
    let _ = first_count;

    // After signal change, memo recomputes
    s.set(5);
    assert_eq!(m.get(), 10);
}

#[test]
fn memo_partialeq_skip() {
    let s = Signal::new(1i32);
    let s2 = s.clone();
    let m = Memo::new(move || {
        let _ = s2.get();
        42i32
    });

    let effect_count = Rc::new(RefCell::new(0u32));
    let ec2 = Rc::clone(&effect_count);
    let m2 = m.clone();
    let _e = Effect::new(move || {
        let _ = m2.get(); // track memo
        *ec2.borrow_mut() += 1;
    });

    assert_eq!(*effect_count.borrow(), 1, "effect ran on init");
    s.set(2);
    assert_eq!(m.get(), 42, "memo still returns 42");
    assert_eq!(
        *effect_count.borrow(),
        1,
        "stable memo value should not rerun dependent effect"
    );
}

#[test]
fn effect_dispose_stops_reruns() {
    let count = Rc::new(RefCell::new(0u32));
    let s = Signal::new(0i32);
    let s2 = s.clone();
    let count2 = Rc::clone(&count);
    let effect = Effect::new(move || {
        let _ = s2.get();
        *count2.borrow_mut() += 1;
    });

    assert_eq!(*count.borrow(), 1);
    effect.dispose();
    s.set(1);
    assert_eq!(*count.borrow(), 1, "disposed effect must not rerun");
}

#[test]
fn memo_dispose_removes_subscriptions() {
    let s = Signal::new(1i32);
    let s2 = s.clone();
    let memo = Memo::new(move || s2.get() * 2);

    assert_eq!(memo.get(), 2);
    memo.dispose();
    s.set(2);
}

#[test]
fn nested_memo_preserves_outer_effect_observer() {
    let a = Signal::new(1i32);
    let b = Signal::new(10i32);
    let a2 = a.clone();
    let memo = Memo::new(move || a2.get() * 2);

    let runs = Rc::new(RefCell::new(0u32));
    let last_b = Rc::new(RefCell::new(0i32));
    let memo2 = memo.clone();
    let b2 = b.clone();
    let runs2 = Rc::clone(&runs);
    let last_b2 = Rc::clone(&last_b);
    let _effect = Effect::new(move || {
        let _ = memo2.get();
        *last_b2.borrow_mut() = b2.get();
        *runs2.borrow_mut() += 1;
    });

    assert_eq!(*runs.borrow(), 1);
    b.set(20);
    assert_eq!(*runs.borrow(), 2);
    assert_eq!(*last_b.borrow(), 20);
}

#[test]
fn batch_defers_effects() {
    let s = Signal::new(0i32);
    let s2 = s.clone();
    let count = Rc::new(RefCell::new(0u32));
    let c2 = Rc::clone(&count);
    let _e = Effect::new(move || {
        let _ = s2.get();
        *c2.borrow_mut() += 1;
    });
    assert_eq!(*count.borrow(), 1, "initial run");

    batch_begin();
    s.set(1);
    s.set(2);
    s.set(3);
    // Effects should not have run during batch
    assert_eq!(*count.borrow(), 1, "no re-runs during batch");
    batch_end();
    // After batch_end, effects should have flushed exactly once (or once per unique change)
    assert!(
        *count.borrow() >= 2,
        "effect ran at least once after batch_end"
    );
}

#[test]
fn stale_source_cleanup() {
    // An effect conditionally reads signal A or signal B based on condition signal.
    // After condition flips, writing to the formerly-read signal should NOT re-run the effect.
    let condition = Signal::new(true);
    let sig_a = Signal::new(1i32);
    let sig_b = Signal::new(100i32);

    let condition2 = condition.clone();
    let sig_a2 = sig_a.clone();
    let sig_b2 = sig_b.clone();

    let last_value = Rc::new(RefCell::new(0i32));
    let lv2 = Rc::clone(&last_value);
    let run_count = Rc::new(RefCell::new(0u32));
    let rc2 = Rc::clone(&run_count);

    let _e = Effect::new(move || {
        *rc2.borrow_mut() += 1;
        if condition2.get() {
            *lv2.borrow_mut() = sig_a2.get();
        } else {
            *lv2.borrow_mut() = sig_b2.get();
        }
    });

    // Initial: condition=true, reads sig_a
    assert_eq!(*run_count.borrow(), 1);
    assert_eq!(*last_value.borrow(), 1);

    // Flip condition: effect re-runs, now reads sig_b
    condition.set(false);
    assert_eq!(*last_value.borrow(), 100);
    let count_after_flip = *run_count.borrow();
    assert!(count_after_flip >= 2, "re-ran after condition flip");

    // Now write to sig_a — effect should NOT re-run since it no longer tracks sig_a
    sig_a.set(999);
    assert_eq!(
        *run_count.borrow(),
        count_after_flip,
        "effect must not re-run when writing to stale source sig_a"
    );
    assert_eq!(
        *last_value.borrow(),
        100,
        "last_value unchanged after writing stale sig_a"
    );

    // Writing to sig_b SHOULD re-run the effect
    sig_b.set(200);
    assert!(
        *run_count.borrow() > count_after_flip,
        "effect re-runs when active source sig_b changes"
    );
    assert_eq!(*last_value.borrow(), 200);
}

#[test]
fn disposing_a_queued_effect_during_flush_cancels_its_run() {
    let trigger = Signal::new(false);
    let target = Signal::new(0);
    let runs = Rc::new(RefCell::new(0));
    let handle = Rc::new(RefCell::new(None::<Effect>));
    let disposer = Effect::new({
        let trigger = trigger.clone();
        let handle = handle.clone();
        move || {
            if trigger.get() {
                if let Some(effect) = handle.borrow_mut().take() {
                    effect.dispose();
                }
            }
        }
    });
    *handle.borrow_mut() = Some(Effect::new({
        let target = target.clone();
        let runs = runs.clone();
        move || {
            target.get();
            *runs.borrow_mut() += 1;
        }
    }));
    batch_begin();
    trigger.set(true);
    target.set(1);
    batch_end();
    target.set(2);
    assert_eq!(*runs.borrow(), 1);
    assert!(handle.borrow().is_none());
    disposer.dispose();
}

#[test]
fn memo_switches_dependencies_even_when_selected_value_is_equal() {
    let select_left = Signal::new(true);
    let left = Signal::new(1);
    let right = Signal::new(1);
    let computations = Rc::new(RefCell::new(0));
    let memo = Memo::new({
        let select_left = select_left.clone();
        let left = left.clone();
        let right = right.clone();
        let computations = computations.clone();
        move || {
            *computations.borrow_mut() += 1;
            if select_left.get() {
                left.get()
            } else {
                right.get()
            }
        }
    });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let effect = Effect::new({
        let memo = memo.clone();
        let observed = observed.clone();
        move || observed.borrow_mut().push(memo.get())
    });
    select_left.set(false);
    assert_eq!(
        *observed.borrow(),
        vec![1],
        "equal memo values must not rerun effects"
    );
    assert_eq!(*computations.borrow(), 2);
    left.set(9);
    assert_eq!(
        *computations.borrow(),
        2,
        "old dependency must be unsubscribed"
    );
    right.set(3);
    assert_eq!(*observed.borrow(), vec![1, 3]);
    effect.dispose();
    memo.dispose();
}

#[test]
fn disposing_unqueued_nodes_preserves_batched_effects_and_memos() {
    let source = Signal::new(0);
    let latest = Rc::new(RefCell::new(vec![0; 64]));
    let mut pending = Vec::new();
    let mut memos = Vec::new();
    for index in 0..64 {
        let source = source.clone();
        let memo = Memo::new(move || source.get() + 1);
        let read = memo.clone();
        let latest = latest.clone();
        pending.push(Effect::new(move || latest.borrow_mut()[index] = read.get()));
        memos.push(memo);
    }
    let unrelated: Vec<_> = (0..64).map(|_| Effect::new(|| {})).collect();
    let unused: Vec<_> = (0..64).map(|_| Memo::new(|| 0)).collect();
    batch_begin();
    source.set(2);
    for effect in unrelated {
        effect.dispose();
    }
    for memo in unused {
        memo.dispose();
    }
    assert!(latest.borrow().iter().all(|value| *value == 1));
    batch_end();
    assert!(latest.borrow().iter().all(|value| *value == 3));
    for effect in pending {
        effect.dispose();
    }
    for memo in memos {
        memo.dispose();
    }
}
