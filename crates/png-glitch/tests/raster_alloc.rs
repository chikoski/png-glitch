//! 2D pixel access makes no heap allocation: `pixel`, `set_pixel`, `column` (consumed), `block_mut`
//! and the `Pixel` conversions. This is the only test in this file because it installs a counting
//! global allocator (the counter is thread-local).

use png_glitch::{Encoding, FilterStrategy, Pixel, PixelSource, PngGlitch};
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
fn raster_access_does_not_allocate() {
    let (w, h) = (64u32, 48u32);
    let frame: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(13)).collect();
    let mut png = PngGlitch::from_pixels(
        PixelSource::rgba8(w, h, &frame),
        Encoding::rgba8(FilterStrategy::Adaptive),
    )
    .expect("supported encoding");
    let mut lines = png.scan_lines_mut();

    // Warm-up, so that lazy one-time work (if any) is not counted.
    let _ = lines.pixel(1, 1);
    let mut sum = 0u64;
    let count = allocations_during(|| {
        for y in 0..h {
            for x in 0..w {
                if let Some(p) = lines.pixel(x, y) {
                    sum += u64::from(p.r());
                    lines.set_pixel(x, y, p.map_channels(|v| v ^ 0x55)).unwrap();
                }
            }
        }
        for x in [0, 17, w - 1] {
            sum += lines.column(x).map(|p| u64::from(p.g())).sum::<u64>();
        }
        let mut block = lines.block_mut(w - 5, h - 5, 9, 9).unwrap();
        for y in 0..block.height() {
            for x in 0..block.width() {
                block.set_pixel(x, y, Pixel::RGBA(1, 2, 3, 4)).unwrap();
                sum += u64::from(block.pixel(x, y).unwrap().a());
            }
        }
        let rgba = Pixel::RGBA(10, 20, 30, 40).to_rgba16(8).unwrap();
        sum += u64::from(rgba[0]);
        sum += u64::from(
            Pixel::from_rgba16(rgba, png_glitch::ColorType::TrueColorAlpha, 8)
                .unwrap()
                .r(),
        );
        sum += u64::from(Pixel::RGB(1, 2, 3).luma16(8).unwrap());
    });
    assert!(sum > 0);
    assert_eq!(count, 0, "raster access allocated");
}
