//! `--variations N`: several outputs from one input with different seeds, and an optional
//! contact sheet.

use crate::any_context::{AnyGlitchContext, ImageFormat, detect_format};
use crate::cli::Cli;
use crate::{
    OrderedStep, check_overwrite, effective_filters, recipe_io, resolve_output_path, resolve_seed,
    save_output, validate_flags_for_format,
};
use anyhow::{Context, bail};
use indicatif::{ProgressBar, ProgressStyle};
use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
use png_glitch_recipe::{ParseOptions, frame_seed};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// `<stem>-s<seed>.<ext>` next to `base`.
pub fn variation_path(base: &Path, seed: u64) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("glitched");
    let name = match base.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!("{stem}-s{seed}.{ext}"),
        None => format!("{stem}-s{seed}"),
    };
    base.with_file_name(name)
}

/// Nearest-neighbour scale of RGBA8 pixels to `new_w` × `new_h`.
fn scale_nearest(src: &[u8], w: u32, h: u32, new_w: u32, new_h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(new_w as usize * new_h as usize * 4);
    for y in 0..new_h as u64 {
        let sy = (y * h as u64 / new_h as u64) as usize;
        for x in 0..new_w as u64 {
            let sx = (x * w as u64 / new_w as u64) as usize;
            let i = (sy * w as usize + sx) * 4;
            out.extend_from_slice(&src[i..i + 4]);
        }
    }
    out
}

/// Height of a cell that keeps the aspect ratio of a `w` × `h` image at `cell_w` width.
fn cell_height(w: u32, h: u32, cell_w: u32) -> u32 {
    ((h as u64 * cell_w as u64 + w as u64 / 2) / w as u64).max(1) as u32
}

/// Lays `cells` (each `cell_w` × `cell_h` RGBA8; `None` stays transparent) out in a grid with
/// `ceil(sqrt(n))` columns. Returns `(width, height, pixels)`.
fn build_contact_sheet(
    cells: &[Option<Vec<u8>>],
    cell_w: u32,
    cell_h: u32,
) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let n = cells.len() as u32;
    let cols = (n as f64).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let width = cols
        .checked_mul(cell_w)
        .context("contact sheet is too wide")?;
    let height = rows
        .checked_mul(cell_h)
        .context("contact sheet is too tall")?;
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| n <= 1 << 30)
        .context("contact sheet is too large; lower --contact-cell-width or --variations")?;
    let mut sheet = vec![0u8; len];
    for (i, cell) in cells.iter().enumerate() {
        let Some(cell) = cell else { continue };
        let (cx, cy) = (i as u32 % cols, i as u32 / cols);
        for row in 0..cell_h as usize {
            let src = &cell[row * cell_w as usize * 4..(row + 1) * cell_w as usize * 4];
            let dst =
                ((cy * cell_h) as usize + row) * width as usize * 4 + (cx * cell_w) as usize * 4;
            sheet[dst..dst + src.len()].copy_from_slice(src);
        }
    }
    Ok((width, height, sheet))
}

fn encode_sheet(width: u32, height: u32, pixels: &[u8]) -> anyhow::Result<Vec<u8>> {
    let png = PngGlitch::from_pixels(
        PixelSource::rgba8(width, height, pixels),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
    )?;
    Ok(png.to_bytes()?)
}

