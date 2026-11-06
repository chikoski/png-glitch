//! End-to-end `webp-glitch-context` pipeline: parse arbitrary WebP bytes, queue every
//! WebP-capable filter, execute, and encode.
#![no_main]
use libfuzzer_sys::fuzz_target;
use webp_glitch_context::{
    AlphaGlitchStrategy, BitOp, FilterConfig, SortCriterion, SwapTarget, WebpGlitchContext,
};

/// Running every filter (plus a libwebp re-encode) per input is expensive, so
/// keep images small to get a useful number of executions per second. Inputs
/// above `MAX_INPUT_LEN` are skipped because WebP decoding has no pixel limit.
const MAX_INPUT_LEN: usize = 64 * 1024;
const MAX_WEBP_PIXELS: u64 = 256 * 256;

/// One configuration per [`FilterConfig`] variant, with parameters chosen to
/// hit edge cases (extreme offsets, out-of-range indices, max strength).
/// PNG-only variants are rejected by `add_from_config`, which is fine.
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
        MacroblockGlitch { magnitude: 1.0 },
        AlphaGlitch {
            magnitude: 1.0,
            strategy: Some(AlphaGlitchStrategy::Invert),
        },
        AlphaGlitch {
            magnitude: 1.0,
            strategy: Some(AlphaGlitchStrategy::Randomize),
        },
        AlphaGlitch {
            magnitude: 1.0,
            strategy: Some(AlphaGlitchStrategy::Zero),
        },
        AlphaGlitch {
            magnitude: 1.0,
            strategy: Some(AlphaGlitchStrategy::One),
        },
        LossyArtifact { quality: 0.0 },
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
            | PaethFilter
            | MacroblockGlitch { .. }
            | AlphaGlitch { .. }
            | LossyArtifact { .. } => {}
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
    let Ok(mut ctx) = WebpGlitchContext::new(data, Some(seed_of(data))) else {
        return;
    };
    if u64::from(ctx.width()) * u64::from(ctx.height()) > MAX_WEBP_PIXELS {
        return;
    }

    for config in every_filter() {
        // PNG-only filters are rejected for WebP; that is fine.
        let _ = ctx.add_from_config(config);
    }
    ctx.execute();
    let _ = ctx.buffer();
});
