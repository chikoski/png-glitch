//! `Mask` and `Masked<E>`: apply a scan-line effect through a per-pixel strength mask.
//!
//! See ADR 0006. A mask value is a strength: 0 keeps the original stored bytes, 255 keeps the
//! effect's result, and values in between blend the two. The wrapper works on the *stored*
//! (possibly filtered) bytes and never removes or re-applies filters.

use super::ScanLinesEffect;
use crate::{ColorType, PngGlitch, ScanLines};
use std::sync::{Mutex, PoisonError};

/// A mask value per pixel (`0..=255`), as data. No file is read here.
///
/// A rectangle is stored without a per-pixel array. Outside the mask's own width and height
/// every value is `0`, so a mask that is smaller than the image selects nothing beyond itself
/// and a larger one is simply cropped.
///
/// # Examples
/// ```
/// use png_glitch::effects::Mask;
///
/// let rect = Mask::rect(8, 4, 2, 1, 3, 2);
/// assert_eq!(rect.value(2, 1), 255);
/// assert_eq!(rect.value(5, 1), 0); // outside the rectangle
/// assert_eq!(rect.clone().inverted().value(5, 1), 255);
/// assert_eq!(rect.value(100, 100), 0); // outside the mask
///
/// assert!(Mask::from_values(2, 2, vec![0; 3]).is_none()); // wrong length
/// let m = Mask::from_values(2, 1, vec![10, 200]).unwrap();
/// assert_eq!((m.value(0, 0), m.value(1, 0)), (10, 200));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    /// 255 inside `x..x+w, y..y+h` (clipped to the mask), 0 outside; `invert` swaps them.
    Rect {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        invert: bool,
    },
    Values(Vec<u8>),
}

impl Mask {
    /// A mask from one value per pixel, row by row. `None` if `values.len() != width * height`.
    pub fn from_values(width: u32, height: u32, values: Vec<u8>) -> Option<Mask> {
        let len = (width as usize).checked_mul(height as usize)?;
        (values.len() == len).then_some(Mask {
            width,
            height,
            kind: Kind::Values(values),
        })
    }

    /// A `width` × `height` mask that is 255 inside the rectangle at `(x, y)` of size `w` × `h`
    /// and 0 outside. The rectangle is clipped to the mask.
    pub fn rect(width: u32, height: u32, x: u32, y: u32, w: u32, h: u32) -> Mask {
        Mask {
            width,
            height,
            kind: Kind::Rect {
                x: x.min(width),
                y: y.min(height),
                w: w.min(width.saturating_sub(x)),
                h: h.min(height.saturating_sub(y)),
                invert: false,
            },
        }
    }

