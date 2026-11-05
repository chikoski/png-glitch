//! Feeds arbitrary bytes to the PNG parser in both lenient and strict mode.
#![no_main]
use libfuzzer_sys::fuzz_target;
use png_glitch::{Limits, ParseOptions, PngGlitch};

/// Tighter than the library defaults so a forged IHDR cannot make a single
/// input exhaust libFuzzer's RSS limit or time budget.
const LIMITS: Limits = Limits {
    max_bytes: 16 * 1024 * 1024,
    max_pixels: 4 * 1024 * 1024,
};

fuzz_target!(|data: &[u8]| {
    for strict in [false, true] {
        let options = ParseOptions::default().with_limits(LIMITS).with_strict(strict);
        if let Ok(png) = PngGlitch::from_bytes_with_options(data, options) {
            // Touch the accessors so header fields are exercised too.
            let _ = (png.width(), png.height(), png.color_type(), png.bit_depth());
            let _ = png.warnings().len();
            // A successfully parsed image must always re-encode.
            let _ = png.to_bytes();
        }
    }
});
