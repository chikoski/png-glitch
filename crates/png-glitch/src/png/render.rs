//! `Renderer`: shows a model's pixels exactly as a PNG decoder would (#96).
//!
//! The live pipeline does not need to compress a glitched frame and decompress it again to see
//! it. [`Renderer::render_into`] runs the decoder's reconstruction step (unfiltering with the
//! shared kernels of #97) and its colour conversion straight into a caller-owned RGBA8 buffer.

use super::scan_line::bytes_per_pixel;
use super::scan_line::filter::{self, kernels};
use super::{ColorType, FilterType, PngError};
use crate::PngGlitch;

/// Renders a [`PngGlitch`] model into RGBA8 pixels, byte for byte as a PNG decoder would show
/// `model.to_bytes()`.
///
/// Make one `Renderer` per image size and format with [`Renderer::new`] (it allocates the row
/// buffers some formats need), then call [`render_into`](Renderer::render_into) once per frame:
/// that call makes **no heap allocation** and does not modify the model, so the scan lines stay
/// filtered and can be glitched further.
///
/// # Decoder equivalence
///
/// **Guarantee.** For any model `m` (any colour type, bit depth and width, any glitch applied
/// to the scan lines), `render_into(m)` is byte-identical to decoding `m.to_bytes()` with the
/// reference decoder, configured to give 8-bit RGBA output.
///
/// **Reference decoder.** The [`png`](https://crates.io/crates/png) crate, 0.18.x, with the
/// transformations `EXPAND | STRIP_16`; gray and RGB output is widened to RGBA with alpha 255.
/// The property test `tests/render.rs` checks the guarantee over the colour type × bit depth ×
/// width matrix (it runs natively and on `wasm32-wasip1` in CI). Comparing with Chrome's
/// decoding of a fixed corpus is tracked in #146.
///
/// **Output.** RGBA8, non-premultiplied (what canvas `ImageData` expects); the buffer must be
/// exactly `width * height * 4` bytes.
///
/// **Conversions** (the same as the reference decoder):
///
/// - Gray 1/2/4-bit: the sample is multiplied by `255 / (2^depth - 1)` (255, 85, 17).
/// - 16-bit samples keep their high byte.
/// - Palette: the colour comes from `PLTE`, the alpha from `tRNS` (255 for entries past its
///   end). A `tRNS` with more entries than the palette is ignored as a whole. An index past the
///   end of the palette gives opaque black (`0, 0, 0, 255`).
/// - `tRNS` on gray/RGB: a pixel equal to the key colour, compared at the full bit depth before
///   stripping, gets alpha 0.
/// - A filter-type byte above 4 is an error ([`PngError::InvalidFilterType`]), as decoders
///   reject it. Parsing already rewrites such bytes to 0, but
///   a model changed from inside the crate can still hold one.
/// - Interlaced images are rejected at parse time, so they do not occur here.
///
/// **Intentional deviations from the reference decoder:** none.
///
/// **Not applied:** gamma, `iCCP`, `sRGB` and `cHRM` are ignored, like the `png` crate's
/// default. A browser applies colour management, so it can show different colours for an image
/// that carries these chunks.
///
/// # Examples
/// ```
/// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch, Renderer};
///
/// let frame = vec![200u8; 8 * 8 * 4];
/// let png = PngGlitch::from_pixels(
///     PixelSource::rgba8(8, 8, &frame),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
/// )?;
/// let mut renderer = Renderer::new(&png);
/// let mut out = vec![0u8; 8 * 8 * 4];
/// renderer.render_into(&png, &mut out)?;
/// assert_eq!(out, frame);
/// # Ok::<(), png_glitch::PngError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Renderer {
    width: u32,
    height: u32,
    color_type: ColorType,
    bit_depth: u8,
    /// The previous and the current reconstructed row (without the filter-type byte).
    prev: Vec<u8>,
    cur: Vec<u8>,
}

impl Renderer {
    /// Makes a renderer for images with the header of `png` (size, colour type, bit depth).
    /// This allocates the row buffers.
    pub fn new(png: &PngGlitch) -> Renderer {
        let header = &png.png.header;
        let row = header.scan_line_width().saturating_sub(1);
        Renderer {
            width: header.width(),
            height: header.height(),
            color_type: header.color_type(),
            bit_depth: header.bit_depth(),
            prev: vec![0; row],
            cur: vec![0; row],
        }
    }