    /// The mask with every value `v` replaced by `255 - v` (still `0` outside the mask's bounds).
    pub fn inverted(mut self) -> Mask {
        match &mut self.kind {
            Kind::Rect { invert, .. } => *invert = !*invert,
            Kind::Values(values) => values.iter_mut().for_each(|v| *v = 255 - *v),
        }
        self
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The value at `(x, y)`; `0` outside the mask's bounds.
    pub fn value(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        match &self.kind {
            Kind::Rect {
                x: rx,
                y: ry,
                w,
                h,
                invert,
            } => {
                let inside = x >= *rx && x - *rx < *w && y >= *ry && y - *ry < *h;
                if inside != *invert { 255 } else { 0 }
            }
            Kind::Values(values) => {
                let i = y as usize * self.width as usize + x as usize;
                values.get(i).copied().unwrap_or(0)
            }
        }
    }
}

/// Applies an effect through a [`Mask`].
///
/// For every call the wrapper copies the stored bytes of **every** row (an effect can move
/// bytes between rows, as `SwapLines` does), runs the effect on the whole view, and then writes
/// back per pixel: the original bytes where the mask is `0`, the result where it is `255`, and
/// a blend in between.
///
/// - **8-bit samples:** `out = (orig × (255 − m) + result × m + 127) / 255`.
/// - **16-bit samples:** the same on big-endian `u16` samples with `m` widened to `m × 257` and
///   the divisor 65535.
/// - **Indexed colour and packed 1/2/4-bit samples** cannot be blended, so the mask is
///   thresholded: `m >= 128` keeps the result.
/// - **The filter-type tag** of a row is the result's if any mask value in that row is `>= 128`,
///   and the original's otherwise. The padding bits after the last pixel of a packed row follow
///   the tag.
///
/// Row `y` of the view the effect receives is row `y` of the mask. The buffer is allocated once
/// ([`new`](Self::new) / [`for_image`](Self::for_image)) and `apply_to_scan_lines` makes no
/// further heap allocation unless the image is larger than the buffer, so the wrapper is usable
/// in the live pipeline (the inner effect must be allocation-free too).
///
/// # Stored bytes, not pixels
/// The mask selects *stored* bytes, filtered or not. A change inside the mask can show up in
/// decoded rows below it, because a decoder rebuilds each row from the one above. For an exact
/// pixel region call [`PngGlitch::remove_filter`] first (a normal recipe step).
///
/// # What cannot be wrapped
/// [`Refilter`](super::Refilter) recomputes filtered values and is an [`Effect`](super::Effect),
/// not a [`ScanLinesEffect`], so it cannot be masked. Recomputing values is a new generation
/// (ADR 0005), not a glitch step.
///
/// # Examples
/// ```
/// use png_glitch::effects::{Invert, Mask, Masked};
/// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
///
/// let frame = vec![10u8; 4 * 4 * 4];
/// let mut png = PngGlitch::from_pixels(
///     PixelSource::rgba8(4, 4, &frame),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
/// )?;
/// let before = png.data().to_vec();
/// let mask = Mask::rect(4, 4, 0, 0, 2, 4); // the left half
/// png.apply(Masked::for_image(Invert::new(), mask, &png));
/// let stride = before.len() / 4;
/// // Left pixel inverted, right pixel untouched:
/// assert_eq!(png.data()[1], 255 - before[1]);
/// assert_eq!(png.data()[stride - 1], before[stride - 1]);
/// # Ok::<(), png_glitch::PngError>(())
/// ```
#[derive(Debug)]
pub struct Masked<E> {
    effect: E,
    mask: Mask,
    snapshot: Mutex<Vec<u8>>,
}

impl<E: ScanLinesEffect> Masked<E> {
    /// Wraps `effect` for images with `height` rows of `stride` bytes (the filter-type tag plus
    /// the filtered bytes). Allocates the snapshot buffer once.
    pub fn new(effect: E, mask: Mask, stride: usize, height: u32) -> Masked<E> {
        Masked {
            effect,
            mask,
            snapshot: Mutex::new(vec![0; stride.saturating_mul(height as usize)]),
        }
    }

    /// Like [`new`](Self::new), sized for `png`.
    pub fn for_image(effect: E, mask: Mask, png: &PngGlitch) -> Masked<E> {
        Masked {
            effect,
            mask,
            snapshot: Mutex::new(vec![0; png.data().len()]),
        }
    }

    /// The mask.
    pub fn mask(&self) -> &Mask {
        &self.mask
    }
}

impl<E: ScanLinesEffect> ScanLinesEffect for Masked<E> {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
        let len = lines.bytes().len();
        if snapshot.len() < len {
            snapshot.resize(len, 0); // the only allocation, once
        }
        let Some(orig) = snapshot.get_mut(..len) else {
            return;
        };
        orig.copy_from_slice(lines.bytes());

        self.effect.apply_to_scan_lines(lines);

        let (stride, width) = (lines.stride(), lines.width());
        let (color_type, bit_depth) = (lines.color_type(), lines.bit_depth());
        if stride == 0 {
            return;
        }
        let rows = lines
            .bytes_mut()
            .chunks_exact_mut(stride)
            .zip(orig.chunks_exact(stride));
        for (y, (row, orig)) in rows.enumerate() {
            blend_row(
                &self.mask, y as u32, width, color_type, bit_depth, row, orig,
            );
        }
    }
}

