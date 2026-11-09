//! `Masked` makes no heap allocation after `Masked::new` and one warm-up call (ADR 0006). This is
//! the only test in this file because it installs a counting global allocator (thread-local, so
//! rayon workers are not counted; the inner effects here are not parallel).

use png_glitch::effects::{Invert, Mask, Masked, ScanLinesEffect, SwapLines};
use png_glitch::{Encoding, FilterStrategy, PixelSource, PngGlitch};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` because the thread-locals may be gone while the thread is shutting down.
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

fn allocations_during(f: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCATIONS.with(Cell::get)
}

#[test]
fn masked_does_not_allocate_in_steady_state() {
    let (w, h) = (64u32, 48u32);
    let frame: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(13)).collect();
    let mut png = PngGlitch::from_pixels(
        PixelSource::rgba8(w, h, &frame),
        Encoding::rgba8(FilterStrategy::Adaptive),
    )
    .expect("supported encoding");
    let stride = png.data().len() / h as usize;

    let half = Mask::rect(w, h, 0, 0, w / 2, h);
    let soft = Mask::from_values(w, h, (0..w * h).map(|i| i as u8).collect()).unwrap();
    let invert_rect = Masked::new(Invert::new(), half, stride, h);
    let swap_soft = Masked::new(SwapLines::new(vec![(0, 5), (2, 9)]), soft, stride, h);

    let mut lines = png.scan_lines_mut();
    invert_rect.apply_to_scan_lines(&mut lines); // warm-up
    swap_soft.apply_to_scan_lines(&mut lines);
    let count = allocations_during(|| {
        for _ in 0..10 {
            invert_rect.apply_to_scan_lines(&mut lines);
            swap_soft.apply_to_scan_lines(&mut lines);
        }
    });
    assert_eq!(count, 0, "Masked allocated in steady state");
}
