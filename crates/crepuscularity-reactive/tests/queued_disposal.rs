use crepuscularity_reactive::{batch_begin, batch_end, Effect, Signal};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

#[test]
#[ignore = "manual release-mode queued teardown measurement"]
fn queued_effect_disposal_scaling() {
    for count in [4_000, 8_000] {
        let mut samples = Vec::new();
        for sample in 0..10 {
            let signal = Signal::new(0);
            let runs = Rc::new(Cell::new(0));
            let effects: Vec<_> = (0..count)
                .map(|_| {
                    let signal = signal.clone();
                    let runs = Rc::clone(&runs);
                    Effect::new(move || {
                        signal.get();
                        runs.set(runs.get() + 1);
                    })
                })
                .collect();
            batch_begin();
            signal.set(1);
            let start = Instant::now();
            for effect in effects {
                effect.dispose();
            }
            batch_end();
            let elapsed = start.elapsed();
            assert_eq!(runs.get(), count);
            if sample >= 3 {
                samples.push(elapsed);
            }
        }
        samples.sort();
        eprintln!(
            "queued disposal {count}: median {:?}",
            samples[samples.len() / 2]
        );
    }
}
