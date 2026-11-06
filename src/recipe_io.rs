//! Writing, embedding and reading back the effective recipe of a run.

use anyhow::{Context, Result, bail};
use png_glitch_recipe::FilterConfig;
use serde::Serialize;

/// Keyword of the PNG `iTXt` chunk that holds the recipe.
const KEYWORD: &str = "png-glitch-recipe";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Serialize)]
struct RecipeFile<'a> {
    filters: &'a [FilterConfig],
}

/// Renders the effective recipe as YAML that `--config` accepts. The seed is a leading comment,
/// so the text can be saved and fed back with `--config FILE --seed SEED`.
pub fn render_recipe(seed: u64, filters: &[FilterConfig]) -> Result<String> {
    let yaml = serde_saphyr::to_string(&RecipeFile { filters })
        .context("failed to serialize the recipe")?;
    Ok(format!("# seed: {seed}\n{yaml}"))
}

fn crc(chunk_type: &[u8], data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    hasher.finalize()
}

/// A PNG chunk: offset of its length field, type and data.
type Chunk<'a> = (usize, [u8; 4], &'a [u8]);

/// Splits a PNG into `(offset, type, data)` chunk records, or `None` if it is malformed.
fn chunks(png: &[u8]) -> Option<Vec<Chunk<'_>>> {
    if !png.starts_with(PNG_SIGNATURE) {
        return None;
    }
    let mut out = Vec::new();
    let mut pos = PNG_SIGNATURE.len();
    while pos + 12 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().ok()?) as usize;
        let end = pos.checked_add(12)?.checked_add(len)?;
        if end > png.len() {
            return None;
        }
        let kind: [u8; 4] = png[pos + 4..pos + 8].try_into().ok()?;
        out.push((pos, kind, &png[pos + 8..pos + 8 + len]));
        pos = end;
    }
    Some(out)
}

/// Returns the text of an `iTXt` chunk with our keyword (uncompressed only).
fn recipe_text(data: &[u8]) -> Option<&str> {
    let rest = data.strip_prefix(KEYWORD.as_bytes())?.strip_prefix(&[0])?;
    // compression flag, compression method (both must be 0), language tag, translated keyword
    let rest = rest.strip_prefix(&[0, 0])?;
    let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..];
    let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..];
    std::str::from_utf8(rest).ok()
}

/// Stores `text` in an `iTXt` chunk before `IEND`, replacing any earlier recipe chunk.
pub fn embed_in_png(png: &[u8], text: &str) -> Result<Vec<u8>> {
    let Some(list) = chunks(png) else {
        bail!("cannot embed the recipe: the output is not a well-formed PNG");
    };
    let mut data = Vec::with_capacity(KEYWORD.len() + 5 + text.len());
    data.extend_from_slice(KEYWORD.as_bytes());
    data.extend_from_slice(&[0, 0, 0, 0, 0]); // keyword end, flag, method, empty language, empty translated keyword
    data.extend_from_slice(text.as_bytes());

    let mut out = Vec::with_capacity(png.len() + data.len() + 12);
    out.extend_from_slice(PNG_SIGNATURE);
    let mut inserted = false;
    for (pos, kind, body) in list {
        let is_ours = &kind == b"iTXt" && recipe_text(body).is_some();
        if is_ours {
            continue;
        }
        if &kind == b"IEND" && !inserted {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(b"iTXt");
            out.extend_from_slice(&data);
            out.extend_from_slice(&crc(b"iTXt", &data).to_be_bytes());
            inserted = true;
        }
        out.extend_from_slice(&png[pos..pos + 12 + body.len()]);
    }
    if !inserted {
        bail!("cannot embed the recipe: the output PNG has no IEND chunk");
    }
    Ok(out)
}

/// Reads the recipe that [`embed_in_png`] stored, if any.
pub fn extract_from_png(png: &[u8]) -> Option<String> {
    chunks(png)?
        .into_iter()
        .filter(|(_, kind, _)| kind == b"iTXt")
        .find_map(|(_, _, body)| recipe_text(body).map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        let ctx = png_glitch_recipe::GlitchContext::new(
            include_bytes!("../crates/png-glitch-recipe/tests/fixtures/small.png"),
            Some(1),
        )
        .unwrap();
        ctx.buffer().unwrap()
    }

    #[test]
    fn embed_and_extract_round_trip_and_replace() {
        let png = tiny_png();
        let once = embed_in_png(&png, "# seed: 1\nfilters: []\n").unwrap();
        assert_eq!(
            extract_from_png(&once).as_deref(),
            Some("# seed: 1\nfilters: []\n")
        );
        let twice = embed_in_png(&once, "# seed: 2\nfilters: []\n").unwrap();
        assert_eq!(
            extract_from_png(&twice).as_deref(),
            Some("# seed: 2\nfilters: []\n")
        );
        assert_eq!(twice.len() - png.len(), once.len() - png.len());
        assert_eq!(extract_from_png(&png), None);
        // The image still decodes and its pixels are untouched.
        let a = png_glitch_recipe::PngGlitch::from_bytes(&png).unwrap();
        let b = png_glitch_recipe::PngGlitch::from_bytes(&twice).unwrap();
        assert_eq!(a.data(), b.data());
    }

    #[test]
    fn rejects_non_png_input() {
        assert!(embed_in_png(b"not a png", "x").is_err());
        assert_eq!(extract_from_png(b"not a png"), None);
    }
}
