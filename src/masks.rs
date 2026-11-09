//! Loading `mask=FILE` images for `rect` / `mask` targeting (ADR 0006).
//!
//! The recipe crate reads no files; the CLI does. A mask is a PNG, turned into one value per
//! pixel by integer Rec. 601 luma times alpha / 255. A step refers to it by `path` and the
//! CRC-32 of the file's bytes, which is checked here and filled in when the recipe omits it.

use anyhow::{Context, bail};
use png_glitch::effects::Mask;
use png_glitch_recipe::{FilterEntry, MaskSet, PngGlitch};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Mutex;

/// Paths already warned about, so a watch loop or a second pass does not repeat the warning.
static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn warn_once(path: &str) {
    let mut guard = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if guard
        .get_or_insert_with(HashSet::new)
        .insert(path.to_string())
    {
        eprintln!(
            "warning: the recipe does not give a crc32 for the mask '{path}'; \
             the printed recipe records the checksum of the file as it is now"
        );
    }
}

/// Decodes PNG `bytes` into a [`Mask`]: Rec. 601 luma (`(77r + 150g + 29b + 128) >> 8`) times
/// alpha / 255, one value per pixel.
pub fn mask_from_png(bytes: &[u8]) -> anyhow::Result<Mask> {
    let png = PngGlitch::from_bytes(bytes).context("the mask is not a valid PNG image")?;
    let (w, h) = (png.width(), png.height());
    let len = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
        .context("the mask image is too large")?;
    let mut rgba = vec![0u8; len];
    png_glitch::Renderer::new(&png).render_into(&png, &mut rgba)?;
    let values: Vec<u8> = rgba
        .chunks_exact(4)
        .map(|px| match px {
            [r, g, b, a] => {
                let luma =
                    (77 * u32::from(*r) + 150 * u32::from(*g) + 29 * u32::from(*b) + 128) >> 8;
                ((luma.min(255) * u32::from(*a) + 127) / 255) as u8
            }
            _ => 0,
        })
        .collect();
    Mask::from_values(w, h, values).context("the mask image has an unexpected size")
}

/// Loads every `mask=FILE` the entries refer to.
///
/// A `crc32` the recipe gives must match the file (otherwise an error); a missing one is filled
/// in (with a warning), so the recipe that is printed, saved or embedded pins the file.
pub fn load_masks(filters: &mut [FilterEntry]) -> anyhow::Result<MaskSet> {
    let mut set = MaskSet::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    for entry in filters {
        let Some(reference) = entry.mask.as_mut() else {
            continue;
        };
        let crc = match seen.get(&reference.path) {
            Some(crc) => *crc,
            None => {
                let bytes = fs::read(&reference.path)
                    .with_context(|| format!("failed to open mask '{}'", reference.path))?;
                let crc = crc32fast::hash(&bytes);
                let mask = mask_from_png(&bytes)
                    .with_context(|| format!("failed to load mask '{}'", reference.path))?;
                set.insert(reference.path.clone(), mask, Some(crc));
                seen.insert(reference.path.clone(), crc);
                crc
            }
        };
        match reference.crc32 {
            Some(expected) if expected != crc => bail!(
                "the mask '{}' has CRC-32 {crc:08x}, but the recipe expects {expected:08x}; \
                 it is not the file the recipe was written for",
                reference.path
            ),
            Some(_) => {}
            None => {
                warn_once(&reference.path);
                reference.crc32 = Some(crc);
            }
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_png(values: &[u8], w: u32, h: u32) -> Vec<u8> {
        let rgba: Vec<u8> = values.iter().flat_map(|&v| [v, v, v, 255]).collect();
        png_glitch::PngGlitch::from_pixels(
            png_glitch::PixelSource::rgba8(w, h, &rgba),
            png_glitch::Encoding::rgba8(png_glitch::FilterStrategy::Fixed(
                png_glitch::FilterType::None,
            )),
        )
        .unwrap()
        .to_bytes()
        .unwrap()
    }

    #[test]
    fn luma_of_gray_is_the_gray_value() {
        let mask = mask_from_png(&gray_png(&[0, 64, 128, 255], 2, 2)).unwrap();
        assert_eq!((mask.width(), mask.height()), (2, 2));
        let got: Vec<u8> = (0..4).map(|i| mask.value(i % 2, i / 2)).collect();
        assert_eq!(got, vec![0, 64, 128, 255]);
    }

    #[test]
    fn a_non_png_is_an_error() {
        assert!(mask_from_png(b"not a png").is_err());
    }
}
