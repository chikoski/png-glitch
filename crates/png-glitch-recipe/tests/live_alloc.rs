//! A `Static` live frame (`encode` + `run` + `decode_into`) makes no heap allocation (#163).
//! `PerFrame` allocation is measured, not asserted (resolving allocates; see #165).
//!
//! Own test binary because it installs a counting global allocator. The counter is
//! thread-local, so allocations on rayon worker threads (parallel per-row effects) are not
//! counted; this test covers the calling thread.

use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
use png_glitch_recipe::{FilterConfig, ImageHeader, LiveRecipe, Recipe, SeedMode};
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

fn allocations_during(f: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCATIONS.with(Cell::get)
}

fn frames_allocations(mode: SeedMode) -> usize {
    let (w, h) = (96u32, 64u32);
    let frames: Vec<Vec<u8>> = (0..3u8)
        .map(|k| {
            (0..w * h * 4)
                .map(|i| (i as u8).wrapping_mul(7).wrapping_add(k.wrapping_mul(90)))
                .collect()
        })
        .collect();
    // No `PixelSort` (it allocates a pixel list per row); one generation boundary in the middle.
    let recipe = Recipe::new()
        .then(FilterConfig::Transpose { magnitude: 0.4 })
        .then(FilterConfig::Replace { magnitude: 0.3 })
        .then(FilterConfig::SubFilter)
        .then(FilterConfig::ColorDistortion {
            magnitude: 0.4,
            strength: None,
        })
        .then(FilterConfig::Invert);
    let mut live = LivePipeline::new(
        PixelSource::rgba8(w, h, &frames[0]),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
    )
    .expect("supported");
    let header = ImageHeader::of(live.model());
    let mut runner = LiveRecipe::new(recipe, header, Some(5), mode).expect("live recipe");
    let mut out = vec![0u8; frames[0].len()];
    // Warm-up frame (also starts the rayon pool).
    live.encode(&frames[1]).expect("encode");
    runner.run(&mut live).expect("run");
    live.decode_into(&mut out).expect("decode");

    allocations_during(|| {
        for i in 0..10 {
            live.encode(&frames[i % 3]).expect("encode");
            runner.run(&mut live).expect("run");
            live.decode_into(&mut out).expect("decode");
        }
    })
}

#[test]
fn a_static_frame_does_not_allocate() {
    assert_eq!(frames_allocations(SeedMode::Static), 0);
}

#[test]
fn per_frame_allocation_is_measured_not_asserted() {
    let count = frames_allocations(SeedMode::PerFrame);
    eprintln!("PerFrame: {count} allocations over 10 frames");
}
