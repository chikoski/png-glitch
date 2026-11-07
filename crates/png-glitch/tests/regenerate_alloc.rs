//! `regenerate` with `Fixed` and `Pattern` must not allocate (#162). Own binary because it
//! installs a counting global allocator; the counter is thread-local.

use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: forwards to `System` unchanged; it only counts calls.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn regenerate_does_not_allocate_for_fixed_and_pattern() {
    let (w, h) = (96u32, 64u32);
    let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(7)).collect();
    let mut live = LivePipeline::new(
        PixelSource::rgba8(w, h, &pixels),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub)),
    )
    .expect("supported");
    let strategies = [
        FilterStrategy::Fixed(FilterType::Paeth),
        FilterStrategy::Pattern(vec![FilterType::Sub, FilterType::Up, FilterType::Average]),
    ];
    // Warm-up.
    for s in &strategies {
        live.regenerate(s).expect("regenerate");
    }
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    for _ in 0..5 {
        for s in &strategies {
            live.regenerate(s).expect("regenerate");
        }
    }
    COUNTING.with(|on| on.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
