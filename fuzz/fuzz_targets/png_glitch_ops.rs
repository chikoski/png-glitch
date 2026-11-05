//! Parses arbitrary bytes as PNG, then runs the low-level `PngGlitch`
//! operations (filter removal/application, presets, transpose) and encodes.
#![no_main]
use libfuzzer_sys::fuzz_target;
use png_glitch::presets::{Brighten, Darken, Invert, ShiftChannels};
use png_glitch::{FilterType, Limits, ParseOptions, PngGlitch};

const LIMITS: Limits = Limits {
    max_bytes: 16 * 1024 * 1024,
    max_pixels: 4 * 1024 * 1024,
};

fuzz_target!(|data: &[u8]| {
    let options = ParseOptions::default().with_limits(LIMITS);
    let Ok(mut glitch) = PngGlitch::from_bytes_with_options(data, options) else {
        return;
    };

    glitch.remove_filter();
    glitch.apply(Invert);
    glitch.apply(Brighten { strength: 100 });
    glitch.apply(Darken { strength: 50 });
    glitch.apply(ShiftChannels { r: 10, g: -10, b: 300 });

    let height = glitch.height();
    if height > 2 {
        let mid = height / 2;
        glitch.transpose(0, mid, mid);
    }
    // Out-of-range regions must be clamped, not panic.
    glitch.transpose(height, 0, u32::MAX);
    glitch.remove_filter_from(height / 3, u32::MAX);

    for filter in [
        FilterType::None,
        FilterType::Sub,
        FilterType::Up,
        FilterType::Average,
        FilterType::Paeth,
    ] {
        glitch.apply_filter(filter);
        glitch.remove_filter();
    }

    let _ = glitch.to_bytes();
});
