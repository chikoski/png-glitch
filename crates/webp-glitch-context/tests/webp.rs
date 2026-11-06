//! WebP pipeline tests: every WebP-capable filter runs, and seeded output is reproducible.

use anyhow::Result;
use webp_glitch_context::{
    AlphaGlitchStrategy, BitOp, FilterConfig, SortCriterion, SwapTarget, WebpGlitchContext,
};

fn sample_webp() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([(x * 7) as u8, (y * 11) as u8, ((x + y) * 5) as u8, 200])
    });
    let dyn_img = image::DynamicImage::ImageRgba8(img);
    webp::Encoder::from_image(&dyn_img)
        .unwrap()
        .encode(90.0)
        .to_vec()
}

#[test]
fn all_webp_filters_and_reproducibility() -> Result<()> {
    let webp_bytes = sample_webp();
    let webp_configs = vec![
        FilterConfig::Invert,
        FilterConfig::Brighten { strength: 10 },
        FilterConfig::Darken { strength: 10 },
        FilterConfig::ShiftChannels { r: 5, g: -5, b: 0 },
        FilterConfig::PixelSort {
            magnitude: 0.5,
            criterion: Some(SortCriterion::Hue),
        },
        FilterConfig::Bitwise {
            magnitude: 0.5,
            op: Some(BitOp::Xor),
            value: Some(0x55),
        },
        FilterConfig::ChannelSwap {
            magnitude: 0.5,
            target: Some(SwapTarget::Gb),
        },
        FilterConfig::HorizontalShift { magnitude: 0.5 },
        FilterConfig::ColorDistortion {
            magnitude: 0.5,
            strength: Some(20),
        },
        FilterConfig::ChromaticAberration {
            magnitude: 0.5,
            r_offset: Some(2),
            g_offset: Some(0),
            b_offset: Some(-2),
        },
        FilterConfig::MacroblockGlitch { magnitude: 0.5 },
        FilterConfig::AlphaGlitch {
            magnitude: 0.5,
            strategy: Some(AlphaGlitchStrategy::Invert),
        },
        FilterConfig::LossyArtifact { quality: 30.0 },
    ];

    let run = |seed: u64| -> Result<Vec<u8>> {
        let mut ctx = WebpGlitchContext::new(&webp_bytes, Some(seed))?;
        for cfg in &webp_configs {
            ctx.add_from_config(cfg.clone())?;
        }
        ctx.execute();
        ctx.buffer()
    };

    let out1 = run(777)?;
    let out2 = run(777)?;
    assert_eq!(out1, out2);
    assert!(out1.starts_with(b"RIFF") && &out1[8..12] == b"WEBP");
    Ok(())
}

#[test]
fn png_only_filters_are_rejected() -> Result<()> {
    let mut ctx = WebpGlitchContext::new(&sample_webp(), Some(1))?;
    let err = ctx.add_from_config(FilterConfig::SubFilter).unwrap_err();
    assert!(err.to_string().contains("PNG-only"));
    Ok(())
}

/// Regression (found by fuzzing): extreme offsets overflowed `x as i32 + offset` and panicked in
/// debug builds.
#[test]
fn chromatic_aberration_extreme_offsets_do_not_overflow() -> Result<()> {
    let webp = include_bytes!("../../webp-glitch/etc/sample00.webp");
    let mut ctx = WebpGlitchContext::new(webp, Some(7))?;
    ctx.add_from_config(FilterConfig::ChromaticAberration {
        magnitude: 1.0,
        r_offset: Some(i32::MAX),
        g_offset: Some(0),
        b_offset: Some(i32::MIN),
    })?;
    ctx.execute();
    ctx.buffer()?;
    Ok(())
}
