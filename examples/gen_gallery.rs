//! Generates the images in `docs/gallery`, used by `docs/CLI_GUIDE.md`.
//!
//! ```bash
//! cargo run --example gen_gallery
//! ```
//!
//! - `docs/gallery/*.png`: one PNG effect each.
//! - `docs/gallery/recipes/<name>.yaml` -> `<name>.png` (or `<name>.webp` for `webp-*` recipes):
//!   multi-filter recipes. Each output equals
//!   `png-glitch <sample> --config <name>.yaml --seed 42`.
//! - `docs/gallery/webp/*.webp`: WebP-only effects.

use anyhow::{Context, Result};
use png_glitch_recipe::{
    BitOp, Bitwise, BlockScramble, Brighten, ChangeFilterType, ChromaticAberration,
    ColorDistortion, ColorSpaceGlitch, FilterConfig, GlitchContext, GlitchFilter, HorizontalShift,
    Invert, PreProcess, RandomCopy, Regenerate, RegenerateStrategy, Replace, SetZero,
    ShiftChannels, SortCriterion, SwapTarget, Transpose,
};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use webp_glitch_context::WebpGlitchContext;

const SEED: u64 = 42;

#[derive(Deserialize)]
struct RecipeFile {
    filters: Vec<FilterConfig>,
}

fn generate_png(
    sample_bytes: &[u8],
    out_path: &Path,
    filters: impl IntoIterator<Item = FilterConfig>,
) -> Result<()> {
    let mut ctx = GlitchContext::new(sample_bytes, Some(SEED))?;
    for filter in filters {
        ctx.add_from_config(filter)?;
    }
    ctx.execute();
    ctx.save(out_path)?;
    Ok(())
}

fn generate_webp(
    sample_bytes: &[u8],
    out_path: &Path,
    filters: impl IntoIterator<Item = FilterConfig>,
) -> Result<()> {
    let mut ctx = WebpGlitchContext::new(sample_bytes, Some(SEED))?;
    for filter in filters {
        ctx.add_from_config(filter)?;
    }
    ctx.execute();
    ctx.save(out_path)?;
    Ok(())
}

/// Renders every `*.yaml` recipe in `dir` next to itself.
fn generate_recipes(png_sample: &[u8], webp_sample: &[u8], dir: &Path) -> Result<usize> {
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    paths.retain(|p| p.extension().is_some_and(|ext| ext == "yaml"));
    paths.sort();

    for path in &paths {
        let yaml = fs::read_to_string(path)?;
        let recipe: RecipeFile = serde_saphyr::from_str(&yaml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let stem = path.file_stem().unwrap().to_string_lossy();
        if stem.starts_with("webp-") {
            generate_webp(webp_sample, &path.with_extension("webp"), recipe.filters)?;
        } else {
            generate_png(png_sample, &path.with_extension("png"), recipe.filters)?;
        }
    }
    Ok(paths.len())
}

fn generate_one(
    sample_bytes: &[u8],
    out_dir: &Path,
    filename: &str,
    filter: impl GlitchFilter + 'static,
) -> Result<()> {
    let mut ctx = GlitchContext::new(sample_bytes, Some(SEED))?;
    ctx.add_filter(filter);
    ctx.execute();
    ctx.save(out_dir.join(filename))?;
    Ok(())
}

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample_path = root.join("crates/png-glitch/etc/sample00.png");
    let sample_bytes = fs::read(&sample_path)?;

    let out_dir = root.join("docs/gallery");
    fs::create_dir_all(&out_dir)?;

    generate_one(&sample_bytes, &out_dir, "invert.png", Invert)?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "brighten.png",
        Brighten { strength: 20000 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "color_distortion.png",
        ColorDistortion {
            magnitude: 0.1,
            strength: 50,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "color_space_glitch.png",
        ColorSpaceGlitch {
            magnitude: 0.2,
            hue_shift: 120.0,
            saturation_mult: 1.5,
            lightness_mult: 1.0,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "chromatic_aberration.png",
        ChromaticAberration {
            magnitude: 0.3,
            r_offset: 10,
            g_offset: 0,
            b_offset: 10,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "shift_channels.png",
        ShiftChannels {
            r: 10000,
            g: 20000,
            b: -10000,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "block_scramble.png",
        BlockScramble {
            magnitude: 0.1,
            block_size: 32,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "transpose.png",
        Transpose { magnitude: 0.1 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "horizontal_shift.png",
        HorizontalShift { magnitude: 0.1 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "random_copy.png",
        RandomCopy { times: 20 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "replace.png",
        Replace { magnitude: 0.05 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "set_zero.png",
        SetZero { magnitude: 0.05 },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "bitwise.png",
        Bitwise {
            magnitude: 0.1,
            op: BitOp::Xor,
            value: 128,
        },
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "sub.png",
        Regenerate::new(RegenerateStrategy::Fixed(PreProcess::SubFilter)),
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "up.png",
        Regenerate::new(RegenerateStrategy::Fixed(PreProcess::UpFilter)),
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "average.png",
        Regenerate::new(RegenerateStrategy::Fixed(PreProcess::AverageFilter)),
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "paeth.png",
        Regenerate::new(RegenerateStrategy::Fixed(PreProcess::PaethFilter)),
    )?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "change_filter_type.png",
        ChangeFilterType { magnitude: 0.1 },
    )?;

    let more_png = [
        ("darken.png", FilterConfig::Darken { strength: 40 }),
        (
            "pixel_sort.png",
            FilterConfig::PixelSort {
                magnitude: 0.3,
                criterion: Some(SortCriterion::Brightness),
            },
        ),
        (
            "channel_swap.png",
            FilterConfig::ChannelSwap {
                magnitude: 0.3,
                target: Some(SwapTarget::Rg),
            },
        ),
        ("remove_filter.png", FilterConfig::RemoveFilter),
        (
            "substitute.png",
            FilterConfig::Substitute {
                index: 100,
                value: 255,
            },
        ),
    ];
    for (filename, filter) in &more_png {
        generate_png(&sample_bytes, &out_dir.join(filename), [filter.clone()])?;
    }

    let webp_sample = fs::read(root.join("crates/webp-glitch/etc/sample00.webp"))?;
    let webp_dir = out_dir.join("webp");
    fs::create_dir_all(&webp_dir)?;
    let webp = [
        (
            "macroblock_glitch.webp",
            FilterConfig::MacroblockGlitch { magnitude: 0.15 },
        ),
        (
            "lossy_artifact.webp",
            FilterConfig::LossyArtifact { quality: 5.0 },
        ),
        (
            "pixel_sort.webp",
            FilterConfig::PixelSort {
                magnitude: 0.3,
                criterion: Some(SortCriterion::Brightness),
            },
        ),
    ];
    for (filename, filter) in &webp {
        generate_webp(&webp_sample, &webp_dir.join(filename), [filter.clone()])?;
    }

    let recipes = generate_recipes(&sample_bytes, &webp_sample, &out_dir.join("recipes"))?;

    println!(
        "Generated {} effect images, {} WebP images and {} recipe images in {}",
        18 + more_png.len(),
        webp.len(),
        recipes,
        out_dir.display()
    );
    Ok(())
}
