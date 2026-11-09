//! Recipes, randomness and execution on top of `png-glitch`.
//!
//! A [`Recipe`] is an ordered list of steps, independent of any image. Resolving it with a seed
//! and an [`ImageHeader`] draws every random choice and yields a [`ResolvedRecipe`], which holds
//! only deterministic `png-glitch` effects; executing that is deterministic and uses no RNG.
//! [`GlitchContext`] is a convenience wrapper that bundles an image, a filter queue and an RNG.
//!
//! WebP support lives in the separate `webp-glitch-context` crate, and format detection and
//! dispatch between PNG and WebP live in the CLI.
//!
//! # Modules
//!
//! | Module | Contents |
//! | :----- | :------- |
//! | `config` | [`FilterConfig`] (YAML / `-f` spec steps) and its parameter enums |
//! | `filters` | The built-in randomized filters (`Replace`, `PixelSort`, …) |
//! | `context` | [`GlitchContext`], [`GlitchFilter`], [`PixelFilter`] |
//! | `error` | [`RecipeError`] |
//! | [`prelude`] | The most commonly used names, flat |
//!
//! Everything is also re-exported at the crate root.
#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

#[macro_use]
mod resolve;
mod targeting;

mod config;
mod context;
mod error;
mod file;
pub(crate) mod filters;
mod live;
mod meta;

pub use config::*;
pub use context::*;
pub use error::*;
pub use file::RecipeFile;
pub use filters::*;
pub use live::{LiveRecipe, SeedMode, frame_seed};
pub use meta::{FilterInfo, Format, ParamInfo, ParamKind, filter_info, filters, info_for};
use resolve::{Builtin, Resolve, StepScratch, take_buffer};
pub use resolve::{
    ImageHeader, Recipe, RecipeStep, ResolvedEffect, ResolvedRecipe, ResolvedStep, Step,
};
pub use targeting::{FilterEntry, MaskRef, MaskSet, RectSpec, RowRange};

#[cfg(feature = "clap")]
use clap::ValueEnum;
use png_glitch::effects as fx;
pub use png_glitch::effects::{Brighten, Darken, Invert, ShiftChannels};
pub use png_glitch::{FilterType, Limits, ParseOptions, Pixel, PngError, PngGlitch, PngWarning};
pub use rand;
use rand::{Rng, RngCore, SeedableRng};
pub use rand_chacha;
pub use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The most commonly used names: `use png_glitch_recipe::prelude::*;`.
pub mod prelude {
    pub use crate::{
        FilterConfig, GlitchContext, GlitchFilter, ImageHeader, PngGlitch, Recipe, RecipeError,
        RecipeStep, ResolvedEffect, ResolvedRecipe, Result,
    };
}

#[cfg(test)]
mod test {
    use super::*;

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[test]
    fn test_filters() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");