    /// Reconstructs `png` and writes it as RGBA8 into `out`. It makes no heap allocation and
    /// leaves the model unchanged.
    ///
    /// # Errors
    /// - [`PngError::PixelDataLength`] if `out.len() != width * height * 4`.
    /// - [`PngError::HeaderMismatch`] if `png` has another size, colour type or bit depth than
    ///   the image this renderer was made for.
    /// - [`PngError::InvalidFilterType`] if a scan line has a filter-type byte above 4.
    pub fn render_into(&mut self, png: &PngGlitch, out: &mut [u8]) -> Result<(), PngError> {
        let model = &png.png;
        let header = &model.header;
        if (
            header.width(),
            header.height(),
            header.color_type(),
            header.bit_depth(),
        ) != (self.width, self.height, self.color_type, self.bit_depth)
        {
            return Err(PngError::HeaderMismatch);
        }
        let width = self.width as usize;
        let expected = width
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(PngError::ImageTooLarge {
                width: self.width,
                height: self.height,
            })?;
        if out.len() != expected {
            return Err(PngError::PixelDataLength {
                expected,
                actual: out.len(),
            });
        }

        let stride = header.scan_line_width();
        let bpp = bytes_per_pixel(self.color_type, self.bit_depth);
        let depth = self.bit_depth;
        let lookup = Lookup::new(model, self.color_type, depth);
        let out_row = width * 4;
        let rows = model.data.chunks_exact(stride);

        if self.color_type == ColorType::TrueColorAlpha && depth == 8 {
            // Reconstruct straight into the output: the previous output row is the previous
            // reconstructed row. Runs of rows with the same filter type go through the multi-row
            // wavefront kernels where they exist (#220, `filter::remove_rows_untagged`); the
            // bytes are the same as row by row.
            let height = self.height as usize;
            let mut y = 0;
            while y < height {
                // The stored rows from `y` on; a model shorter than its header stops here.
                let stored = model.data.get(y * stride..).unwrap_or_default();
                let Some(row) = stored.get(..stride) else {
                    break;
                };
                let (ft, bytes) = split_row(row)?;
                // The rows from `y` on that share the filter type (at most 4 are used).
                let run = stored
                    .chunks_exact(stride)
                    .take(4)
                    .take_while(|r| r.first() == row.first())
                    .count();
                let (done, rest) = out.split_at_mut(y * out_row);
                let prev = done
                    .len()
                    .checked_sub(out_row)
                    .and_then(|start| done.get(start..));
                // Only the rows the wavefront takes are copied first; the others go one at a time.
                let rows = filter::untagged_run(ft, run, bpp);
                if let Some(prev) = prev
                    && rows > 0
                {
                    let block = rest.get_mut(..rows * out_row).unwrap_or_default();
                    for (dst, src) in block
                        .chunks_exact_mut(out_row)
                        .zip(stored.chunks_exact(stride))
                    {
                        if let Some(src) = src.get(1..) {
                            dst.copy_from_slice(src);
                        }
                    }
                    let n = filter::remove_rows_untagged(ft, rows, block, out_row, bpp, prev);
                    if n > 0 {
                        y += n;
                        continue;
                    }
                }
                let Some(cur) = rest.get_mut(..out_row) else {
                    break;
                };
                cur.copy_from_slice(bytes);
                kernels::unfilter(ft, bpp, cur, prev);
                y += 1;
            }
            return Ok(());
        }

        for (y, (row, out_line)) in rows.zip(out.chunks_exact_mut(out_row)).enumerate() {
            let (ft, bytes) = split_row(row)?;
            self.cur.copy_from_slice(bytes);
            let prev = (y > 0).then_some(self.prev.as_slice());
            kernels::unfilter(ft, bpp, &mut self.cur, prev);
            convert_row(&self.cur, out_line, width, self.color_type, depth, &lookup);
            std::mem::swap(&mut self.prev, &mut self.cur);
        }
        Ok(())
    }
}

