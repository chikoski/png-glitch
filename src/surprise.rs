//! `--surprise`: a random recipe of 2-5 built-in steps, drawn from a ChaCha8 RNG seeded with
//! the run's seed. The same `--seed` gives the same recipe and therefore the same image.

use crate::cli::Cli;
use crate::{
    OrderedStep, check_overwrite, recipe_io, resolve_output_path, resolve_seed, save_output,
    validate_flags_for_format,
};
use anyhow::{Context, bail};
use png_glitch_recipe::{FilterConfig, ParseOptions};
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::fs;
use std::path::Path;

/// Kinds of step `--surprise` can draw.
///
/// The ranges drawn below are deliberately narrower than the valid ranges in
/// `png_glitch_recipe::filters()`: they keep the image recognisable, and changing them changes
/// the image a given `--seed` produces. The tests check that they stay inside the valid ranges
/// and that the kinds agree with each filter's supported formats.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Replace,
    SetZero,
    Transpose,
    ChangeFilterType,
    HorizontalShift,
    BlockScramble,
    ColorDistortion,
    ChromaticAberration,
    PixelSort,
    Bitwise,
    ChannelSwap,
    RandomCopy,
    Invert,
    Brighten,
    Darken,
    ShiftChannels,
}

use Kind::*;

/// Steps that work on PNG input.
const PNG_KINDS: &[Kind] = &[
    Replace,
    SetZero,
    Transpose,
    ChangeFilterType,
    HorizontalShift,
    BlockScramble,
    ColorDistortion,
    ChromaticAberration,
    PixelSort,
    Bitwise,
    ChannelSwap,
    RandomCopy,
    Invert,
    Brighten,
    Darken,
    ShiftChannels,
];

/// Steps that also work on WebP input.
const WEBP_KINDS: &[Kind] = &[
    PixelSort,
    Bitwise,
    ChannelSwap,
    Invert,
    Brighten,
    Darken,
    ShiftChannels,
];

/// Magnitudes stay in a range where the image is still recognisable.
fn magnitude(rng: &mut ChaCha8Rng) -> f64 {
    // Rounded to 3 decimals so the printed recipe is short and round-trips exactly.
    (rng.random_range(0.01..=0.3_f64) * 1000.0).round() / 1000.0
}

fn make(kind: Kind, rng: &mut ChaCha8Rng) -> FilterConfig {
    match kind {
        Replace => FilterConfig::Replace {
            magnitude: magnitude(rng),
        },
        SetZero => FilterConfig::SetZero {
            magnitude: magnitude(rng),
        },
        Transpose => FilterConfig::Transpose {
            magnitude: magnitude(rng),
        },
        ChangeFilterType => FilterConfig::ChangeFilterType {
            magnitude: magnitude(rng),
        },
        HorizontalShift => FilterConfig::HorizontalShift {
            magnitude: magnitude(rng),
        },
        BlockScramble => FilterConfig::BlockScramble {
            magnitude: magnitude(rng),
            block_size: Some(rng.random_range(2..=32)),
        },
        ColorDistortion => FilterConfig::ColorDistortion {
            magnitude: magnitude(rng),
            strength: Some(rng.random_range(10..=80)),
        },
        ChromaticAberration => FilterConfig::ChromaticAberration {
            magnitude: magnitude(rng),
            r_offset: Some(rng.random_range(-8..=8)),
            g_offset: None,
            b_offset: Some(rng.random_range(-8..=8)),
        },
        PixelSort => FilterConfig::PixelSort {
            magnitude: magnitude(rng),
            criterion: None,
        },
        Bitwise => FilterConfig::Bitwise {
            magnitude: magnitude(rng),
            op: None,
            value: None,
        },
        ChannelSwap => FilterConfig::ChannelSwap {
            magnitude: magnitude(rng),
            target: None,
        },
        RandomCopy => FilterConfig::RandomCopy {
            times: rng.random_range(1..=20),
        },
        Invert => FilterConfig::Invert,
        Brighten => FilterConfig::Brighten {
            strength: rng.random_range(10..=80),
        },
        Darken => FilterConfig::Darken {
            strength: rng.random_range(10..=80),
        },
        ShiftChannels => FilterConfig::ShiftChannels {
            r: rng.random_range(-40..=40),
            g: rng.random_range(-40..=40),
            b: rng.random_range(-40..=40),
        },
    }
}

