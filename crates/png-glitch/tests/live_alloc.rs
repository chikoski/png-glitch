//! `encode`, taking the view and `decode_into` must not allocate in steady state (#160). This is
//! the only test in the file because it installs a counting global allocator; the counter is
//! thread-local, so rayon workers are not counted and `par_for_each` is not called here.

use png_glitch::{ColorType, Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
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

#[test]
fn a_live_frame_does_not_allocate() {
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
        FilterStrategy::Pattern(vec![FilterType::Sub, FilterType::Up, FilterType::Average]),
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
            let mut live =
                LivePipeline::new(PixelSource::rgba8(w, h, &frames[0]), enc).expect("supported");
            let mut out = vec![0u8; frames[0].len()];
            // Warm-up frame.
            live.encode(&frames[1]).expect("encode");
            live.decode_into(&mut out).expect("decode");

            let count = allocations_during(|| {
                for i in 0..10 {
                    let mut lines = live.encode(&frames[i % 3]).expect("encode");
                    // Sequential scan-line operations, including the scratch buffer.
                    lines.swap(0, 1);
                    lines.copy(2, 3);
                    lines.set_filter_type(4, FilterType::Sub);
                    lines.for_each_mut(|row| row.update(1, 7));
                    lines.scratch().fill(1);
                    live.decode_into(&mut out).expect("decode");
                }
            });
            assert_eq!(count, 0, "{color_type:?} {filter:?} allocated");
        }
    }
}
