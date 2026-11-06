//! Tests for the `Recipe` → resolve → `ResolvedRecipe` → execute API.

use png_glitch_recipe::{FilterConfig, GlitchContext, ImageHeader, PngGlitch, Recipe, RecipeError};

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
        ctx.add_from_config(step.clone()).unwrap();
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
