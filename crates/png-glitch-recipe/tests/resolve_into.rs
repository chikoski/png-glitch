//! `Recipe::resolve_into` gives exactly what `Recipe::resolve` gives (#165), whatever the
//! `ResolvedRecipe` it writes into held before. This is the oracle for the reproducibility
//! guarantee (#58): the RNG draws must be the same in number and order.

use png_glitch::ColorType;
use png_glitch_recipe::rand::{Rng, SeedableRng};
use png_glitch_recipe::{
    BitOp, ChaCha8Rng, FilterConfig, ImageHeader, Invert, Recipe, SortCriterion, SwapTarget,
};

/// One random config of every built-in kind, in the order of `kind`.
fn config(kind: usize, rng: &mut ChaCha8Rng) -> FilterConfig {
    // Mostly in range, with the edges 0.0 and 1.0 now and then.
    let m = match rng.random_range(0..8) {
        0 => 0.0,
        1 => 1.0,
        _ => rng.random_range(0.0..=1.0),
    };
    let some = |rng: &mut ChaCha8Rng| rng.random_bool(0.5);
    match kind {
        0 => FilterConfig::ChangeFilterType { magnitude: m },
        1 => FilterConfig::Replace { magnitude: m },
        2 => FilterConfig::Transpose { magnitude: m },
        3 => FilterConfig::SetZero { magnitude: m },
        4 => FilterConfig::RandomCopy {
            times: rng.random_range(0..50),
        },
        5 => FilterConfig::Substitute {
            index: rng.random_range(0..40),
            value: rng.random(),
        },
        6 => FilterConfig::PixelSort {
            magnitude: m,
            criterion: some(rng).then_some(SortCriterion::Hue),
        },
        7 => FilterConfig::Bitwise {
            magnitude: m,
            op: some(rng).then_some(BitOp::And),
            value: some(rng).then(|| rng.random()),
        },
        8 => FilterConfig::ChannelSwap {
            magnitude: m,
            target: some(rng).then_some(SwapTarget::Br),
        },
        9 => FilterConfig::HorizontalShift { magnitude: m },
        10 => FilterConfig::BlockScramble {
            magnitude: m,
            block_size: some(rng).then(|| rng.random_range(0..12)),
        },
        11 => FilterConfig::ColorDistortion {
            magnitude: m,
            strength: some(rng).then(|| rng.random_range(0..40)),
        },
        12 => FilterConfig::ColorSpaceGlitch {
            magnitude: m,
            hue_shift: some(rng).then(|| rng.random_range(0.0..360.0)),
            saturation_mult: some(rng).then(|| rng.random_range(0.0..2.0)),
            lightness_mult: some(rng).then(|| rng.random_range(0.0..2.0)),
        },
        13 => FilterConfig::ChromaticAberration {
            magnitude: m,
            r_offset: some(rng).then(|| rng.random_range(-5..5)),
            g_offset: some(rng).then(|| rng.random_range(-5..5)),
            b_offset: some(rng).then(|| rng.random_range(-5..5)),
        },
        14 => FilterConfig::Invert,
        15 => FilterConfig::Brighten {
            strength: rng.random(),
        },
        16 => FilterConfig::Darken {
            strength: rng.random(),
        },
        17 => FilterConfig::ShiftChannels {
            r: rng.random(),
            g: rng.random(),
            b: rng.random(),
        },
        18 => FilterConfig::RemoveFilter,
        19 => FilterConfig::SubFilter,
        20 => FilterConfig::UpFilter,
        21 => FilterConfig::AverageFilter,
        _ => FilterConfig::PaethFilter,
    }
}

const KINDS: usize = 23;

fn random_recipe(rng: &mut ChaCha8Rng) -> Recipe {
    let len = rng.random_range(0..8);
    let mut recipe = Recipe::new();
    for _ in 0..len {
        recipe = if rng.random_bool(0.1) {
            recipe.then_effect(Invert)
        } else {
            let kind = rng.random_range(0..KINDS);
            recipe.then(config(kind, rng))
        };
    }
    recipe
}

fn random_header(rng: &mut ChaCha8Rng) -> ImageHeader {
    let (color_type, bit_depth) = match rng.random_range(0..5) {
        0 => (ColorType::TrueColorAlpha, 8),
        1 => (ColorType::TrueColor, 8),
        2 => (ColorType::GrayScaleAlpha, 8),
        3 => (ColorType::GrayScale, 1),
        _ => (ColorType::TrueColorAlpha, 16),
    };
    ImageHeader {
        width: rng.random_range(0..40),
        height: rng.random_range(0..40),
        color_type,
        bit_depth,
    }
}

#[test]
fn every_built_in_kind_resolves_into_the_same_steps() {
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let header = ImageHeader {
        width: 33,
        height: 29,
        color_type: ColorType::TrueColorAlpha,
        bit_depth: 8,
    };
    for kind in 0..KINDS {
        let recipe = Recipe::new().then(config(kind, &mut rng));
        let mut out = recipe.resolve(&header, Some(0)).unwrap();
        for seed in 1..50 {
            recipe.resolve_into(&header, Some(seed), &mut out).unwrap();
            assert_eq!(
                out,
                recipe.resolve(&header, Some(seed)).unwrap(),
                "kind {kind}"
            );
        }
    }
}

#[test]
fn resolve_into_a_reused_recipe_equals_resolve() {
    let mut rng = ChaCha8Rng::seed_from_u64(165);
    // Built up across iterations: it holds results for other recipes, headers and seeds.
    let mut out = Recipe::new()
        .resolve(&random_header(&mut rng), Some(0))
        .unwrap();
    for _ in 0..300 {
        let recipe = random_recipe(&mut rng);
        let header = random_header(&mut rng);
        for _ in 0..8 {
            let seed = rng.random();
            let expected = recipe.resolve(&header, Some(seed)).unwrap();
            recipe.resolve_into(&header, Some(seed), &mut out).unwrap();
            assert_eq!(out, expected, "{recipe:?} {header:?} seed {seed}");
            assert_eq!(out.seed(), Some(seed));
            assert_eq!(out.header(), &header);
        }
        // Sometimes start over from another recipe's result.
        if rng.random_bool(0.3) {
            let other = random_recipe(&mut rng);
            out = other.resolve(&header, Some(rng.random())).unwrap();
        }
    }
}

#[test]
fn resolve_into_fails_like_resolve() {
    let header = ImageHeader {
        width: 4,
        height: 4,
        color_type: ColorType::TrueColorAlpha,
        bit_depth: 8,
    };
    let recipe = Recipe::new()
        .then(FilterConfig::Invert)
        .then(FilterConfig::ColorDistortion {
            magnitude: 0.5,
            strength: Some(-1),
        });
    let mut out = Recipe::new().resolve(&header, Some(0)).unwrap();
    let a = recipe.resolve(&header, Some(1)).unwrap_err();
    let b = recipe.resolve_into(&header, Some(1), &mut out).unwrap_err();
    assert_eq!(a.to_string(), b.to_string());
}
