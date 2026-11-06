use anyhow::Result;
use png_glitch_recipe::{
    AverageFilter, BitOp, Bitwise, BlockScramble, Brighten, ChangeFilterType, ChromaticAberration,
    ColorDistortion, ColorSpaceGlitch, GlitchContext, GlitchFilter, HorizontalShift, Invert,
    PaethFilter, RandomCopy, Replace, SetZero, ShiftChannels, SubFilter, Transpose, UpFilter,
};
use std::fs;
use std::path::Path;

const SEED: u64 = 42;

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
    generate_one(&sample_bytes, &out_dir, "sub.png", SubFilter)?;
    generate_one(&sample_bytes, &out_dir, "up.png", UpFilter)?;
    generate_one(&sample_bytes, &out_dir, "average.png", AverageFilter)?;
    generate_one(&sample_bytes, &out_dir, "paeth.png", PaethFilter)?;
    generate_one(
        &sample_bytes,
        &out_dir,
        "change_filter_type.png",
        ChangeFilterType { magnitude: 0.1 },
    )?;

    println!("Generated 18 gallery images in {}", out_dir.display());
    Ok(())
}
