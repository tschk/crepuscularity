use crepuscularity_core::context::*;
use std::time::Instant;

#[test]
fn bench_perf() {
    let mut ctx = TemplateContext::new();
    let mut items = vec![];
    for i in 0..5000 {
        let mut item_ctx = TemplateContext::new();
        item_ctx
            .vars
            .insert("value".to_string(), TemplateValue::Str(i.to_string()));
        items.push(item_ctx);
    }
    ctx.vars
        .insert("items".to_string(), TemplateValue::List(items));

    let start = Instant::now();
    let ctx_clone = ctx.clone();

    // Baseline logic
    for item_ctx in ctx_clone.get_list_ref("items") {
        let mut child_ctx = ctx_clone.clone();
        child_ctx.vars.extend(item_ctx.vars.clone());
        let item_str = item_ctx.get_str("value");
        if !item_str.is_empty() {
            child_ctx
                .vars
                .insert("item".to_string(), TemplateValue::Str(item_str));
        }
    }

    let baseline_duration = start.elapsed();
    println!("Baseline clone overhead time: {:?}", baseline_duration);

    let start_opt = Instant::now();
    let mut child_ctx = ctx.clone();
    let mut written: Vec<String> = Vec::new();

    // Optimized logic
    for item_ctx in ctx.get_list_ref("items") {
        for key in written.drain(..) {
            match ctx.vars.get(&key) {
                Some(v) => {
                    child_ctx.vars.insert(key, v.clone());
                }
                None => {
                    child_ctx.vars.remove(&key);
                }
            }
        }
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
    }

    let optimized_duration = start_opt.elapsed();
    println!("Optimized clone overhead time: {:?}", optimized_duration);
}
