use crepuscularity_core::context::{TemplateContext, TemplateValue};
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use std::hint::black_box;

fn bench_for_loop_context(c: &mut Criterion) {
    let mut base_ctx = TemplateContext::default();
    // Add some other variables to make cloning realistic
    for j in 0..50 {
        base_ctx
            .vars
            .insert(format!("var{}", j), TemplateValue::Str(format!("val{}", j)));
    }

    let mut list = Vec::new();
    for i in 0..100 {
        let mut item = TemplateContext::default();
        item.vars.insert(
            "value".to_string(),
            TemplateValue::Str(format!("item{}", i)),
        );
        list.push(item);
    }
    base_ctx
        .vars
        .insert("items".to_string(), TemplateValue::List(list));

    // Baseline: The original code that clones the entire context inside the loop
    c.bench_function("baseline_for_loop", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            for item_ctx in base_ctx.get_list_ref("items") {
                let mut child_ctx = base_ctx.clone();
                child_ctx.vars.extend(item_ctx.vars.clone());
                let item_str = item_ctx.get_str("value");
                if !item_str.is_empty() {
                    child_ctx
                        .vars
                        .insert("item".to_string(), TemplateValue::Str(item_str));
                }
                // we clone here to simulate the context being passed into an inner function that takes ownership,
                // or just evaluating it as part of rendering
                out.push(child_ctx.get_str("item"));
            }
            black_box(out);
        })
    });

    // Optimized: using a `written` vector to incrementally modify and restore the context
    c.bench_function("optimized_for_loop", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            let mut child_ctx = base_ctx.clone();
            let mut written: Vec<String> = Vec::new();

            for item_ctx in base_ctx.get_list_ref("items") {
                // Restore child_ctx
                for key in written.drain(..) {
                    match base_ctx.vars.get(&key) {
                        Some(v) => {
                            child_ctx.vars.insert(key, v.clone());
                        }
                        None => {
                            child_ctx.vars.remove(&key);
                        }
                    }
                }

                // Merge all fields from the item context into the child context.
                for (k, v) in &item_ctx.vars {
                    child_ctx.vars.insert(k.clone(), v.clone());
                    written.push(k.clone());
                }

                let item_str = item_ctx.get_str("value");
                if !item_str.is_empty() {
                    child_ctx
                        .vars
                        .insert("item".to_string(), TemplateValue::Str(item_str));
                    written.push("item".to_string());
                }

                // simulate the inner function evaluation
                out.push(child_ctx.get_str("item"));
            }
            black_box(out);
        })
    });
}

criterion_group!(benches, bench_for_loop_context);
criterion_main!(benches);
