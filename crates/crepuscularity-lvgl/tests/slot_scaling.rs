use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use std::sync::Arc;

#[test]
#[ignore = "manual scaling comparison; no wall-clock assertion"]
fn repeated_used_slots_scaling() {
    for width in [100, 400, 1000] {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("card.crepus".into(), "div\n  slot".into());
        let rows = (0..width)
            .map(|index| {
                let mut row = TemplateContext::new();
                row.set("title", format!("ROW-{index}"));
                row
            })
            .collect();
        ctx.set("rows", TemplateValue::List(rows));
        let source = "for row in rows\n  include card.crepus\n    span \"{row.title}\"";
        let nodes = parse_template(source).unwrap();
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let output = render(source, &nodes, &ctx);
            samples.push(start.elapsed().as_nanos());
            assert!(output.contains(&format!("ROW-{}", width - 1)));
            std::hint::black_box(output);
        }
        samples.sort_unstable();
        println!("width={width} median_ns={}", samples[2]);
    }
}

fn render(
    source: &str,
    _nodes: &[crepuscularity_core::ast::Node],
    ctx: &TemplateContext,
) -> String {
    crepuscularity_lvgl::render_template_to_lvgl_xml(source, ctx).unwrap()
}
