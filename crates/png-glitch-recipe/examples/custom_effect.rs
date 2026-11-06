//! Mixes a built-in filter, a user-defined effect and direct `PngGlitch` calls.
//!
//! A custom effect is a `png-glitch` [`Effect`]; a recipe step draws its parameters from the
//! RNG and returns it. The same pieces work in a `GlitchContext` and in a reusable `Recipe`.

use png_glitch::effects::{Effect, SwapLines};
use png_glitch_recipe::rand::{Rng, RngCore};
use png_glitch_recipe::{
    FilterConfig, GlitchContext, ImageHeader, PngGlitch, Recipe, ResolvedEffect,
};
use std::path::Path;

/// A deterministic user-defined effect: zeroes the first byte of the listed rows.
#[derive(Debug)]
struct BlankRows(Vec<u32>);

impl Effect for BlankRows {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for &row in &self.0 {
            if let Some(line) = lines.get_mut(row as usize) {
                line.update(0, 0);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../png-glitch/etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    std::fs::create_dir_all(&out_dir)?;

    // 1. In a context: a built-in filter, a closure and a direct PngGlitch call.
    let mut ctx = GlitchContext::open(&input, Some(1))?;
    ctx.add_from_config(FilterConfig::Replace { magnitude: 0.05 })?;
    ctx.add_fn(|png, rng| {
        let row = rng.random_range(0..png.height());
        png.apply(BlankRows(vec![row]));
    });
    ctx.execute();
    ctx.png_mut().transpose(0, 1, 1); // low-level call on the same image
    ctx.save(out_dir.join("custom_context.png"))?;

    // 2. In a recipe: reusable for any image and seed.
    let recipe = Recipe::new()
        .then(FilterConfig::Replace { magnitude: 0.05 })
        .then_step(|header: &ImageHeader, rng: &mut dyn RngCore| {
            let rows = (0..header.height)
                .filter(|_| rng.random_bool(0.1))
                .collect();
            ResolvedEffect::new(BlankRows(rows))
        })
        .then_effect(SwapLines::new(vec![(0, 1)]));
    for seed in 0..3 {
        let mut png = PngGlitch::open(&input)?;
        recipe.run(&mut png, Some(seed))?;
        png.save(out_dir.join(format!("custom_recipe_{seed}.png")))?;
    }
    println!("Wrote images to {}", out_dir.display());
    Ok(())
}
