//! Built-in `ScanLinesEffect`s must not allocate in `apply_to_scan_lines`. `PixelSort` is the
//! documented exception and is left out. This is the only test in this file because it installs
//! a counting global allocator; the counter is thread-local, so rayon workers are not counted:
//! `LineEffect`s run through `for_each_mut` here, and their parallel path is not covered.

use png_glitch::effects::*;
use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
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
fn built_in_scan_lines_effects_do_not_allocate() {
    let (w, h) = (64u32, 48u32);
    let frame: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(13)).collect();
    let mut png = PngGlitch::from_pixels(
        PixelSource::rgba8(w, h, &frame),
        Encoding::rgba8(FilterStrategy::Adaptive),
    )
    .expect("supported encoding");
    let rows: Vec<u32> = (0..h + 2).collect();

    let effects: Vec<(&str, Box<dyn ScanLinesEffect>)> = vec![
        (
            "SetFilterTypes",
            Box::new(SetFilterTypes::new(vec![
                Some(FilterType::Paeth),
                None,
                Some(FilterType::Sub),
            ])),
        ),
        (
            "SetBytes",
            Box::new(SetBytes::new(vec![ByteEdit {
                line: 3,
                index: 5,
                value: 9,
            }])),
        ),
        ("Substitute", Box::new(Substitute::new(7, 200))),
        ("SwapLines", Box::new(SwapLines::new(vec![(0, 5), (2, 99)]))),
        ("CopyLines", Box::new(CopyLines::new(vec![(1, 4), (7, 0)]))),
        (
            "ShiftLines",
            Box::new(ShiftLines::new(vec![LineShift { line: 2, shift: 11 }])),
        ),
        (
            "Bitwise",
            Box::new(Bitwise::new(BitOp::Xor, 0x5A, rows.clone())),
        ),
        (
            "SwapChannels",
            Box::new(SwapChannels::new(SwapTarget::Gb, rows.clone())),
        ),
        (
            "OffsetColors",
            Box::new(OffsetColors::new(vec![LineOffsets {
                line: 4,
                offsets: vec![[3, -2, 1]; 64],
            }])),
        ),
        (
            "HslShift",
            Box::new(HslShift::new(40.0, 1.2, 0.9, rows.clone())),
        ),
        (
            "ChannelShift",
            Box::new(ChannelShift::new(3, 0, -3, rows.clone())),
        ),
        (
            "SwapBlocks",
            Box::new(SwapBlocks::new(
                8,
                vec![
                    BlockSwap {
                        src: (0, 0),
                        dst: (3, 2),
                    },
                    BlockSwap {
                        src: (1, 1),
                        dst: (2, 1),
                    },
                ],
            )),
        ),
    ];

    let mut lines = png.scan_lines_mut();
    for (name, effect) in &effects {
        effect.apply_to_scan_lines(&mut lines); // warm-up
        let count = allocations_during(|| effect.apply_to_scan_lines(&mut lines));
        assert_eq!(count, 0, "{name} allocated");
    }

    // `rows()` and `WithinRows` do not allocate either.
    let count = allocations_during(|| {
        let mut part = lines.rows(3..9);
        assert_eq!(part.len(), 6);
        for (_, effect) in &effects {
            struct D<'a>(&'a dyn ScanLinesEffect);
            impl ScanLinesEffect for D<'_> {
                fn apply_to_scan_lines(&self, l: &mut png_glitch::ScanLines<'_>) {
                    self.0.apply_to_scan_lines(l);
                }
            }
            D(&**effect).apply_to_scan_lines(&mut part);
        }
    });
    assert_eq!(count, 0, "rows() / effects on a sub-view allocated");
    let within = Substitute::new(7, 200).within_rows(4..20);
    let count = allocations_during(|| within.apply_to_scan_lines(&mut lines));
    assert_eq!(count, 0, "WithinRows allocated");

    // `LineEffect`s, sequentially (the parallel path is not counted by this allocator).
    let line_effects: Vec<(&str, Box<dyn LineEffect>)> = vec![
        ("Invert", Box::new(Invert::new())),
        ("Brighten", Box::new(Brighten::new(10))),
        ("Darken", Box::new(Darken::new(10))),
        ("ShiftChannels", Box::new(ShiftChannels::new(1, -2, 3))),
    ];
    for (name, effect) in &line_effects {
        let count = allocations_during(|| lines.for_each_mut(|line| effect.apply_to_line(line)));
        assert_eq!(count, 0, "{name} allocated");
    }
}
