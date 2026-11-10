//! A live frame (`encode` + `run` + `decode_into`) makes no heap allocation: in `Static` mode
//! (#163), and in `PerFrame` mode after one warm-up frame, as resolving reuses the previous
//! frame's buffers (#165).
//!
//! Own test binary because it installs a counting global allocator. The counter is
//! thread-local, so allocations on other rayon worker threads (parallel per-row effects) are
//! not counted; this test covers the thread that runs the frames.
//!
//! The frames run on the only worker of a one-thread rayon pool (`install`). From a thread
//! outside any pool, rayon sends each parallel call through its global job queue, which
//! allocates a new block every few dozen jobs. With more than one worker, the measuring thread
//! may wait for a stolen job, and the first time it sleeps, the OS mutex behind rayon's sleep
//! state can be allocated lazily (seen on macOS). Both are rayon's costs, not the frame's;
//! with one worker every job runs on the measuring thread and neither happens.

use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
use png_glitch_recipe::rand::SeedableRng;
use png_glitch_recipe::{
    ChaCha8Rng, FilterConfig, FilterEntry, ImageHeader, LiveRecipe, Recipe, RectSpec, RowRange,
    SeedMode,
};
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

/// No `PixelSort` (it allocates a pixel list per row when it runs); one generation boundary
/// in the middle. Every other step has an upper bound on its list sizes. Steps targeted with
/// `rows:` and `rect:` (#207) reuse their buffers too.
fn recipe() -> Recipe {
    Recipe::new()
        .then(FilterConfig::ChangeFilterType { magnitude: 0.2 })
        .then(FilterConfig::Transpose { magnitude: 0.4 })
        .then(FilterConfig::Replace { magnitude: 0.3 })
        .then(FilterConfig::SetZero { magnitude: 0.3 })
        .then(FilterConfig::RandomCopy { times: 5 })
        .then(FilterConfig::SubFilter)
        .then(FilterConfig::ColorDistortion {
            magnitude: 0.4,
            strength: None,
        })
        .then(FilterConfig::Bitwise {
            magnitude: 0.3,
            op: None,
            value: Some(0x5a),
        })
        .then(FilterConfig::ChannelSwap {
            magnitude: 0.3,
            target: None,
        })
        .then(FilterConfig::HorizontalShift { magnitude: 0.2 })
        .then(FilterConfig::BlockScramble {
            magnitude: 0.3,
            block_size: Some(8),
        })
        .then(FilterConfig::ColorSpaceGlitch {
            magnitude: 0.2,
            hue_shift: Some(90.0),
            saturation_mult: None,
            lightness_mult: None,
        })
        .then(FilterConfig::ChromaticAberration {
            magnitude: 0.3,
            r_offset: None,
            g_offset: None,
            b_offset: None,
        })
        .then(FilterConfig::Invert)
        .then_rows(FilterConfig::Invert, RowRange::new(5, 20))
        .then_rows(
            FilterConfig::Replace { magnitude: 0.3 },
            RowRange::new(10, 40),
        )
        .then_entry(FilterEntry {
            rect: Some(RectSpec {
                x: 4,
                y: 6,
                w: 30,
                h: 20,
            }),
            ..FilterConfig::SetZero { magnitude: 0.3 }.into()
        })
        .then_entry(FilterEntry {
            rows: Some(RowRange::new(8, 50)),
            rect: Some(RectSpec {
                x: 10,
                y: 0,
                w: 40,
                h: 30,
            }),
            invert_mask: true,
            ..FilterConfig::Bitwise {
                magnitude: 0.4,
                op: None,
                value: Some(0x33),
            }
            .into()
        })
}

fn frames_allocations(mode: SeedMode, frames_run: usize) -> usize {
    frames_allocations_of(recipe(), mode, frames_run)
}

fn frames_allocations_of(recipe: Recipe, mode: SeedMode, frames_run: usize) -> usize {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("thread pool")
        .install(|| frames_allocations_on_this_thread(recipe, mode, frames_run))
}

fn frames_allocations_on_this_thread(recipe: Recipe, mode: SeedMode, frames_run: usize) -> usize {
    let (w, h) = (96u32, 64u32);
    let frames: Vec<Vec<u8>> = (0..3u8)
        .map(|k| {
            (0..w * h * 4)
                .map(|i| (i as u8).wrapping_mul(7).wrapping_add(k.wrapping_mul(90)))
                .collect()
        })
        .collect();
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
        for i in 0..frames_run {
            live.encode(&frames[i % 3]).expect("encode");
            runner.run(&mut live).expect("run");
            live.decode_into(&mut out).expect("decode");
        }
    })
}

#[test]
fn a_static_frame_does_not_allocate() {
    assert_eq!(frames_allocations(SeedMode::Static, 100), 0);
}

#[test]
fn a_per_frame_frame_does_not_allocate_after_warm_up() {
    // Each frame has its own seed, so list lengths change from frame to frame.
    assert_eq!(frames_allocations(SeedMode::PerFrame, 100), 0);
}

#[test]
fn resolve_into_does_not_allocate_after_warm_up() {
    let header = ImageHeader {
        width: 50,
        height: 70,
        color_type: png_glitch::ColorType::TrueColorAlpha,
        bit_depth: 8,
    };
    let recipe = recipe();
    let mut out = recipe.resolve(&header, Some(0)).expect("resolve");
    recipe
        .resolve_into(&header, Some(1), &mut out)
        .expect("resolve");
    let count = allocations_during(|| {
        for seed in 2..200 {
            recipe
                .resolve_into(&header, Some(seed), &mut out)
                .expect("resolve");
        }
    });
    assert_eq!(count, 0);
}

#[test]
fn seeding_the_rng_does_not_allocate() {
    let count = allocations_during(|| {
        for seed in 0..100 {
            std::hint::black_box(ChaCha8Rng::seed_from_u64(seed));
        }
    });
    assert_eq!(count, 0);
}
