use crepuscularity_reactive::{Effect, Memo, Signal};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn effect_panic_before_read_preserves_subscription_for_retry() {
    let source = Signal::new(0);
    let fail = Rc::new(Cell::new(false));
    let observed = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let source = source.clone();
        let fail = fail.clone();
        let observed = observed.clone();
        move || {
            assert!(!fail.get(), "callback failed before reading");
            observed.set(source.get());
        }
    });
    fail.set(true);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.set(1))).is_err());
    fail.set(false);
    source.set(2);
    assert_eq!(observed.get(), 2);
    effect.dispose();
}

#[test]
fn memo_panic_before_read_preserves_subscription_for_retry() {
    let source = Signal::new(0);
    let fail = Rc::new(Cell::new(false));
    let memo = Memo::new({
        let source = source.clone();
        let fail = fail.clone();
        move || {
            assert!(!fail.get(), "computation failed before reading");
            source.get() * 2
        }
    });
    let observed = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let memo = memo.clone();
        let observed = observed.clone();
        move || observed.set(memo.get())
    });
    fail.set(true);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.set(1))).is_err());
    fail.set(false);
    source.set(2);
    assert_eq!(observed.get(), 4);
    effect.dispose();
    memo.dispose();
}

#[test]
fn failed_partial_reads_are_retained_until_the_next_successful_run() {
    let old = Signal::new(0);
    let new = Signal::new(0);
    let phase = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let old = old.clone();
        let new = new.clone();
        let phase = phase.clone();
        let runs = runs.clone();
        move || {
            runs.set(runs.get() + 1);
            if phase.get() == 0 {
                old.get();
            } else {
                new.get();
                assert_ne!(phase.get(), 1, "failed after a new dependency read");
            }
        }
    });
    phase.set(1);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| old.set(1))).is_err());
    phase.set(2);
    new.set(1);
    assert_eq!(runs.get(), 3, "new partial subscription was lost");
    old.set(2);
    assert_eq!(
        runs.get(),
        3,
        "successful run retained the old subscription"
    );
    effect.dispose();
}

#[test]
fn disposal_during_a_failed_callback_does_not_restore_the_observer() {
    let source = Signal::new(0);
    let owner = Rc::new(std::cell::RefCell::new(None::<Effect>));
    let runs = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let source = source.clone();
        let owner = owner.clone();
        let runs = runs.clone();
        move || {
            runs.set(runs.get() + 1);
            if let Some(effect) = owner.borrow_mut().take() {
                effect.dispose();
                panic!("disposed before reading");
            }
            source.get();
        }
    });
    *owner.borrow_mut() = Some(effect);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.set(1))).is_err());
    source.set(2);
    assert_eq!(runs.get(), 2, "disposed observer was restored");
}
