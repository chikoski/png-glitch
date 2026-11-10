//! After one warm-up frame, `reload_pixels` -> `apply` -> `render_into` (and `reset`) make no
//! heap allocation. The only test in the file: it installs a counting global allocator.

use png_glitch_component::{ColorType, Encoding, FilterStrategy, FilterType, SeedMode, Session};
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

/// Steps targeted with `rows:` and `rect:` are re-resolved in place in `PerFrame` mode (#207), and
/// `pixel-sort` sorts in the scan-line scratch buffer (#208).
#[test]
fn a_live_frame_does_not_allocate() {
    let (w, h) = (64u32, 48u32);
    let frames: Vec<Vec<u8>> = (0..3u8)
        .map(|k| {
            (0..w * h * 4)
                .map(|i| (i as u8).wrapping_mul(7).wrapping_add(k.wrapping_mul(90)))
                .collect()
        })
        .collect();
    let recipe = "filters:\n  - type: replace\n    magnitude: 0.2\n  - type: transpose\n    magnitude: 0.2\n  - type: invert\n    rows: 5..20\n  - type: set-zero\n    magnitude: 0.3\n    rect: 4,6,30,20\n  - type: pixel-sort\n    magnitude: 0.3\n  - type: pixel-sort\n    magnitude: 0.3\n    criterion: hue\n  - type: sub-filter\n";
    for mode in [SeedMode::Static, SeedMode::PerFrame] {
        let mut s = Session::from_pixels(
            w,
            h,
            &frames[0],
            Encoding {
                color_type: ColorType::TrueColorAlpha,
                bit_depth: 8,
                filter: FilterStrategy::Fixed(FilterType::Paeth),
            },
        )
        .unwrap();
        s.set_recipe(recipe, 3, mode).unwrap();
        let mut out = vec![0u8; (w * h * 4) as usize];
        // Warm-up frame: sizes every scratch buffer.
        s.reload_pixels(&frames[0]).unwrap();
        s.apply().unwrap();
        s.render_into(&mut out).unwrap();

        let n = allocations_during(|| {
            for f in &frames {
                s.reload_pixels(f).unwrap();
                s.apply().unwrap();
                s.render_into(&mut out).unwrap();
                s.reset().unwrap();
            }
        });
        assert_eq!(n, 0, "{mode:?}: a live frame allocated {n} time(s)");
    }
}