/// Splits a stored scan line into its filter type and filtered bytes.
fn split_row(row: &[u8]) -> Result<(FilterType, &[u8]), PngError> {
    let (tag, bytes) = row.split_first().ok_or(PngError::InvalidFilterType)?;
    Ok((FilterType::try_from(*tag)?, bytes))
}

/// The palette and transparency key of an image, in the form the conversion needs.
struct Lookup {
    palette: [[u8; 4]; 256],
    /// The `tRNS` key colour of a gray/RGB image: one 16-bit sample per channel, big endian.
    /// `None` if there is no usable `tRNS` (the reference decoder compares the whole chunk, so a
    /// chunk of another length never matches).
    key: Option<[u8; 6]>,
}

impl Lookup {
    fn new(model: &super::Png, color_type: ColorType, depth: u8) -> Lookup {
        let find = |name: &[u8; 4]| {
            model
                .pre_idat_chunks
                .iter()
                .find(|chunk| chunk.chunk_type.as_bytes() == name)
                .map(|chunk| chunk.data.as_slice())
        };
        let mut palette = [[0, 0, 0, 255]; 256];
        let mut key = None;
        let trns = find(b"tRNS");
        match color_type {
            ColorType::IndexColor => {
                let plte = find(b"PLTE").unwrap_or(&[]);
                let entries = plte.len() / 3;
                for (slot, rgb) in palette.iter_mut().zip(plte.as_chunks::<3>().0.iter()) {
                    for (dst, src) in slot.iter_mut().zip(rgb) {
                        *dst = *src;
                    }
                }
                // Longer than the palette: ignored as a whole.
                if let Some(trns) = trns.filter(|t| t.len() <= entries) {
                    for (slot, alpha) in palette.iter_mut().zip(trns) {
                        if let Some(a) = slot.get_mut(3) {
                            *a = *alpha;
                        }
                    }
                }
            }
            ColorType::GrayScale | ColorType::TrueColor => {
                let channels = if color_type == ColorType::GrayScale {
                    1
                } else {
                    3
                };
                if let Some(trns) = trns {
                    let mut k = [0u8; 6];
                    if depth == 16 {
                        if trns.len() == channels * 2 {
                            for (dst, src) in k.iter_mut().zip(trns) {
                                *dst = *src;
                            }
                            key = Some(k);
                        }
                    } else if trns.len() >= channels * 2 {
                        // Below 16 bits the decoder keeps only the low byte of each sample.
                        for (dst, pair) in k.iter_mut().zip(trns.as_chunks::<2>().0.iter()) {
                            *dst = pair.get(1).copied().unwrap_or(0);
                        }
                        key = Some(k);
                    }
                }
            }
            ColorType::GrayScaleAlpha | ColorType::TrueColorAlpha => {}
        }
        Lookup { palette, key }
    }
}