/// `(o × (255 − m) + r × m + 127) / 255`
fn blend8(o: u8, r: u8, m: u8) -> u8 {
    let (o, r, m) = (u32::from(o), u32::from(r), u32::from(m));
    ((o * (255 - m) + r * m + 127) / 255) as u8
}

/// The 16-bit version, with `m` widened to `m × 257`.
fn blend16(o: u16, r: u16, m: u8) -> u16 {
    let (o, r, m) = (u32::from(o), u32::from(r), u32::from(m) * 257);
    ((o * (65535 - m) + r * m + 32767) / 65535) as u16
}

fn blend_row(
    mask: &Mask,
    y: u32,
    width: u32,
    color_type: ColorType,
    bit_depth: u8,
    row: &mut [u8],
    orig: &[u8],
) {
    let (Some((tag, data)), Some((orig_tag, orig_data))) =
        (row.split_first_mut(), orig.split_first())
    else {
        return;
    };
    let bits = color_type.bit_per_pixel(bit_depth);
    if bits == 0 {
        return;
    }
    let packed = color_type == ColorType::IndexColor || bit_depth < 8;
    let mut any_on = false;

    if packed {
        // Thresholded: a pixel's bits come from the result only where `m >= 128`.
        for x in 0..width {
            let on = mask.value(x, y) >= 128;
            any_on |= on;
            if on {
                continue;
            }
            let bit = x as usize * bits;
            let (i, shift) = (bit / 8, 8 - bits.min(8) - bit % 8);
            let bits_mask = (((1u16 << bits.min(8)) - 1) as u8) << shift;
            if let (Some(b), Some(o)) = (data.get_mut(i), orig_data.get(i)) {
                *b = (*b & !bits_mask) | (*o & bits_mask);
            }
        }
    } else {
        let bpp = bits / 8;
        let wide = bit_depth == 16;
        let mut x = 0u32;
        while x < width {
            let m = mask.value(x, y);
            any_on |= m >= 128;
            // Runs of 0 and 255 are handled as a block: 255 keeps the result, 0 restores the
            // original with one copy.
            if m == 255 || m == 0 {
                let mut end = x + 1;
                while end < width && mask.value(end, y) == m {
                    end += 1;
                }
                if m == 0 {
                    let range = x as usize * bpp..end as usize * bpp;
                    if let (Some(px), Some(opx)) =
                        (data.get_mut(range.clone()), orig_data.get(range))
                    {
                        px.copy_from_slice(opx);
                    }
                } else {
                    any_on = true;
                }
                x = end;
                continue;
            }
            let start = x as usize * bpp;
            x += 1;
            let (Some(px), Some(opx)) = (
                data.get_mut(start..start + bpp),
                orig_data.get(start..start + bpp),
            ) else {
                continue;
            };
            if wide {
                for (p, o) in px.chunks_exact_mut(2).zip(opx.chunks_exact(2)) {
                    if let ([p0, p1], [o0, o1]) = (p, o) {
                        let r = u16::from_be_bytes([*p0, *p1]);
                        let ov = u16::from_be_bytes([*o0, *o1]);
                        [*p0, *p1] = blend16(ov, r, m).to_be_bytes();
                    }
                }
            } else {
                for (p, o) in px.iter_mut().zip(opx) {
                    *p = blend8(*o, *p, m);
                }
            }
        }
    }
    if !any_on {
        *tag = *orig_tag;
        if packed {
            // The padding bits after the last pixel follow the tag: the result's if any pixel
            // of the row is on, the original's otherwise.
            let start = width as usize * bits;
            let first_whole = start.div_ceil(8);
            let rem = start & 7;
            if rem > 0
                && let (Some(b), Some(o)) = (data.get_mut(start / 8), orig_data.get(start / 8))
            {
                let pad = 0xFFu8 >> rem;
                *b = (*b & !pad) | (*o & pad);
            }
            if let (Some(b), Some(o)) = (data.get_mut(first_whole..), orig_data.get(first_whole..))
            {
                b.copy_from_slice(o);
            }
        }
    }
}
