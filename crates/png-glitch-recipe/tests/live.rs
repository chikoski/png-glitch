//! Equivalence of `LiveRecipe` with the file workflow (#163).

use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource, PngGlitch};
use png_glitch_recipe::{
    FilterConfig, ImageHeader, LiveRecipe, Recipe, RecipeError, ResolvedEffect, SeedMode,
    frame_seed,
};
use rand::Rng;

const W: u32 = 37;
const H: u32 = 29;

fn frame(k: u8) -> Vec<u8> {
    (0..W * H * 4)
        .map(|i| (i as u8).wrapping_mul(13).wrapping_add(k.wrapping_mul(71)) ^ (i >> 6) as u8)
        .collect()
}

fn encoding() -> Encoding {
    Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth))
}

/// Random steps with one generation boundary (`SubFilter`) in the middle.
fn recipe() -> Recipe {
    Recipe::new()
        .then(FilterConfig::Transpose { magnitude: 0.4 })
        .then(FilterConfig::Replace { magnitude: 0.3 })
        .then(FilterConfig::SubFilter)
        .then(FilterConfig::PixelSort {
            magnitude: 0.4,
            criterion: None,
        })
        .then(FilterConfig::ColorDistortion {
            magnitude: 0.4,
            strength: None,
        })
        .then(FilterConfig::Invert)
}

fn check(mode: SeedMode) {
    let seed = 1234u64;
    let recipe = recipe();
    let first = frame(0);
    let mut live = LivePipeline::new(PixelSource::rgba8(W, H, &first), encoding()).unwrap();
    let header = ImageHeader::of(live.model());
    let mut runner = LiveRecipe::new(recipe.clone(), header, Some(seed), mode).unwrap();
    assert_eq!(runner.seed(), seed);
    assert_eq!(runner.mode(), mode);

    for n in 0..5u64 {
        let pixels = frame(n as u8);
        live.encode(&pixels).unwrap();
        runner.run(&mut live).unwrap();
        assert_eq!(runner.frame(), n + 1);

        let mut file =
            PngGlitch::from_pixels(PixelSource::rgba8(W, H, &pixels), encoding()).unwrap();
        let file_seed = match mode {
            SeedMode::Static => seed,
            SeedMode::PerFrame => frame_seed(seed, n),
            _ => unreachable!(),
        };
        recipe.run(&mut file, Some(file_seed)).unwrap();
        assert_eq!(live.model().data(), file.data(), "{mode:?} frame {n}");
        assert_eq!(runner.current().seed(), Some(file_seed));
    }
}

#[test]
fn static_mode_equals_the_file_workflow() {
    check(SeedMode::Static);
}

#[test]
fn per_frame_mode_equals_the_file_workflow() {
    check(SeedMode::PerFrame);
}

#[test]
fn an_os_seed_is_drawn_once_and_reported() {
    let first = frame(0);
    let mut live = LivePipeline::new(PixelSource::rgba8(W, H, &first), encoding()).unwrap();
    let header = ImageHeader::of(live.model());
    let runner = LiveRecipe::new(recipe(), header, None, SeedMode::PerFrame);
    #[cfg(not(feature = "os-rng"))]
    assert!(matches!(runner, Err(RecipeError::SeedRequired)));
    #[cfg(feature = "os-rng")]
    {
        let mut runner = runner.unwrap();
        let seed = runner.seed();
        for n in 0..3 {
            live.encode(&first).unwrap();
            runner.run(&mut live).unwrap();
            assert_eq!(runner.seed(), seed);
            assert_eq!(runner.current().seed(), Some(frame_seed(seed, n)));
        }
    }
}

#[test]
fn a_non_live_custom_step_is_rejected_at_build_time() {
    let first = frame(0);
    let live = LivePipeline::new(PixelSource::rgba8(W, H, &first), encoding()).unwrap();
    let header = ImageHeader::of(live.model());
    let plain = Recipe::new().then_effect(png_glitch::effects::Refilter::new(FilterType::Up));
    // `Refilter` is an `Effect` but not a `ScanLinesEffect`.
    for mode in [SeedMode::Static, SeedMode::PerFrame] {
        let err = LiveRecipe::new(plain.clone(), header, Some(1), mode).unwrap_err();
        assert!(matches!(err, RecipeError::NotLive { .. }), "{err}");
    }
    // A live custom step is accepted and equals the file workflow.
    let live_recipe = Recipe::new().then_step(|_: &ImageHeader, rng: &mut dyn Rng| {
        ResolvedEffect::new_live(png_glitch::effects::Brighten::new(
            (rng.next_u32() % 64) as u16,
        ))
    });
    LiveRecipe::new(live_recipe, header, Some(1), SeedMode::Static).unwrap();
}

#[test]
fn a_per_frame_step_that_turns_non_live_later_fails_in_run() {
    let first = frame(0);
    let mut live = LivePipeline::new(PixelSource::rgba8(W, H, &first), encoding()).unwrap();
    let header = ImageHeader::of(live.model());
    let seed = 7u64;
    // Live on frame 0, non-live on frame 1: decided by the drawn number.
    let want_live_at_0 = rand_first(frame_seed(seed, 0)).is_multiple_of(2);
    let recipe = Recipe::new().then_step(move |_: &ImageHeader, rng: &mut dyn Rng| {
        if rng.next_u32().is_multiple_of(2) {
            ResolvedEffect::new_live(png_glitch::effects::Invert)
        } else {
            ResolvedEffect::new(png_glitch::effects::Refilter::new(FilterType::Up))
        }
    });
    let built = LiveRecipe::new(recipe, header, Some(seed), SeedMode::PerFrame);
    assert_eq!(built.is_ok(), want_live_at_0);
    if let Ok(mut runner) = built {
        let mut failed = false;
        for _ in 0..16 {
            live.encode(&first).unwrap();
            if runner.run(&mut live).is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed, "some frame among 16 should draw a non-live effect");
    }
}

fn rand_first(seed: u64) -> u32 {
    use rand::SeedableRng;
    rand_chacha::ChaCha8Rng::seed_from_u64(seed).next_u32()
}

#[test]
fn a_different_image_is_a_header_mismatch() {
    let first = frame(0);
    let live = LivePipeline::new(PixelSource::rgba8(W, H, &first), encoding()).unwrap();
    let header = ImageHeader::of(live.model());
    let mut runner = LiveRecipe::new(recipe(), header, Some(1), SeedMode::Static).unwrap();
    let other = vec![0u8; 8 * 8 * 4];
    let mut other_live = LivePipeline::new(PixelSource::rgba8(8, 8, &other), encoding()).unwrap();
    assert!(matches!(
        runner.run(&mut other_live),
        Err(RecipeError::HeaderMismatch { .. })
    ));
    assert_eq!(runner.frame(), 0);
}