/// Converts one reconstructed row to RGBA8.
fn convert_row(
    row: &[u8],
    out: &mut [u8],
    width: usize,
    color_type: ColorType,
    depth: u8,
    lookup: &Lookup,
) {
    let key = lookup.key.as_ref();
    let key_alpha = |matches: bool| if matches { 0 } else { 255 };
    let pixels = out.as_chunks_mut::<4>().0.iter_mut().take(width);
    match (color_type, depth) {
        (ColorType::GrayScale, 8) => {
            let key = key.and_then(|k| k.first().copied());
            for (px, out) in row.iter().zip(pixels) {
                out.copy_from_slice(&[*px, *px, *px, key_alpha(Some(*px) == key)]);
            }
        }
        (ColorType::GrayScale, 16) => {
            for (px, out) in row.as_chunks::<2>().0.iter().zip(pixels) {
                let g = px.first().copied().unwrap_or(0);
                let is_key = key.is_some_and(|k| k.get(..2) == Some(px.as_slice()));
                out.copy_from_slice(&[g, g, g, key_alpha(is_key)]);
            }
        }
        (ColorType::GrayScale, _) => {
            let scale = 255 / ((1u16 << depth) - 1) as u8;
            let key = key.and_then(|k| k.first().copied());
            unpack(row, pixels, depth, |v, out| {
                let g = v * scale;
                out.copy_from_slice(&[g, g, g, key_alpha(Some(v) == key)]);
            });
        }
        (ColorType::IndexColor, _) => {
            unpack(row, pixels, depth, |v, out| {
                let rgba = lookup.palette.get(v as usize).copied();
                out.copy_from_slice(&rgba.unwrap_or([0, 0, 0, 255]));
            });
        }
        (ColorType::GrayScaleAlpha, 8) => {
            for (px, out) in row.as_chunks::<2>().0.iter().zip(pixels) {
                let [g, a] = *px;
                out.copy_from_slice(&[g, g, g, a]);
            }
        }
        (ColorType::GrayScaleAlpha, _) => {
            for (px, out) in row.as_chunks::<4>().0.iter().zip(pixels) {
                let [g, _, a, _] = *px;
                out.copy_from_slice(&[g, g, g, a]);
            }
        }
        (ColorType::TrueColor, 8) => {
            for (px, out) in row.as_chunks::<3>().0.iter().zip(pixels) {
                let [r, g, b] = *px;
                let is_key = key.is_some_and(|k| k.get(..3) == Some(px.as_slice()));
                out.copy_from_slice(&[r, g, b, key_alpha(is_key)]);
            }
        }
        (ColorType::TrueColor, _) => {
            for (px, out) in row.as_chunks::<6>().0.iter().zip(pixels) {
                let [r, _, g, _, b, _] = *px;
                let is_key = key.is_some_and(|k| k == px);
                out.copy_from_slice(&[r, g, b, key_alpha(is_key)]);
            }
        }
        (ColorType::TrueColorAlpha, 8) => {
            for (px, out) in row.as_chunks::<4>().0.iter().zip(pixels) {
                out.copy_from_slice(px);
            }
        }
        (ColorType::TrueColorAlpha, _) => {
            for (px, out) in row.as_chunks::<8>().0.iter().zip(pixels) {
                let [r, _, g, _, b, _, a, _] = *px;
                out.copy_from_slice(&[r, g, b, a]);
            }
        }
    }
}

/// Calls `f` with each sample of a row packed at `depth` (1, 2, 4 or 8) bits, most significant
/// bits first, once per output pixel.
fn unpack<'a>(
    row: &[u8],
    pixels: impl Iterator<Item = &'a mut [u8; 4]>,
    depth: u8,
    f: impl Fn(u8, &mut [u8; 4]),
) {
    if depth >= 8 {
        for (v, out) in row.iter().zip(pixels) {
            f(*v, out);
        }
        return;
    }
    let mask = (1u16 << depth) as u8 - 1;
    let per_byte = (8 / depth) as usize;
    for (x, out) in pixels.enumerate() {
        let Some(byte) = row.get(x / per_byte) else {
            break;
        };
        let shift = 8 - depth as usize * (x % per_byte + 1);
        f((byte >> shift) & mask, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Encoding, FilterStrategy, PixelSource, PngGlitch};

    #[test]
    fn a_filter_type_byte_above_four_is_an_error() {
        let frame = vec![9u8; 4 * 2 * 4];
        let mut model = PngGlitch::from_pixels(
            PixelSource::rgba8(4, 2, &frame),
            Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
        )
        .expect("model");
        let mut renderer = Renderer::new(&model);
        let mut out = vec![0; frame.len()];
        renderer.render_into(&model, &mut out).expect("valid");

        let stride = model.png.header.scan_line_width();
        for bad in [5u8, 9, 255] {
            // The second row: the first one is reconstructed before the error is found.
            model.png.data[stride] = bad;
            let err = renderer.render_into(&model, &mut out).unwrap_err();
            assert!(matches!(err, PngError::InvalidFilterType), "{bad}: {err:?}");
        }
        // The same for a format that goes through the scratch rows.
        let mut model = PngGlitch::from_pixels(
            PixelSource::rgba8(4, 2, &frame),
            Encoding {
                color_type: ColorType::GrayScale,
                bit_depth: 8,
                filter: FilterStrategy::Fixed(FilterType::None),
            },
        )
        .expect("model");
        let mut renderer = Renderer::new(&model);
        model.png.data[0] = 7;
        let err = renderer.render_into(&model, &mut out).unwrap_err();
        assert!(matches!(err, PngError::InvalidFilterType), "{err:?}");
    }
}