pub fn variations_process(args: &Cli, steps: &[OrderedStep], count: u32) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    let input_path = Path::new(input);
    if input_path.is_dir() {
        bail!("'{input}' is a directory; --variations works on a single image");
    }
    let data = fs::read(input).with_context(|| format!("failed to open '{input}'"))?;
    let webp = detect_format(&data) == Some(ImageFormat::Webp)
        || input_path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("webp"));
    validate_flags_for_format(args, webp)?;
    if args.contact_sheet.is_some() && webp {
        bail!("--contact-sheet supports PNG input only");
    }

    let base = resolve_output_path(args, input);
    let base_seed = resolve_seed(args.seed);
    let seeds: Vec<u64> = (0..count as u64)
        .map(|i| frame_seed(base_seed, i))
        .collect();
    let paths: Vec<PathBuf> = seeds.iter().map(|&s| variation_path(&base, s)).collect();
    for path in &paths {
        check_overwrite(path, args.force)?;
    }
    if let Some(sheet) = &args.contact_sheet {
        check_overwrite(Path::new(sheet), args.force)?;
    }

    let filters = effective_filters(args, steps)?;
    // `--print-recipe` / `--save-recipe` describe the base seed; every output's own seed is
    // printed below and embedded with `--embed-recipe`.
    if args.print_recipe {
        eprint!("{}", recipe_io::render_recipe(base_seed, &filters)?);
    }
    if let Some(path) = &args.save_recipe {
        fs::write(path, recipe_io::render_recipe(base_seed, &filters)?)
            .with_context(|| format!("failed to write recipe to '{path}'"))?;
    }

    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );
    let succeeded = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let want_sheet = args.contact_sheet.is_some();
    let cell_w = args.contact_cell_width;
    // (source size, cell height, cells)
    let thumbs: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(vec![None; count as usize]);
    let dims: Mutex<Option<(u32, u32)>> = Mutex::new(None);

    (0..count as usize).into_par_iter().for_each(|i| {
        let seed = seeds[i];
        let result = (|| -> anyhow::Result<()> {
            let options = ParseOptions::default().with_strict(args.strict);
            let mut context = AnyGlitchContext::open_with_options(input, Some(seed), options)?;
            for f in &filters {
                context.add_from_config(f.clone())?;
            }
            context.execute();
            let recipe = recipe_io::render_recipe(seed, &filters)?;
            save_output(args, &context, &paths[i], &recipe)?;
            if want_sheet {
                match context.rgba_pixels() {
                    Ok((w, h, px)) => {
                        let ch = cell_height(w, h, cell_w);
                        thumbs.lock().unwrap()[i] = Some(scale_nearest(&px, w, h, cell_w, ch));
                        *dims.lock().unwrap() = Some((w, h));
                    }
                    Err(e) => pb.println(format!(
                        "warning: {}: left out of the contact sheet: {e:#}",
                        paths[i].display()
                    )),
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                succeeded.fetch_add(1, Ordering::Relaxed);
                pb.println(format!("{}: seed {seed}", paths[i].display()));
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                pb.println(format!("Error generating '{}': {e:#}", paths[i].display()));
            }
        }
        pb.inc(1);
    });

    let ok_count = succeeded.load(Ordering::Relaxed);
    let err_count = failed.load(Ordering::Relaxed);
    pb.finish_with_message(format!(
        "Variations complete: {ok_count} succeeded, {err_count} failed"
    ));

    if let Some(sheet_path) = &args.contact_sheet {
        let dims = *dims.lock().unwrap();
        let (w, h) = dims.context("no variation could be decoded for the contact sheet")?;
        let cells = thumbs.into_inner().unwrap();
        let (sw, sh, pixels) = build_contact_sheet(&cells, cell_w, cell_height(w, h, cell_w))?;
        fs::write(sheet_path, encode_sheet(sw, sh, &pixels)?)
            .with_context(|| format!("failed to save '{sheet_path}'"))?;
        eprintln!("{sheet_path}: contact sheet of {count} variation(s) of seed {base_seed}");
    }

    if err_count > 0 {
        bail!("variations finished with {err_count} failure(s) ({ok_count} succeeded)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variation_path_inserts_seed_before_extension() {
        assert_eq!(
            variation_path(Path::new("d/out.png"), 7),
            PathBuf::from("d/out-s7.png")
        );
    }

    #[test]
    fn contact_sheet_dimensions() {
        let cells = vec![Some(vec![255u8; 4 * 2 * 4]); 5];
        let (w, h, px) = build_contact_sheet(&cells, 4, 2).unwrap();
        assert_eq!((w, h), (12, 4)); // 3 columns x 2 rows
        assert_eq!(px.len(), 12 * 4 * 4);
    }

    #[test]
    fn nearest_scale_keeps_corner_pixels() {
        let src = [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4];
        let out = scale_nearest(&src, 2, 2, 4, 4);
        assert_eq!(&out[..4], &[1, 1, 1, 1]);
        assert_eq!(&out[out.len() - 4..], &[4, 4, 4, 4]);
    }
}
