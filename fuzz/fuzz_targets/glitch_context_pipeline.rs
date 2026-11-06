//! End-to-end `glitch-context` pipeline: parse arbitrary PNG bytes, queue every
//! PNG-capable filter, execute, and encode.
#![no_main]
use glitch_context::{
    BitOp, FilterConfig, GlitchContext, Limits, ParseOptions, PreProcess, SortCriterion, SwapTarget,
};
use libfuzzer_sys::fuzz_target;

/// Running ~30 filters per input is expensive, so keep images small to get a
/// useful number of executions per second.
const LIMITS: Limits = Limits {
    max_bytes: 1024 * 1024,
    max_pixels: 256 * 256,
};
const MAX_INPUT_LEN: usize = 64 * 1024;

/// One configuration per [`FilterConfig`] variant, with parameters chosen to
/// hit edge cases (extreme offsets, out-of-range indices, max strength).
/// WebP-only variants are not listed; they are exercised by `webp_pipeline`.
fn every_filter() -> Vec<FilterConfig> {
    use FilterConfig::*;
    let configs = vec![
        RemoveFilter,
        ChangeFilterType { magnitude: 0.5 },
        Replace { magnitude: 0.5 },
        Transpose { magnitude: 0.5 },
        SetZero { magnitude: 0.5 },
        RandomCopy { times: 64 },
        Substitute {
            index: 0,
            value: 0xff,
        },
        Substitute {
            index: usize::MAX,
            value: 0x00,
        },
        PixelSort {
            magnitude: 1.0,
            criterion: Some(SortCriterion::Brightness),
        },
        PixelSort {
            magnitude: 1.0,
            criterion: Some(SortCriterion::Hue),
        },
        Bitwise {
            magnitude: 1.0,
            op: Some(BitOp::And),
            value: Some(0x0f),
        },
        Bitwise {
            magnitude: 1.0,
            op: Some(BitOp::Or),
            value: Some(0xf0),
        },
        Bitwise {
            magnitude: 1.0,
            op: Some(BitOp::Xor),
            value: Some(0xaa),
        },
        ChannelSwap {
            magnitude: 1.0,
            target: Some(SwapTarget::Rg),
        },
        ChannelSwap {
            magnitude: 1.0,
            target: Some(SwapTarget::Gb),
        },
        ChannelSwap {
            magnitude: 1.0,
            target: Some(SwapTarget::Br),
        },
        HorizontalShift { magnitude: 1.0 },
        BlockScramble {
            magnitude: 1.0,
            block_size: Some(1),
        },
        BlockScramble {
            magnitude: 1.0,
            block_size: Some(u32::MAX),
        },
        ColorDistortion {
            magnitude: 1.0,
            strength: Some(i16::MAX),
        },
        ColorSpaceGlitch {
            magnitude: 1.0,
            hue_shift: Some(-720.0),
            saturation_mult: Some(1e9),
            lightness_mult: Some(0.0),
        },
        ChromaticAberration {
            magnitude: 1.0,
            r_offset: Some(i32::MAX),
            g_offset: Some(0),
            b_offset: Some(i32::MIN),
        },
        Invert,
        Brighten { strength: u16::MAX },
        Darken { strength: u16::MAX },
        ShiftChannels {
            r: i16::MAX,
            g: i16::MIN,
            b: 1,
        },
        SubFilter,
        UpFilter,
        AverageFilter,
        PaethFilter,
    ];
    // Compile-time reminder: adding a `FilterConfig` variant breaks this match
    // until the variant is also added to the list above.
    for c in &configs {
        match c {
            ChangeFilterType { .. }
            | Replace { .. }
            | Transpose { .. }
            | SetZero { .. }
            | RandomCopy { .. }
            | Substitute { .. }
            | PixelSort { .. }
            | Bitwise { .. }
            | ChannelSwap { .. }
            | HorizontalShift { .. }
            | BlockScramble { .. }
            | ColorDistortion { .. }
            | ColorSpaceGlitch { .. }
            | ChromaticAberration { .. }
            | Invert
            | Brighten { .. }
            | Darken { .. }
            | ShiftChannels { .. }
            | RemoveFilter
            | SubFilter
            | UpFilter
            | AverageFilter
            | PaethFilter => {}
            // WebP-only variants: covered by the `webp_pipeline` target.
            MacroblockGlitch { .. } | AlphaGlitch { .. } | LossyArtifact { .. } => {}
        }
    }
    configs
}

/// FNV-1a, so the RNG seed varies with the input but stays reproducible.
fn seed_of(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }
    let options = ParseOptions::default().with_limits(LIMITS);
    let Ok(mut ctx) = GlitchContext::new_with_options(data, Some(seed_of(data)), options) else {
        return;
    };

    ctx.pre_process(PreProcess::PaethFilter);
    for config in every_filter() {
        // `every_filter` is PNG-only, so a rejection is unexpected but harmless.
        let _ = ctx.add_from_config(config);
    }
    ctx.execute();
    let _ = ctx.buffer();
});
