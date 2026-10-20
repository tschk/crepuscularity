use crepuscularity_reactive::{batch_begin, batch_end, Memo, Signal};
use std::{cell::RefCell, rc::Rc};

type Hook = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;
fn invoke(hook: &Hook) {
    let callback = hook.borrow_mut().take();
    if let Some(callback) = callback {
        callback();
    }
}
struct Value {
    number: i32,
    clone: Hook,
    equal: Hook,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        invoke(&self.clone);
        Self {
            number: self.number,
            clone: self.clone.clone(),
            equal: self.equal.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        invoke(&self.equal);
        self.number == other.number
    }
}
fn value(number: i32, clone: &Hook, equal: &Hook) -> Value {
    Value {
        number,
        clone: clone.clone(),
        equal: equal.clone(),
    }
}

#[test]
fn signal_clone_write_cannot_leave_a_memo_cache_stale() {
    let clone: Hook = Default::default();
    let equal: Hook = Default::default();
    let signal = Signal::new(value(1, &clone, &equal));
    let captured = signal.clone();
    let memo = Memo::new(move || captured.get().number);
    let captured = signal.clone();
    let (c, e) = (clone.clone(), equal.clone());
    *clone.borrow_mut() = Some(Box::new(move || captured.set(value(7, &c, &e))));
    assert_eq!(memo.get(), 7);
    assert_eq!(memo.get(), signal.get().number);
    memo.dispose();
}

#[test]
fn outer_comparison_does_not_overwrite_a_fresher_nested_commit() {
    let source = Signal::new(1);
    let clone: Hook = Default::default();
    let equal: Hook = Default::default();
    let captured = source.clone();
    let (c, e) = (clone.clone(), equal.clone());
    let memo = Memo::new(move || value(captured.get(), &c, &e));
    assert_eq!(memo.get().number, 1);
    batch_begin();
    source.set(2);
    let captured = source.clone();
    *equal.borrow_mut() = Some(Box::new(move || {
        captured.set(3);
        // Ends the batch during comparison, synchronously recomputing this memo.
        batch_end();
    }));
    assert_eq!(memo.get().number, 3);
    assert_eq!(memo.get().number, 3);
    source.set(4);
    assert_eq!(memo.get().number, 4, "nested subscriptions remain live");
    memo.dispose();
}

#[test]
fn comparison_panic_after_invalidation_remains_recoverable() {
    let source = Signal::new(1);
    let clone: Hook = Default::default();
    let equal: Hook = Default::default();
    let captured = source.clone();
    let (c, e) = (clone.clone(), equal.clone());
    let memo = Memo::new(move || value(captured.get(), &c, &e));
    assert_eq!(memo.get().number, 1);
    let read = memo.clone();
    let mut chain = vec![Memo::new(move || read.get().number)];
    for _ in 0..32 {
        let read = chain.last().unwrap().clone();
        chain.push(Memo::new(move || read.get() + 1));
    }
    let top = chain.last().unwrap();
    assert_eq!(top.get(), 33);
    batch_begin();
    source.set(2);
    let captured = source.clone();
    *equal.borrow_mut() = Some(Box::new(move || {
        captured.set(3);
        panic!("comparison failed");
    }));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| top.get())).is_err());
    assert_eq!(
        top.get(),
        35,
        "validation can resume before the batch flushes"
    );
    batch_end();
    assert_eq!(memo.get().number, 3);
    assert_eq!(top.get(), 35);
    source.set(4);
    assert_eq!(memo.get().number, 4);
    assert_eq!(top.get(), 36);
    for memo in chain.into_iter().rev() {
        memo.dispose();
    }
    memo.dispose();
}

#[test]
fn clone_write_reaches_active_descendants_through_an_already_dirty_chain() {
    let clone: Hook = Default::default();
    let equal: Hook = Default::default();
    let signal = Signal::new(value(1, &clone, &equal));
    let captured = signal.clone();
    let root = Memo::new(move || captured.get().number);
    let captured = root.clone();
    let middle = Memo::new(move || captured.get() * 2);
    let captured = middle.clone();
    let top = Memo::new(move || captured.get() + 1);
    assert_eq!(top.get(), 3);
    batch_begin();
    signal.set(value(2, &clone, &equal));
    let captured = signal.clone();
    let (c, e) = (clone.clone(), equal.clone());
    *clone.borrow_mut() = Some(Box::new(move || captured.set(value(7, &c, &e))));
    assert_eq!(top.get(), 15);
    batch_end();
    assert_eq!(top.get(), 15);
    top.dispose();
    middle.dispose();
    root.dispose();
}

#[test]
fn check_validation_survives_reentrant_writes_and_preserves_equality_suppression() {
    let source = Signal::new(1);
    let clone: Hook = Default::default();
    let equal: Hook = Default::default();
    let captured = source.clone();
    let (c, e) = (clone.clone(), equal.clone());
    let root = Memo::new(move || value(captured.get(), &c, &e));
    let captured = root.clone();
    let parity = Memo::new(move || captured.get().number % 2);
    let captured = parity.clone();
    let runs = Rc::new(std::cell::Cell::new(0));
    let count = runs.clone();
    let top = Memo::new(move || {
        count.set(count.get() + 1);
        captured.get() * 10
    });
    assert_eq!(top.get(), 10);
    batch_begin();
    source.set(3);
    let captured = source.clone();
    *equal.borrow_mut() = Some(Box::new(move || captured.set(5)));
    assert_eq!(top.get(), 10);
    batch_end();
    assert_eq!(source.get(), 5);
    assert_eq!(root.get().number, 5);
    assert_eq!(
        runs.get(),
        1,
        "unchanged parity does not rerun its dependent"
    );
    top.dispose();
    parity.dispose();
    root.dispose();
}

#[test]
fn displaced_cached_value_drop_cannot_leave_a_stale_memo() {
    #[derive(Clone)]
    struct DroppingValue(i32, Hook);
    impl PartialEq for DroppingValue {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Drop for DroppingValue {
        fn drop(&mut self) {
            invoke(&self.1);
        }
    }
    let source = Signal::new(1);
    let hook: Hook = Default::default();
    let captured = source.clone();
    let drop_hook = hook.clone();
    let memo = Memo::new(move || DroppingValue(captured.get(), drop_hook.clone()));
    assert_eq!(memo.get().0, 1);
    batch_begin();
    source.set(2);
    let captured = source.clone();
    *hook.borrow_mut() = Some(Box::new(move || captured.set(3)));
    assert_eq!(memo.get().0, 3);
    batch_end();
    assert_eq!(memo.get().0, 3);
    memo.dispose();
}

#[test]
fn deep_check_chain_reads_fresh_inside_batches_for_changed_and_equal_values() {
    let source = Signal::new(0);
    let captured = source.clone();
    let mut chain = vec![Memo::new(move || captured.get() / 2)];
    for _ in 1..80 {
        let captured = chain.last().unwrap().clone();
        chain.push(Memo::new(move || captured.get() + 1));
    }
    assert_eq!(chain.last().unwrap().get(), 79);
    for (input, expected) in [(1, 79), (2, 80), (3, 80), (4, 81)] {
        batch_begin();
        source.set(input);
        assert_eq!(chain.last().unwrap().get(), expected);
        batch_end();
        assert_eq!(chain.last().unwrap().get(), expected);
    }
    for memo in chain.into_iter().rev() {
        memo.dispose();
    }
}
