//! Tests for the `Recipe` → resolve → `ResolvedRecipe` → execute API.

use png_glitch_recipe::{
    FilterConfig, GlitchContext, ImageHeader, PngGlitch, Recipe, RecipeError, Step,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

fn recipe() -> Recipe {
    Recipe::new()
        .then(FilterConfig::Replace { magnitude: 0.3 })
        .then(FilterConfig::Transpose { magnitude: 0.3 })
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

fn via_context(seed: u64) -> Vec<u8> {
    let mut ctx = GlitchContext::new(FIXTURE, Some(seed)).unwrap();
    for step in recipe().steps() {
        let Step::Config(config) = step else {
            unreachable!()
        };
        ctx.add_from_config(config.clone()).unwrap();
    }
    ctx.execute();
    ctx.buffer().unwrap()
}

fn via_recipe(recipe: &Recipe, seed: u64) -> Vec<u8> {
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    recipe.run(&mut png, Some(seed)).unwrap();
    png.to_bytes().unwrap()
}

#[test]
fn recipe_matches_context_for_every_seed() {
    let recipe = recipe();
    for seed in 0..5 {
        assert_eq!(via_recipe(&recipe, seed), via_context(seed));
    }
}

#[test]
fn one_recipe_serves_many_images_and_seeds() {
    let recipe = recipe();
    let a = via_recipe(&recipe, 1);
    let b = via_recipe(&recipe, 2);
    assert_ne!(a, b);
    assert_eq!(a, via_recipe(&recipe, 1));
    assert_eq!(recipe, recipe.clone());
}

#[test]
fn resolved_recipe_replays_without_a_seed() {
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let resolved = recipe().resolve(&ImageHeader::of(&png), Some(7)).unwrap();
    assert_eq!(resolved.seed(), Some(7));
    resolved.execute(&mut png).unwrap();
    assert_eq!(png.to_bytes().unwrap(), via_context(7));

    let mut again = PngGlitch::from_bytes(FIXTURE).unwrap();
    resolved.execute(&mut again).unwrap();
    assert_eq!(again.to_bytes().unwrap(), via_context(7));
}

#[test]
fn executing_on_a_different_header_is_an_error() {
    let png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let mut header = ImageHeader::of(&png);
    header.width += 1;
    let resolved = recipe().resolve(&header, Some(1)).unwrap();
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    assert!(matches!(
        resolved.execute(&mut png),
        Err(RecipeError::HeaderMismatch { .. })
    ));
}

#[test]
fn invalid_steps_fail_at_resolve_time() {
    let recipe = Recipe::new().then(FilterConfig::RandomCopy { times: u32::MAX });
    let header = ImageHeader::of(&PngGlitch::from_bytes(FIXTURE).unwrap());
    assert!(matches!(
        recipe.resolve(&header, Some(1)),
        Err(RecipeError::InvalidSpec(_))
    ));
}

#[test]
fn header_line_size_matches_scan_lines() {
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let header = ImageHeader::of(&png);
    assert_eq!(header.line_size(), png.scan_lines()[0].size());
}

#[derive(Debug)]
struct BlankFirstByte(Vec<u32>);

impl png_glitch::effects::Effect for BlankFirstByte {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for &row in &self.0 {
            if let Some(line) = lines.get_mut(row as usize) {
                line.update(0, 0xAA);
            }
        }
    }
}

#[test]
fn custom_steps_resolve_with_the_seeded_rng_and_replay() {
    use png_glitch_recipe::ResolvedEffect;
    use png_glitch_recipe::rand::{Rng, RngCore};

    let recipe = Recipe::new()
        .then(FilterConfig::Invert)
        .then_step(|header: &ImageHeader, rng: &mut dyn RngCore| {
            let rows = (0..header.height)
                .filter(|_| rng.random_bool(0.5))
                .collect();
            ResolvedEffect::new(BlankFirstByte(rows))
        })
        .then_effect(BlankFirstByte(vec![0]));

    assert_eq!(via_recipe(&recipe, 3), via_recipe(&recipe, 3));
    assert_ne!(via_recipe(&recipe, 3), via_recipe(&recipe, 4));
    assert_eq!(recipe, recipe.clone());

    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let resolved = recipe.resolve(&ImageHeader::of(&png), Some(3)).unwrap();
    resolved.execute(&mut png).unwrap();
    assert_eq!(png.to_bytes().unwrap(), via_recipe(&recipe, 3));
}

#[test]
fn context_exposes_the_image_and_closures() {
    let mut ctx = GlitchContext::new(FIXTURE, Some(1)).unwrap();
    let before = ctx.png().to_bytes().unwrap();
    ctx.add_fn(|png, _rng| {
        png.apply(BlankFirstByte(vec![0]));
    });
    ctx.execute();
    ctx.png_mut().transpose(0, 1, 1);
    let inner = ctx.into_inner();
    assert_ne!(inner.to_bytes().unwrap(), before);
}

fn every_config() -> Vec<FilterConfig> {
    use png_glitch_recipe::{AlphaGlitchStrategy, BitOp, SortCriterion, SwapTarget};
    vec![
        FilterConfig::ChangeFilterType { magnitude: 0.25 },
        FilterConfig::Replace { magnitude: 0.1 },
        FilterConfig::Transpose { magnitude: 1.0 },
        FilterConfig::SetZero { magnitude: 0.0 },
        FilterConfig::RandomCopy { times: 5 },
        FilterConfig::Substitute {
            index: 4,
            value: 255,
        },
        FilterConfig::PixelSort {
            magnitude: 0.2,
            criterion: None,
        },
        FilterConfig::PixelSort {
            magnitude: 0.2,
            criterion: Some(SortCriterion::Hue),
        },
        FilterConfig::Bitwise {
            magnitude: 0.3,
            op: Some(BitOp::And),
            value: Some(7),
        },
        FilterConfig::Bitwise {
            magnitude: 0.3,
            op: None,
            value: None,
        },
        FilterConfig::ChannelSwap {
            magnitude: 0.5,
            target: Some(SwapTarget::Br),
        },
        FilterConfig::HorizontalShift { magnitude: 0.05 },
        FilterConfig::BlockScramble {
            magnitude: 0.1,
            block_size: Some(32),
        },
        FilterConfig::ColorDistortion {
            magnitude: 0.2,
            strength: Some(30),
        },
        FilterConfig::ColorSpaceGlitch {
            magnitude: 0.2,
            hue_shift: Some(-90.5),
            saturation_mult: Some(1.5),
            lightness_mult: None,
        },
        FilterConfig::ChromaticAberration {
            magnitude: 0.2,
            r_offset: Some(4),
            g_offset: None,
            b_offset: Some(-4),
        },
        FilterConfig::Invert,
        FilterConfig::Brighten { strength: 30 },
        FilterConfig::Darken { strength: 5 },
        FilterConfig::ShiftChannels {
            r: 15,
            g: 0,
            b: -15,
        },
        FilterConfig::RemoveFilter,
        FilterConfig::SubFilter,
        FilterConfig::UpFilter,
        FilterConfig::AverageFilter,
        FilterConfig::PaethFilter,
        FilterConfig::MacroblockGlitch { magnitude: 0.1 },
        FilterConfig::AlphaGlitch {
            magnitude: 0.5,
            strategy: Some(AlphaGlitchStrategy::Zero),
        },
        FilterConfig::LossyArtifact { quality: 15.5 },
    ]
}

#[test]
fn filter_config_round_trips_through_yaml() {
    for config in every_config() {
        let yaml = serde_saphyr::to_string(&config).unwrap();
        let back: FilterConfig = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(back, config, "{yaml}");
    }
}

#[test]
fn recipe_round_trips_and_matches_the_config_file_shape() {
    let recipe = Recipe::from_configs(every_config());
    let yaml = serde_saphyr::to_string(&recipe).unwrap();
    assert!(yaml.starts_with("filters:"), "{yaml}");
    let back: Recipe = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, recipe);
}

#[test]
fn recipe_with_custom_step_cannot_be_serialized() {
    let recipe = Recipe::new().then_effect(BlankFirstByte(vec![0]));
    assert!(recipe.configs().is_none());
    assert!(serde_saphyr::to_string(&recipe).is_err());
}

#[test]
fn display_renders_filter_specs() {
    let shown: Vec<String> = every_config().iter().map(ToString::to_string).collect();
    assert_eq!(shown[0], "change-filter-type:magnitude=0.25");
    assert_eq!(shown[7], "pixel-sort:magnitude=0.2,criterion=hue");
    assert_eq!(shown[16], "invert");
    assert_eq!(shown[19], "shift-channels:r=15,g=0,b=-15");
}
