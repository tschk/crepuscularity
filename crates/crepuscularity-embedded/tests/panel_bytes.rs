#![cfg(feature = "std")]

use crepuscularity_embedded::{PanelConfig, Rgb565ByteOrder, Ui};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}
struct Counting;
fn record(bytes: usize) {
    if ACTIVE.try_with(Cell::get).unwrap_or(false) {
        ALLOCATIONS.with(|count| {
            let (calls, size) = count.get();
            count.set((calls + 1, size + bytes));
        });
    }
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn panel_bytes_reuse_storage_and_follow_rendered_pixels_without_frame_copies() {
    for order in [Rgb565ByteOrder::Rgb, Rgb565ByteOrder::Bgr] {
        let mut ui = Ui::new(240, 320, "div w-full h-full bg-{color}").panel(PanelConfig {
            byte_order: order,
            ..PanelConfig::default()
        });
        let mut scratch_pointer = None;
        let mut previous = None;
        for color in ["red-500", "blue-500"] {
            ui.set("color", color);
            ui.render().unwrap();
            let raw_pointer = ui.rgb565().as_ptr();
            let expected: Vec<_> = ui
                .rgb565()
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|pair| match order {
                    Rgb565ByteOrder::Rgb => [pair[0], pair[1]],
                    Rgb565ByteOrder::Bgr => [pair[1], pair[0]],
                })
                .collect();
            assert_ne!(
                previous.as_ref(),
                Some(&expected),
                "color update did not change pixels"
            );
            previous = Some(expected.clone());
            let bytes = ui.rgb565_for_panel();
            assert_eq!(bytes, expected);
            match order {
                Rgb565ByteOrder::Rgb => assert_eq!(bytes.as_ptr(), raw_pointer),
                Rgb565ByteOrder::Bgr => {
                    if let Some(pointer) = scratch_pointer {
                        assert_eq!(bytes.as_ptr(), pointer);
                    }
                    scratch_pointer = Some(bytes.as_ptr());
                }
            }
            ALLOCATIONS.with(|count| count.set((0, 0)));
            ACTIVE.with(|active| active.set(true));
            for _ in 0..10 {
                std::hint::black_box(ui.rgb565_for_panel());
            }
            ACTIVE.with(|active| active.set(false));
            let measured = ALLOCATIONS.with(Cell::get);
            println!(
                "order={order:?} dimensions=240x320 extractions=10 allocations={} bytes={}",
                measured.0, measured.1
            );
            assert_eq!(
                measured,
                (0, 0),
                "warmed panel extraction copied the framebuffer"
            );
        }
    }
    let mut empty = Ui::new(0, 0, "").panel(PanelConfig::st7789_240x320());
    assert!(empty.rgb565_for_panel().is_empty());
}
