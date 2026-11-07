//! `reload_pixels` must not allocate. This is the only test in this file because it installs a
//! counting global allocator; the counter is thread-local so allocations made by the test
//! harness on other threads are not counted.

use png_glitch::{ColorType, Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
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
fn reload_pixels_does_not_allocate() {
    let (w, h) = (96u32, 64u32);
    let frames: Vec<Vec<u8>> = (0..3u8)
        .map(|k| {
            (0..w * h * 4)
                .map(|i| (i as u8).wrapping_mul(7).wrapping_add(k.wrapping_mul(90)))
                .collect()
        })
        .collect();

    let strategies = [
        FilterStrategy::Fixed(FilterType::Paeth),
        FilterStrategy::Pattern(vec![FilterType::Sub, FilterType::Up, FilterType::Paeth]),
        FilterStrategy::Adaptive,
    ];
    for color_type in [
        ColorType::TrueColorAlpha,
        ColorType::TrueColor,
        ColorType::GrayScaleAlpha,
        ColorType::GrayScale,
    ] {
        for filter in &strategies {
            let enc = Encoding {
                color_type,
                bit_depth: 8,
                filter: filter.clone(),
            };
            let mut png = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &frames[0]), enc)
                .expect("supported encoding");
            // Warm-up call, as the acceptance criteria describe.
            png.reload_pixels(&frames[1]).expect("reload");

            let count = allocations_during(|| {
                for i in 0..10 {
                    png.reload_pixels(&frames[i % 3]).expect("reload");
                }
            });
            assert_eq!(count, 0, "{color_type:?} {filter:?} allocated");
        }
    }
}
