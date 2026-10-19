use crepuscularity_core::ast::{ForBlock, Node};
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn baseline_focus(nodes: &[Node], ctx: &TemplateContext, out: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::For(block) => {
                for item_ctx in ctx.get_list_ref(&block.iterator) {
                    let mut child_ctx = ctx.clone();
                    child_ctx.vars.extend(item_ctx.vars.clone());
                    if !block.pattern.trim().is_empty() {
                        let item_str = item_ctx.get_str("value");
                        if !item_str.is_empty() {
                            child_ctx.vars.insert(
                                block.pattern.trim().to_string(),
                                TemplateValue::Str(item_str),
                            );
                        }
                    }
                    baseline_focus(&block.body, &child_ctx, out);
                }
            }
            _ => {}
        }
    }
}

fn optimized_focus(nodes: &[Node], ctx: &TemplateContext, out: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::For(block) => {
                let pattern = block.pattern.trim();
                let has_pattern = !pattern.is_empty();

                let mut child_ctx = ctx.clone();
                let mut written: Vec<String> = Vec::new();

                for item_ctx in ctx.get_list_ref(&block.iterator) {
                    // Restore child_ctx
                    for key in written.drain(..) {
                        if let Some(v) = ctx.vars.get(&key) {
                            child_ctx.vars.insert(key, v.clone());
                        } else {
                            child_ctx.vars.remove(&key);
                        }
                    }

                    for (k, v) in &item_ctx.vars {
                        child_ctx.vars.insert(k.clone(), v.clone());
                        written.push(k.clone());
                    }

                    if has_pattern {
                        let item_str = item_ctx.get_str("value");
                        if !item_str.is_empty() {
                            child_ctx
                                .vars
                                .insert(pattern.to_string(), TemplateValue::Str(item_str));
                            written.push(pattern.to_string());
                        }
                    }

                    optimized_focus(&block.body, &child_ctx, out);
                }
            }
            _ => {}
        }
    }
}

fn bench_focus(c: &mut Criterion) {
    let mut ctx = TemplateContext::default();
    let mut items = Vec::new();
    for i in 0..100 {
        let mut item = TemplateContext::default();
        item.vars
            .insert("id".to_string(), TemplateValue::Str(format!("item-{}", i)));
        item.vars.insert(
            "value".to_string(),
            TemplateValue::Str(format!("val-{}", i)),
        );
        items.push(item);
    }
    ctx.vars
        .insert("items".to_string(), TemplateValue::List(items));

    // Fill ctx with some variables to make cloning expensive
    for i in 0..100 {
        ctx.vars
            .insert(format!("var{}", i), TemplateValue::Str(format!("val{}", i)));
    }

    let nodes = vec![Node::For(ForBlock {
        iterator: "items".to_string(),
        pattern: "item".to_string(),
        body: vec![],
    })];

    c.bench_function("focus_baseline", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            baseline_focus(&nodes, &ctx, &mut out);
            black_box(out);
        })
    });

    c.bench_function("focus_optimized", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            optimized_focus(&nodes, &ctx, &mut out);
            black_box(out);
        })
    });
}

criterion_group!(benches, bench_focus);
criterion_main!(benches);
