#![cfg(feature = "ssr")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use crepuscularity_core::{context::TemplateContext, parser::parse_template};
use crepuscularity_web::render_nodes_ssr;

struct CountingAllocator;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count_allocation() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct StopCounting;

impl Drop for StopCounting {
    fn drop(&mut self) {
        COUNTING.with(|counting| counting.set(false));
    }
}

fn render_allocations(depth: usize, dynamic: bool) -> usize {
    let leaf = if dynamic { "{value}" } else { "literal" };
    let source = format!(
        "{}{}{}",
        "<div>".repeat(depth),
        leaf,
        "</div>".repeat(depth)
    );
    // Parsing, source construction and context setup are outside the measurement.
    let nodes = parse_template(&source).unwrap();
    let mut context = TemplateContext::new();
    context.set("value", "resolved");
    let counter = Cell::new(0);
    let mut bindings = Default::default();
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    let stop = StopCounting;
    let html = render_nodes_ssr(&nodes, &context, &counter, &mut bindings, true).unwrap();
    drop(stop);
    let allocations = ALLOCATIONS.with(Cell::get);

    assert_eq!(html.matches("</div>").count(), depth);
    if dynamic {
        assert_eq!(bindings.len(), depth + 1);
        assert_eq!(html.matches("data-crepus-root=\"1\"").count(), 1);
        assert_eq!(html.matches("data-crepus-kind=\"element\"").count(), depth);
        assert_eq!(html.matches("data-crepus-kind=\"text\"").count(), 1);
        assert!(html.contains(">resolved</span>"));
    } else {
        assert!(bindings.is_empty());
        assert_eq!(html, source);
    }
    allocations
}

#[test]
fn nested_ssr_elements_do_not_allocate_quadratic_subtree_copies() {
    for dynamic in [false, true] {
        let small = render_allocations(32, dynamic);
        let large = render_allocations(64, dynamic);
        assert!(
            large < small * 3,
            "doubling nested depth exceeded allocation bound: dynamic={dynamic}, {small} -> {large}"
        );
    }
}