/// The random recipe for `seed`: 2-5 distinct kinds of step valid for the input format.
pub fn random_recipe(seed: u64, webp: bool) -> Vec<FilterConfig> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut kinds: Vec<Kind> = if webp { WEBP_KINDS } else { PNG_KINDS }.to_vec();
    kinds.shuffle(&mut rng);
    let n = rng.random_range(2..=5).min(kinds.len());
    kinds[..n].iter().map(|&k| make(k, &mut rng)).collect()
}

pub fn surprise_process(args: &Cli, steps: &[OrderedStep]) -> anyhow::Result<()> {
    if !steps.is_empty() || args.pre_process.is_some() {
        bail!(
            "--surprise builds its own recipe; it cannot be combined with --config, -f or filter flags"
        );
    }
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    if Path::new(input).is_dir() {
        bail!("'{input}' is a directory; --surprise works on a single image");
    }
    let options = ParseOptions::default().with_strict(args.strict);
    let mut context = crate::stdio::open_context(input, args.seed, options)?;
    validate_flags_for_format(args, context.is_webp())?;
    for w in context.warnings() {
        eprintln!("warning: {w}");
    }
    let output = resolve_output_path(args, input);
    check_overwrite(&output, args.force)?;

    let seed = resolve_seed(args.seed);
    context.set_seed(Some(seed));
    let filters = random_recipe(seed, context.is_webp());
    let recipe = recipe_io::render_recipe(seed, &filters)?;
    // Always shown, so a lucky result can be re-run with `--config saved.yaml --seed S`.
    eprint!("{recipe}");
    if let Some(path) = &args.save_recipe {
        fs::write(path, &recipe).with_context(|| format!("failed to write recipe to '{path}'"))?;
    }
    for f in filters {
        context.add_from_config(f)?;
    }
    context.execute();
    save_output(args, &context, &output, &recipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_recipe() {
        for seed in 0..50 {
            assert_eq!(random_recipe(seed, false), random_recipe(seed, false));
        }
        assert_ne!(random_recipe(1, false), random_recipe(2, false));
    }

    #[test]
    fn two_to_five_steps_and_in_range() {
        for seed in 0..200 {
            for webp in [false, true] {
                let r = random_recipe(seed, webp);
                assert!((2..=5).contains(&r.len()), "{} steps", r.len());
                let yaml = recipe_io::render_recipe(seed, &r).unwrap();
                // Every recipe parses back through the normal validation.
                let cfg: crate::cli::ConfigFile = serde_saphyr::from_str(&yaml).unwrap();
                let want: Vec<png_glitch_recipe::FilterEntry> =
                    r.into_iter().map(Into::into).collect();
                assert_eq!(cfg.filters, want);
            }
        }
    }

    fn all_kinds() -> Vec<Kind> {
        let mut kinds = PNG_KINDS.to_vec();
        kinds.extend_from_slice(WEBP_KINDS);
        kinds
    }

    #[test]
    fn kinds_agree_with_the_filter_metadata_formats() {
        use png_glitch_recipe::{Format, info_for};
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        for &k in PNG_KINDS {
            let info = info_for(&make(k, &mut rng));
            assert!(info.supports(Format::Png), "{k:?} is not a PNG filter");
        }
        for &k in WEBP_KINDS {
            let info = info_for(&make(k, &mut rng));
            assert!(info.supports(Format::Webp), "{k:?} is not a WebP filter");
        }
    }

    #[test]
    fn drawn_values_lie_inside_the_metadata_ranges() {
        use png_glitch_recipe::info_for;
        for seed in 0..300 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            for k in all_kinds() {
                let config = make(k, &mut rng);
                let info = info_for(&config);
                let value = serde_json::to_value(&config).unwrap();
                for p in info.params {
                    let Some(n) = value.get(p.name).and_then(|v| v.as_f64()) else {
                        continue;
                    };
                    assert!(
                        p.min.is_none_or(|lo| n >= lo) && p.max.is_none_or(|hi| n <= hi),
                        "{}.{} = {n} is outside {:?}..={:?}",
                        info.name,
                        p.name,
                        p.min,
                        p.max
                    );
                }
            }
        }
    }
}
