//! Manual cumulative-allocation probe, scoped to the calling test thread.
use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_tui::{BufferRenderer, Template};
use ratatui::{buffer::Buffer, layout::Rect};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

struct CountingAllocator;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}
fn record(bytes: usize) {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = COUNTS.try_with(|counts| {
                let (calls, total) = counts.get();
                counts.set((calls + 1, total + bytes));
            });
        }
    });
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = System.alloc(layout);
        if !result.is_null() {
            record(layout.size());
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = System.alloc_zeroed(layout);
        if !result.is_null() {
            record(layout.size());
        }
        result
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, size);
        if !result.is_null() {
            record(size);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure<T>(run: impl FnOnce() -> T) -> (T, (usize, usize)) {
    COUNTS.with(|counts| counts.set((0, 0)));
    ENABLED.with(|enabled| enabled.set(true));
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(false));
        }
    }
    let reset = Reset;
    let result = run();
    drop(reset);
    (result, COUNTS.with(Cell::get))
}

#[test]
#[ignore = "manual cumulative-allocation scaling comparison; no latency assertion"]
fn repeated_forwarded_slots_render_and_focus_allocations() {
    let source = "for row in rows\n  include outer.crepus prefix=\"CALLEE\"\n    span id={row.id} focusable h-[1]\n      \"{prefix}:{row.title}\"";
    let nodes = parse_template(source).unwrap();
    for rows in [100u16, 200, 400] {
        let mut ctx = TemplateContext::new();
        ctx.set("prefix", "CALLER");
        let files = Arc::make_mut(&mut ctx.virtual_files);
        files.insert("outer.crepus".into(), "include card.crepus\n  slot".into());
        files.insert("card.crepus".into(), "slot".into());
        ctx.set(
            "rows",
            TemplateValue::List(
                (0..rows)
                    .map(|index| {
                        let mut row = TemplateContext::new();
                        row.set("id", format!("item-{index}"));
                        row.set("title", format!("ROW-{index}"));
                        row.set("payload", "x".repeat(128));
                        row
                    })
                    .collect(),
            ),
        );
        let area = Rect::new(0, 0, 40, rows);
        let mut buffer = Buffer::empty(area);
        let mut renderer = BufferRenderer::new();
        let (result, (render_calls, render_bytes)) =
            measure(|| renderer.render_nodes(&nodes, &ctx, &mut buffer, area));
        result.unwrap();
        for index in 0..rows {
            let line = (0..40)
                .map(|x| buffer[(x, index)].symbol())
                .collect::<String>();
            assert!(
                line.contains(&format!("CALLER:ROW-{index}")),
                "row {index}: {line}"
            );
            assert!(!line.contains("CALLEE"));
        }
        let mut template = Template::from_source(source);
        *template.context_mut() = ctx.clone();
        template.refresh_focus(); // Warm the root AST outside the measured event path.
        let (_, (focus_calls, focus_bytes)) = measure(|| template.refresh_focus());
        assert_eq!(template.focus().focusable_ids.len(), usize::from(rows));
        for (index, id) in template.focus().focusable_ids.iter().enumerate() {
            assert_eq!(id, &format!("item-{index}"));
        }
        assert_eq!(template.context().get_str("prefix"), "CALLER");
        assert!(template.context().get("row").is_none());
        println!("rows={rows} render_calls={render_calls} render_bytes={render_bytes} focus_calls={focus_calls} focus_bytes={focus_bytes}");
    }
}
