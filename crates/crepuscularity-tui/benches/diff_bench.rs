use crepuscularity_core::TemplateContext;
use crepuscularity_tui::diff::DiffTracker;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_diff_tracker(c: &mut Criterion) {
    let mut ctx = TemplateContext::new();
    for i in 0..1000 {
        ctx.set(
            format!("key_very_long_string_to_avoid_inline_{}", i),
            i as i64,
        );
    }

    let mut tracker = DiffTracker::new();
    tracker.update(&ctx);

    c.bench_function("diff_tracker_has_changed", |b| {
        b.iter(|| tracker.has_changed(black_box(&ctx)))
    });

    c.bench_function("diff_tracker_update", |b| {
        b.iter(|| {
            let mut t = tracker.clone();
            let mut new_ctx = ctx.clone();
            new_ctx.set("key_very_long_string_to_avoid_inline_500", 9999);
            t.update(black_box(&new_ctx))
        })
    });
}

criterion_group!(benches, bench_diff_tracker);
criterion_main!(benches);
