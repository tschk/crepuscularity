use crepuscularity_reactive::Signal;
use std::cell::RefCell;
use std::rc::Rc;

type Hook = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;
fn invoke(hook: &Hook) {
    let callback = hook.borrow_mut().take();
    if let Some(callback) = callback {
        callback();
    }
}
#[derive(Default)]
struct Hooks {
    clone: Hook,
    equal: Hook,
    drop: Hook,
}
struct Value {
    number: i32,
    hooks: Rc<Hooks>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        invoke(&self.hooks.clone);
        Self {
            number: self.number,
            hooks: self.hooks.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        invoke(&self.hooks.equal);
        self.number == other.number
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        invoke(&self.hooks.drop);
    }
}
fn value(number: i32, hooks: &Rc<Hooks>) -> Value {
    Value {
        number,
        hooks: hooks.clone(),
    }
}
fn arm(hook: &Hook, callback: impl FnOnce() + 'static) {
    *hook.borrow_mut() = Some(Box::new(callback));
}

#[test]
fn equality_can_read_the_same_signal() {
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let captured = signal.clone();
    arm(&hooks.equal, move || assert_eq!(captured.get().number, 1));
    signal.set(value(2, &hooks));
    assert_eq!(signal.get().number, 2);
}

#[test]
fn outer_set_completes_after_nested_equality_writes_even_if_snapshot_equal() {
    for requested in [1, 2] {
        let hooks = Rc::new(Hooks::default());
        let signal = Signal::new(value(1, &hooks));
        let captured = signal.clone();
        let nested_hooks = hooks.clone();
        arm(&hooks.equal, move || {
            captured.set(value(9, &nested_hooks));
            assert_eq!(captured.get().number, 9);
        });
        signal.set(value(requested, &hooks));
        assert_eq!(signal.get().number, requested);
    }
}

#[test]
fn get_clone_callback_can_write_without_changing_its_snapshot() {
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let captured = signal.clone();
    let nested_hooks = hooks.clone();
    arm(&hooks.clone, move || captured.set(value(7, &nested_hooks)));
    assert_eq!(signal.get().number, 1);
    assert_eq!(signal.get().number, 7);
}

#[test]
fn update_clone_callback_can_write_and_outer_update_still_completes() {
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let captured = signal.clone();
    let nested_hooks = hooks.clone();
    arm(&hooks.clone, move || captured.set(value(7, &nested_hooks)));
    signal.update(|previous| {
        assert_eq!(previous.number, 1);
        value(previous.number + 1, &hooks)
    });
    assert_eq!(signal.get().number, 2);
}

#[test]
fn displaced_drop_can_read_and_write_the_committed_signal() {
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let captured = signal.clone();
    let nested_hooks = hooks.clone();
    arm(&hooks.drop, move || {
        assert_eq!(captured.get().number, 2);
        captured.set(value(8, &nested_hooks));
    });
    signal.set(value(2, &hooks));
    assert_eq!(signal.get().number, 8);
}

#[test]
fn panicking_comparison_preserves_value_and_future_updates() {
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    arm(&hooks.equal, || panic!("comparison failed"));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        signal.set(value(2, &hooks));
    }));
    assert!(result.is_err());
    assert_eq!(signal.get().number, 1);
    signal.set(value(3, &hooks));
    assert_eq!(signal.get().number, 3);
}

#[test]
fn panicking_displaced_drop_leaves_committed_value_pending_for_effects() {
    use crepuscularity_reactive::{flush, Effect};
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let captured = signal.clone();
    let output = observed.clone();
    let effect = Effect::new(move || output.borrow_mut().push(captured.get().number));
    arm(&hooks.drop, || panic!("displaced value drop failed"));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        signal.set(value(2, &hooks));
    }));
    assert!(result.is_err());
    assert_eq!(signal.get().number, 2);
    flush();
    assert_eq!(*observed.borrow(), vec![1, 2]);
    effect.dispose();
}

#[test]
fn update_snapshot_clone_does_not_subscribe_the_active_effect() {
    use crepuscularity_reactive::Effect;
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let runs = Rc::new(RefCell::new(0));
    let count = runs.clone();
    let captured = signal.clone();
    let effect = Effect::new(move || {
        *count.borrow_mut() += 1;
        captured.update(|previous| previous);
    });
    assert_eq!(*runs.borrow(), 1);
    signal.set(value(2, &hooks));
    assert_eq!(
        *runs.borrow(),
        1,
        "update must not track its internal snapshot read"
    );
    effect.dispose();
}

#[test]
fn clone_write_during_memo_read_does_not_leave_its_snapshot_cached() {
    use crepuscularity_reactive::{flush, Effect, Memo};
    let hooks = Rc::new(Hooks::default());
    let signal = Signal::new(value(1, &hooks));
    let captured = signal.clone();
    let memo = Memo::new(move || captured.get().number);
    let written = signal.clone();
    let nested_hooks = hooks.clone();
    arm(&hooks.clone, move || written.set(value(7, &nested_hooks)));
    // The in-flight read may finish its original snapshot. Its invalidation
    // must survive that completion, rather than caching the stale value forever.
    let first = memo.get();
    assert!([1, 7].contains(&first));
    assert_eq!(signal.get().number, 7);
    assert_eq!(memo.get(), 7);
    memo.dispose();

    // Also start the first memo computation under an active effect, so its
    // queued invalidation must reach the observer rather than only a manual read.
    let captured = signal.clone();
    let memo = Memo::new(move || captured.get().number);
    let written = signal.clone();
    let nested_hooks = hooks.clone();
    arm(&hooks.clone, move || written.set(value(11, &nested_hooks)));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let output = observed.clone();
    let read = memo.clone();
    let effect = Effect::new(move || output.borrow_mut().push(read.get()));
    flush();
    assert_eq!(signal.get().number, 11);
    assert_eq!(observed.borrow().last(), Some(&11));
    effect.dispose();
    memo.dispose();
}
