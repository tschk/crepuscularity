use crepuscularity_reactive::{Effect, Memo, Signal};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn wide_observer_deduplicates_reads_and_replaces_subscriptions() {
    let signals: Vec<_> = (0..1024).map(|_| Signal::new(1usize)).collect();
    let alternate = Signal::new(false);
    let runs = Rc::new(Cell::new(0));
    let read = signals.clone();
    let branch = alternate.clone();
    let memo = Memo::new(move || {
        let offset = usize::from(branch.get());
        read.iter()
            .skip(offset)
            .step_by(2)
            .map(|s| s.get() + s.get())
            .sum::<usize>()
    });
    let observed = memo.clone();
    let count = runs.clone();
    let effect = Effect::new(move || {
        observed.get();
        count.set(count.get() + 1);
    });
    assert_eq!(runs.get(), 1);
    alternate.set(true);
    signals[0].set(20);
    assert_eq!(
        runs.get(),
        1,
        "equal memo values should suppress downstream effects"
    );
    signals[1].set(2);
    assert_eq!(memo.get(), 1026);
    assert_eq!(runs.get(), 2);
    effect.dispose();
    memo.dispose();
}

#[test]
#[ignore = "manual timing comparison; no wall-clock assertion"]
fn wide_memo_tracking_benchmark() {
    for width in [1000, 4000, 8000] {
        let signals: Vec<_> = (0..width).map(|_| Signal::new(1usize)).collect();
        let read = signals.clone();
        let memo = Memo::new(move || read.iter().map(Signal::get).sum::<usize>());
        assert_eq!(memo.get(), width);
        let mut samples = Vec::new();
        for round in 0..7 {
            let start = std::time::Instant::now();
            signals[0].set(round + 2);
            assert_eq!(std::hint::black_box(memo.get()), width + round + 1);
            samples.push(start.elapsed().as_nanos());
        }
        samples.sort_unstable();
        println!("width={width} median_ns={}", samples[3]);
        memo.dispose();
    }
}
