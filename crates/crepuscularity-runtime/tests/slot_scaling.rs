use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_runtime::renderer::render_nodes;
use std::sync::Arc;

#[test]
#[ignore = "manual construction timing; no wall-clock assertion or painting"]
fn repeated_used_slots_scaling() {
    for width in [100, 400, 1000] {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("card.crepus".into(), "slot".into());
        let rows = (0..width)
            .map(|index| {
                let mut row = TemplateContext::new();
                row.set("title", format!("ROW-{index}"));
                row
            })
            .collect();
        ctx.set("rows", TemplateValue::List(rows));
        let nodes = parse_template(
            "div #benchmark\n  for row in rows\n    include card.crepus\n      div id={row.title}",
        )
        .unwrap();
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let mut output = render_nodes(&nodes, &ctx);
            samples.push(start.elapsed().as_nanos());
            let root = output.downcast_mut::<gpui::Stateful<gpui::Div>>().unwrap();
            assert_eq!(
                gpui::Element::id(root),
                Some(gpui::ElementId::Name("benchmark".into()))
            );
            std::hint::black_box(output);
        }
        samples.sort_unstable();
        println!("width={width} median_ns={}", samples[2]);
    }
}
