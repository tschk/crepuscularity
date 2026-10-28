use crepuscularity_reactive::{batch_begin, batch_end, flush, Effect, Memo, Signal};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
fn counted(source: Signal<i32>, runs: Rc<Cell<usize>>, factor: i32) -> Memo<i32> {
    Memo::new(move || {
        runs.set(runs.get() + 1);
        source.get() * factor
    })
}
#[test]
fn thousand_writes_coalesce_memos_and_effects() {
    let s = Signal::new(0);
    let r = Rc::new(Cell::new(0));
    let m = counted(s.clone(), r.clone(), 2);
    let e = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let m = m.clone();
        let e = e.clone();
        move || {
            m.get();
            e.set(e.get() + 1)
        }
    });
    batch_begin();
    for i in 1..=1000 {
        s.set(i)
    }
    assert_eq!(r.get(), 1);
    assert_eq!(e.get(), 1);
    batch_end();
    assert_eq!(r.get(), 2);
    assert_eq!(e.get(), 2);
    assert_eq!(m.get(), 2000);
    effect.dispose();
    m.dispose();
}
#[test]
fn diamond_has_one_final_computation_and_no_intermediate_glitch() {
    let s = Signal::new(0);
    let lr = Rc::new(Cell::new(0));
    let rr = Rc::new(Cell::new(0));
    let br = Rc::new(Cell::new(0));
    let left = counted(s.clone(), lr.clone(), 2);
    let right = counted(s.clone(), rr.clone(), 3);
    let bottom = Memo::new({
        let l = left.clone();
        let r = right.clone();
        let n = br.clone();
        move || {
            n.set(n.get() + 1);
            let a = l.get();
            let b = r.get();
            assert_eq!(a * 3, b * 2);
            a + b
        }
    });
    let values = Rc::new(RefCell::new(vec![]));
    let effect = Effect::new({
        let b = bottom.clone();
        let v = values.clone();
        move || v.borrow_mut().push(b.get())
    });
    batch_begin();
    for i in 1..=1000 {
        s.set(i)
    }
    batch_end();
    assert_eq!((lr.get(), rr.get(), br.get()), (2, 2, 2));
    assert_eq!(*values.borrow(), vec![0, 5000]);
    effect.dispose();
    bottom.dispose();
    left.dispose();
    right.dispose();
}
#[test]
fn stable_intermediate_skips_descendant_memos_and_effects() {
    let s = Signal::new(0);
    let rootruns = Rc::new(Cell::new(0));
    let leafruns = Rc::new(Cell::new(0));
    let root = Memo::new({
        let s = s.clone();
        let n = rootruns.clone();
        move || {
            n.set(n.get() + 1);
            s.get() % 2
        }
    });
    let leaf = Memo::new({
        let m = root.clone();
        let n = leafruns.clone();
        move || {
            n.set(n.get() + 1);
            m.get() + 10
        }
    });
    let er = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let m = leaf.clone();
        let n = er.clone();
        move || {
            m.get();
            n.set(n.get() + 1)
        }
    });
    batch_begin();
    for i in 1..=1000 {
        s.set(i * 2)
    }
    batch_end();
    assert_eq!(rootruns.get(), 2);
    assert_eq!(leaf.get(), 10);
    assert_eq!(leafruns.get(), 1);
    assert_eq!(er.get(), 1);
    effect.dispose();
    leaf.dispose();
    root.dispose();
}
#[test]
fn nested_batches_and_reads_are_fresh_without_early_effects() {
    let s = Signal::new(0);
    let n = Rc::new(Cell::new(0));
    let m = counted(s.clone(), n.clone(), 5);
    let values = Rc::new(RefCell::new(vec![]));
    let effect = Effect::new({
        let m = m.clone();
        let v = values.clone();
        move || v.borrow_mut().push(m.get())
    });
    batch_begin();
    s.set(1);
    assert_eq!(m.get(), 5);
    batch_begin();
    s.set(2);
    s.set(3);
    assert_eq!(m.get(), 15);
    batch_end();
    assert_eq!(*values.borrow(), vec![0]);
    assert_eq!(n.get(), 3);
    batch_end();
    assert_eq!(*values.borrow(), vec![0, 15]);
    assert_eq!(n.get(), 3);
    effect.dispose();
    m.dispose();
}
#[test]
fn equal_dynamic_dependency_switch_removes_old_source() {
    let flag = Signal::new(true);
    let first = Signal::new(1);
    let second = Signal::new(1);
    let n = Rc::new(Cell::new(0));
    let m = Memo::new({
        let f = flag.clone();
        let a = first.clone();
        let b = second.clone();
        let n = n.clone();
        move || {
            n.set(n.get() + 1);
            if f.get() {
                a.get()
            } else {
                b.get()
            }
        }
    });
    let values = Rc::new(RefCell::new(vec![]));
    let effect = Effect::new({
        let m = m.clone();
        let v = values.clone();
        move || v.borrow_mut().push(m.get())
    });
    batch_begin();
    flag.set(false);
    first.set(99);
    batch_end();
    assert_eq!(*values.borrow(), vec![1]);
    assert_eq!(n.get(), 2);
    first.set(100);
    assert_eq!(n.get(), 2);
    second.set(2);
    assert_eq!(*values.borrow(), vec![1, 2]);
    effect.dispose();
    m.dispose();
}
#[test]
fn cached_unobserved_memos_keep_eager_after_batch_behavior() {
    let source = Signal::new(0);
    let runs = Rc::new(Cell::new(0));
    let memo = counted(source.clone(), runs.clone(), 2);
    assert_eq!(memo.get(), 0);
    batch_begin();
    source.set(1);
    source.set(2);
    assert_eq!(runs.get(), 1);
    batch_end();
    assert_eq!(runs.get(), 2);
    source.set(3);
    assert_eq!(runs.get(), 3);
    assert_eq!(memo.get(), 6);
    memo.dispose();
}
#[test]
fn disposing_queued_memos_removes_pending_work() {
    let source = Signal::new(0);
    let runs = Rc::new(Cell::new(0));
    let memo = counted(source.clone(), runs.clone(), 2);
    memo.get();
    batch_begin();
    source.set(1);
    memo.dispose();
    batch_end();
    assert_eq!(runs.get(), 1);
}
#[test]
fn panic_restores_observer_and_flush_and_preserves_queued_peers() {
    let s = Signal::new(0);
    let m = Memo::new({
        let s = s.clone();
        move || {
            let v = s.get();
            assert_ne!(v, 1, "memo failure");
            v * 2
        }
    });
    let seen = Rc::new(Cell::new(0));
    let effect = Effect::new({
        let m = m.clone();
        let v = seen.clone();
        move || v.set(m.get())
    });
    let other = Signal::new(0);
    let otherseen = Rc::new(Cell::new(0));
    let peer = Effect::new({
        let s = other.clone();
        let v = otherseen.clone();
        move || v.set(s.get())
    });
    batch_begin();
    s.set(1);
    other.set(2);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(batch_end)).is_err());
    flush();
    assert_eq!(otherseen.get(), 2);
    s.set(2);
    assert_eq!(seen.get(), 4);
    effect.dispose();
    peer.dispose();
    m.dispose();
}

#[test]
fn wide_fanout_coalesces_updates_and_stays_current_after_flushing() {
    let source = Signal::new(0);
    let runs: Vec<_> = (0..128).map(|_| Rc::new(Cell::new(0))).collect();
    let memos: Vec<_> = runs
        .iter()
        .enumerate()
        .map(|(index, runs)| {
            let source = source.clone();
            let runs = runs.clone();
            Memo::new(move || {
                runs.set(runs.get() + 1);
                source.get() + index
            })
        })
        .collect();
    for memo in &memos {
        memo.get();
    }
    batch_begin();
    for value in 1..=64 {
        source.set(value);
    }
    assert!(runs.iter().all(|runs| runs.get() == 1));
    batch_end();
    assert!(runs.iter().all(|runs| runs.get() == 2));
    for (index, memo) in memos.iter().enumerate() {
        assert_eq!(memo.get(), 64 + index);
    }
    source.set(65);
    assert!(runs.iter().all(|runs| runs.get() == 3));
    for (index, memo) in memos.into_iter().enumerate() {
        assert_eq!(memo.get(), 65 + index);
        memo.dispose();
    }
}