        // Test SubFilter
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::SubFilter,
        )));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Sub, line.filter_type());
        }

        // Test UpFilter
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::UpFilter,
        )));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Up, line.filter_type());
        }

        // Test AverageFilter
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::AverageFilter,
        )));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Average, line.filter_type());
        }

        // Test PaethFilter
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::PaethFilter,
        )));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Paeth, line.filter_type());
        }

        // Test RemoveFilter (None)
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::SubFilter,
        )));
        ctx.add_filter(Regenerate::new(RegenerateStrategy::Fixed(
            PreProcess::RemoveFilter,
        )));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::None, line.filter_type());
        }

        // Test Substitute
        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(Substitute::new(0, 255));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::from_bytes(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(line.index(0), Some(255));
        }

        // Test RandomCopy
        let mut ctx = GlitchContext::new(bytes, Some(12345))?;
        ctx.add_filter(RandomCopy::new(10));
        ctx.execute();
        let buffer = ctx.buffer()?;
        assert!(!buffer.is_empty());

        Ok(())
    }

    #[cfg(not(feature = "os-rng"))]
    #[test]
    fn missing_seed_is_an_error_without_os_rng() {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");
        assert!(matches!(
            GlitchContext::new(bytes, None),
            Err(RecipeError::SeedRequired)
        ));
    }

    #[test]
    fn test_pixel_filter_adapter_on_png() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");

        let mut ctx = GlitchContext::new(bytes, Some(0))?;
        ctx.add_filter(PixelFilterAdapter(Invert));
        ctx.execute();
        let buffer = ctx.buffer()?;
        assert!(
            !buffer.is_empty(),
            "PixelFilterAdapter(Invert) should produce output"
        );

        let mut ctx2 = GlitchContext::new(bytes, Some(0))?;
        ctx2.add_filter(PixelFilterAdapter(Brighten::new(20)));
        ctx2.execute();
        let buffer2 = ctx2.buffer()?;
        assert!(
            !buffer2.is_empty(),
            "PixelFilterAdapter(Brighten) should produce output"
        );

        Ok(())
    }

    #[test]
    fn test_execute_drains_filter_queue() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");
        let mut ctx = GlitchContext::new(bytes, Some(42))?;
        ctx.add_filter(Invert);
        ctx.execute();
        let after_first = ctx.buffer()?;

        // Calling execute() a second time without adding filters must be a no-op
        // (if Invert ran again, it would revert back to original bytes).
        ctx.execute();
        let after_second = ctx.buffer()?;
        assert_eq!(after_first, after_second);
        Ok(())
    }

    /// Regression (found by `fuzz/glitch_context_pipeline`): extreme offsets
    /// overflowed `x as i32 + offset` and panicked in debug builds.
    #[test]
    fn test_chromatic_aberration_extreme_offsets_do_not_overflow() -> Result<()> {
        let config = || FilterConfig::ChromaticAberration {
            magnitude: 1.0,
            r_offset: Some(i32::MAX),
            g_offset: Some(0),
            b_offset: Some(i32::MIN),
        };

        let png = include_bytes!("../../png-glitch/etc/sample00.png");
        let mut ctx = GlitchContext::new(png, Some(7))?;
        ctx.add_from_config(config())?;
        ctx.execute();
        ctx.buffer()?;
        Ok(())
    }

    #[test]
    fn test_filter_defaults_and_constructors() {
        let bs = BlockScramble::new(0.3);
        assert_eq!(bs.block_size, 16);
        assert_eq!(bs.with_block_size(32).block_size, 32);

        let cd = ColorDistortion::new(0.4);
        assert_eq!(cd.strength, 20);
        assert_eq!(cd.with_strength(50).strength, 50);

        let ca = ChromaticAberration::new(0.25);
        assert_eq!((ca.r_offset, ca.g_offset, ca.b_offset), (2, 0, -2));

        let csg = ColorSpaceGlitch::new(0.5);
        assert_eq!(
            (csg.hue_shift, csg.saturation_mult, csg.lightness_mult),
            (0.0, 1.0, 1.0)
        );
    }

    #[test]
    fn test_png_context_rejects_webp_only_filter() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");
        let mut ctx = GlitchContext::new(bytes, Some(1))?;

        ctx.add_from_config(FilterConfig::Invert)?;
        let err = ctx
            .add_from_config(FilterConfig::MacroblockGlitch { magnitude: 0.5 })
            .unwrap_err();
        assert!(err.to_string().contains("WebP-only"));
        Ok(())
    }

    fn build_synthetic_png(
        width: u32,
        height: u32,
        color_type: u8,
        bit_depth: u8,
        plte: Option<&[u8]>,
    ) -> Vec<u8> {
        let samples_per_pixel: usize = match color_type {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => unreachable!(),
        };
        let bits_per_pixel = samples_per_pixel * (bit_depth as usize);
        let row_bytes = (width as usize * bits_per_pixel).div_ceil(8);
        let mut raw = Vec::with_capacity(height as usize * (1 + row_bytes));
        for y in 0..height as usize {
            raw.push(0); // FilterType::None
            for x in 0..row_bytes {
                raw.push(((x * 37 + y * 13 + 7) & 0xFF) as u8);
            }
        }
        let compressed = fdeflate::compress_to_vec(&raw);

        let write_chunk = |out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(tag);
            out.extend_from_slice(data);
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(tag);
            hasher.update(data);
            out.extend_from_slice(&hasher.finalize().to_be_bytes());
        };

        let mut png = Vec::from(&b"\x89PNG\r\n\x1a\n"[..]);
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(bit_depth);
        ihdr.push(color_type);
        ihdr.push(0); // compression
        ihdr.push(0); // filter
        ihdr.push(0); // interlace
        write_chunk(&mut png, b"IHDR", &ihdr);
        if let Some(palette) = plte {
            write_chunk(&mut png, b"PLTE", palette);
        }
        write_chunk(&mut png, b"IDAT", &compressed);
        write_chunk(&mut png, b"IEND", &[]);
        png
    }

    fn all_png_filter_configs(magnitude: f64) -> Vec<FilterConfig> {
        vec![
            FilterConfig::ChangeFilterType { magnitude },
            FilterConfig::Replace { magnitude },
            FilterConfig::Transpose { magnitude },
            FilterConfig::SetZero { magnitude },
            FilterConfig::RandomCopy {
                times: if magnitude == 0.0 { 0 } else { 4 },
            },
            FilterConfig::Substitute {
                index: 1,
                value: 42,
            },
            FilterConfig::PixelSort {
                magnitude,
                criterion: Some(SortCriterion::Brightness),
            },
            FilterConfig::PixelSort {
                magnitude,
                criterion: Some(SortCriterion::Hue),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::Xor),
                value: Some(0xAA),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::And),
                value: Some(0xF0),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::Or),
                value: Some(0x0F),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Rg),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Gb),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Br),
            },
            FilterConfig::HorizontalShift { magnitude },
            FilterConfig::BlockScramble {
                magnitude,
                block_size: Some(4),
            },
            FilterConfig::ColorDistortion {
                magnitude,
                strength: Some(15),
            },
            FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift: Some(120.0),
                saturation_mult: Some(1.2),
                lightness_mult: Some(0.9),
            },
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset: Some(2),
                g_offset: Some(0),
                b_offset: Some(-2),
            },
            FilterConfig::Invert,
            FilterConfig::Brighten { strength: 10 },
            FilterConfig::Darken { strength: 10 },
            FilterConfig::ShiftChannels { r: 5, g: -5, b: 10 },
            FilterConfig::RemoveFilter,
            FilterConfig::SubFilter,
            FilterConfig::UpFilter,
            FilterConfig::AverageFilter,
            FilterConfig::PaethFilter,
            FilterConfig::SetFilterType {
                filter: PreProcess::PaethFilter,
            },
            FilterConfig::Regenerate {
                strategy: RegenerateStrategy::Adaptive,
            },
        ]
    }

    #[test]
    fn test_all_png_filters_across_color_types_and_bit_depths() -> Result<()> {
        let palette: Vec<u8> = (0..=255).flat_map(|i: u8| [i, 255 - i, i / 2]).collect();
        let formats: &[(u8, u8, Option<&[u8]>)] = &[
            (0, 1, None),           // Grayscale 1-bit
            (0, 2, None),           // Grayscale 2-bit
            (0, 4, None),           // Grayscale 4-bit
            (0, 8, None),           // Grayscale 8-bit
            (0, 16, None),          // Grayscale 16-bit
            (2, 8, None),           // RGB 8-bit
            (2, 16, None),          // RGB 16-bit
            (3, 1, Some(&palette)), // Indexed 1-bit
            (3, 2, Some(&palette)), // Indexed 2-bit
            (3, 4, Some(&palette)), // Indexed 4-bit
            (3, 8, Some(&palette)), // Indexed 8-bit
            (4, 8, None),           // GrayAlpha 8-bit
            (4, 16, None),          // GrayAlpha 16-bit
            (6, 8, None),           // RGBA 8-bit
            (6, 16, None),          // RGBA 16-bit
        ];

        for &(color_type, bit_depth, plte) in formats {
            let png_bytes = build_synthetic_png(16, 16, color_type, bit_depth, plte);
            for cfg in all_png_filter_configs(1.0) {
                let is_raw_filter_override = matches!(cfg, FilterConfig::ChangeFilterType { .. });
                let mut ctx = GlitchContext::new(&png_bytes, Some(99))?;
                ctx.add_from_config(cfg)?;
                ctx.execute();
                let out = ctx.buffer()?;
                assert!(!out.is_empty());

                // Verify round-trip decoding with standard `image` decoder for all filters
                // that preserve valid PNG filter type bytes.
                if !is_raw_filter_override {
                    let decoded =
                        image::load_from_memory_with_format(&out, image::ImageFormat::Png)
                            .expect("glitched PNG should decode cleanly");
                    assert_eq!((decoded.width(), decoded.height()), (16, 16));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_boundary_magnitude_zero_leaves_scanlines_unchanged() -> Result<()> {
        let png_bytes = build_synthetic_png(16, 16, 6, 8, None);
        let baseline = GlitchContext::new(&png_bytes, Some(42))?.buffer()?;

        let magnitude_filters = vec![
            FilterConfig::ChangeFilterType { magnitude: 0.0 },
            FilterConfig::Replace { magnitude: 0.0 },
            FilterConfig::Transpose { magnitude: 0.0 },
            FilterConfig::SetZero { magnitude: 0.0 },
            FilterConfig::RandomCopy { times: 0 },
            FilterConfig::PixelSort {
                magnitude: 0.0,
                criterion: Some(SortCriterion::Brightness),
            },
            FilterConfig::Bitwise {
                magnitude: 0.0,
                op: Some(BitOp::Xor),
                value: Some(255),
            },
            FilterConfig::ChannelSwap {
                magnitude: 0.0,
                target: Some(SwapTarget::Rg),
            },
            FilterConfig::HorizontalShift { magnitude: 0.0 },
            FilterConfig::BlockScramble {
                magnitude: 0.0,
                block_size: Some(4),
            },
            FilterConfig::ColorDistortion {
                magnitude: 0.0,
                strength: Some(20),
            },
            FilterConfig::ColorSpaceGlitch {
                magnitude: 0.0,
                hue_shift: Some(90.0),
                saturation_mult: Some(1.5),
                lightness_mult: Some(1.5),
            },
            FilterConfig::ChromaticAberration {
                magnitude: 0.0,
                r_offset: Some(4),
                g_offset: Some(0),
                b_offset: Some(-4),
            },
        ];

        for cfg in magnitude_filters {
            let mut ctx = GlitchContext::new(&png_bytes, Some(42))?;
            ctx.add_from_config(cfg.clone())?;
            ctx.execute();
            assert_eq!(
                ctx.buffer()?,
                baseline,
                "magnitude 0.0 should leave image unchanged for {cfg:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_seed_reproducibility() -> Result<()> {
        let png_bytes = build_synthetic_png(32, 32, 6, 8, None);

        let run_with_seed = |seed: u64| -> Result<Vec<u8>> {
            let mut ctx = GlitchContext::new(&png_bytes, Some(seed))?;
            for cfg in all_png_filter_configs(0.5) {
                ctx.add_from_config(cfg)?;
            }
            ctx.execute();
            Ok(ctx.buffer()?)
        };

        let a1 = run_with_seed(2026)?;
        let a2 = run_with_seed(2026)?;
        let b = run_with_seed(2027)?;
        assert_eq!(a1, a2, "same seed must produce identical output bytes");
        assert_ne!(
            a1, b,
            "different seeds should produce different output bytes"
        );
        Ok(())
    }

    #[test]
    fn test_yaml_deserialization_kebab_and_pascal_case() {
        let yaml = r#"
- type: change-filter-type
  magnitude: 0.1
- type: ChangeFilterType
  magnitude: 0.1
- type: replace
  magnitude: 0.2
- type: transpose
  magnitude: 0.3
- type: set-zero
  magnitude: 0.4
- type: random-copy
  times: 3
- type: substitute
  index: 2
  value: 128
- type: pixel-sort
  magnitude: 0.5
  criterion: hue
- type: bitwise
  magnitude: 0.5
  op: xor
  value: 255
- type: channel-swap
  magnitude: 0.5
  target: br
- type: horizontal-shift
  magnitude: 0.5
- type: block-scramble
  magnitude: 0.5
  block_size: 8
- type: color-distortion
  magnitude: 0.5
  strength: 30
- type: color-space-glitch
  magnitude: 0.5
  hue_shift: 45.0
  saturation_mult: 1.2
  lightness_mult: 0.8
- type: chromatic-aberration
  magnitude: 0.5
  r_offset: 3
  g_offset: 0
  b_offset: -3
- type: invert
- type: brighten
  strength: 15
- type: darken
  strength: 15
- type: shift-channels
  r: 10
  g: -10
  b: 5
- type: remove-filter
- type: sub-filter
- type: up-filter
- type: average-filter
- type: paeth-filter
- type: macroblock-glitch
  magnitude: 0.25
- type: alpha-glitch
  magnitude: 0.75
  strategy: randomize
- type: lossy-artifact
  quality: 15.0
"#;
        let configs: Vec<FilterConfig> =
            serde_saphyr::from_str(yaml).expect("all filter configs should deserialize");
        assert_eq!(configs.len(), 27);

        // Verify PreProcess accepts kebab-case, PascalCase, and lowercase aliases
        for s in ["remove-filter", "RemoveFilter", "removefilter", "none"] {
            let p: PreProcess = serde_saphyr::from_str(s).unwrap();
            assert_eq!(p, PreProcess::RemoveFilter);
        }
        for s in ["sub-filter", "SubFilter", "subfilter", "sub"] {
            let p: PreProcess = serde_saphyr::from_str(s).unwrap();
            assert_eq!(p, PreProcess::SubFilter);
        }
    }

    #[test]
    fn test_1x1_and_adversarial_filter_parameters_never_panic() -> Result<()> {
        // 1x1 1-bit grayscale (1 byte per row)
        let png_1x1 = build_synthetic_png(1, 1, 0, 1, None);
        let mut ctx = GlitchContext::new(&png_1x1, Some(1))?;
        for cfg in all_png_filter_configs(1.0) {
            ctx.add_from_config(cfg)?;
        }
        // Also test negative ColorDistortion::strength (including i16::MIN), NaN ColorSpaceGlitch, and huge RandomCopy::times via direct struct construction
        ctx.add_filter(ColorDistortion {
            magnitude: 1.0,
            strength: -5,
        });
        ctx.add_filter(ColorDistortion {
            magnitude: 1.0,
            strength: i16::MIN,
        });
        ctx.add_filter(ColorSpaceGlitch {
            magnitude: 1.0,
            hue_shift: f64::NAN,
            saturation_mult: f64::INFINITY,
            lightness_mult: -1.0,
        });
        ctx.add_filter(RandomCopy::new(u32::MAX));
        ctx.execute();
        assert!(!ctx.buffer()?.is_empty());

        // Config validation rejects negative strength, NaN/negative multipliers, and excessive RandomCopy::times
        assert!(
            ctx.add_from_config(FilterConfig::ColorDistortion {
                magnitude: 0.5,
                strength: Some(-5),
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::ColorSpaceGlitch {
                magnitude: 0.5,
                hue_shift: Some(f64::NAN),
                saturation_mult: None,
                lightness_mult: None,
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::ColorSpaceGlitch {
                magnitude: 0.5,
                hue_shift: None,
                saturation_mult: Some(-0.5),
                lightness_mult: None,
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::RandomCopy {
                times: MAX_RANDOM_COPY_TIMES + 1,
            })
            .is_err()
        );
        Ok(())
    }
}
