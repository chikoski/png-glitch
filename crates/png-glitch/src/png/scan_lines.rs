//! `ScanLines`: a borrowed view of the filtered scan-line buffer (ADR 0005, decision 2).
//!
//! The view offers only scan-line-layer operations. It has no operation that recomputes filtered
//! values, changes the header, resizes, compresses or allocates, so code written against it
//! (the glitch step `G` of the live pipeline) cannot break those rules.

use super::scan_line::ScanLine;
use super::{ColorType, FilterType, Png};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// A borrowed view of a model's filtered scan lines.
///
/// Every row is `stride` bytes: the filter-type tag followed by the filtered bytes, exactly as
/// stored in the model. The view also carries the header fields an effect needs (width, colour
/// type, bit depth) and a scratch buffer of at least one scan line, allocated once by the owner.
///
/// Get one from [`PngGlitch::scan_lines_mut`](crate::PngGlitch::scan_lines_mut),
/// [`LivePipeline::encode`](crate::LivePipeline::encode) or
/// [`LivePipeline::scan_lines`](crate::LivePipeline::scan_lines).
///
/// Out-of-range row indices never panic: the methods return `None` or `false`, or do nothing.
///
/// # Examples
/// ```
/// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
///
/// let frame = vec![0u8; 4 * 4 * 4];
/// let mut png = PngGlitch::from_pixels(
///     PixelSource::rgba8(4, 4, &frame),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub)),
/// )?;
/// let mut lines = png.scan_lines_mut();
/// assert_eq!(lines.len(), 4);
/// lines.set_filter_type(1, FilterType::Up);
/// assert_eq!(lines.filter_type(1), Some(FilterType::Up));
/// assert!(lines.swap(0, 3));
/// assert!(!lines.swap(0, 9)); // out of range: nothing happens
/// # Ok::<(), png_glitch::PngError>(())
/// ```
#[derive(Debug)]
pub struct ScanLines<'a> {
    data: &'a mut [u8],
    stride: usize,
    color_type: ColorType,
    bit_depth: u8,
    width: u32,
    scratch: &'a mut [u8],
}

impl<'a> ScanLines<'a> {
    /// Lends the scan lines of `png` together with `scratch`.
    pub(crate) fn new(png: &'a mut Png, scratch: &'a mut [u8]) -> ScanLines<'a> {
        ScanLines {
            stride: png.header.scan_line_width(),
            color_type: png.header.color_type(),
            bit_depth: png.header.bit_depth(),
            width: png.header.width(),
            data: &mut png.data,
            scratch,
        }
    }

    /// The stored bytes of all rows, tags included (crate-internal; effects wrapping others use
    /// it to snapshot and blend).
    pub(crate) fn bytes(&self) -> &[u8] {
        self.data
    }

    /// The stored bytes of all rows, mutable (crate-internal).
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        self.data
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.data.len().checked_div(self.stride).unwrap_or(0)
    }

    /// `true` if there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Width of the image in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Colour type of the image.
    pub fn color_type(&self) -> ColorType {
        self.color_type
    }

    /// Bit depth of the image.
    pub fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    /// Bytes per row, including the filter-type tag.
    pub fn stride(&self) -> usize {
        self.stride
    }

    fn range(&self, first: usize, lines: usize) -> Option<std::ops::Range<usize>> {
        let start = first.checked_mul(self.stride)?;
        let end = first.checked_add(lines)?.checked_mul(self.stride)?;
        (end <= self.data.len()).then_some(start..end)
    }

    /// Row `i` as a [`ScanLine`], or `None` if `i` is out of range.
    pub fn get_mut(&mut self, i: usize) -> Option<ScanLine<'_>> {
        let range = self.range(i, 1)?;
        let (color_type, bit_depth, width) = (self.color_type, self.bit_depth, self.width);
        let chunk = self.data.get_mut(range)?;
        Some(ScanLine::new(chunk, color_type, bit_depth, width))
    }

    /// Two different rows at once, or `None` if either index is out of range or `a == b`.
    pub fn pair_mut(&mut self, a: usize, b: usize) -> Option<(ScanLine<'_>, ScanLine<'_>)> {
        if a == b {
            return None;
        }
        let (lo, hi, swapped) = if a < b { (a, b, false) } else { (b, a, true) };
        let lo_range = self.range(lo, 1)?;
        let hi_range = self.range(hi, 1)?;
        let (color_type, bit_depth, width) = (self.color_type, self.bit_depth, self.width);
        let (head, tail) = self.data.split_at_mut(hi_range.start);
        let first = ScanLine::new(head.get_mut(lo_range)?, color_type, bit_depth, width);
        let second = ScanLine::new(
            tail.get_mut(..hi_range.end - hi_range.start)?,
            color_type,
            bit_depth,
            width,
        );
        Some(if swapped {
            (second, first)
        } else {
            (first, second)
        })
    }

    /// A view of the rows in `range`, sharing this view's scratch buffer. Inside the sub-view,
    /// row 0 is row `range.start` of this view, so an effect with absolute row indices works
    /// unchanged on it.
    ///
    /// `range` is clamped to the image: an empty range, or one that starts at or past the last
    /// row, gives an empty view. Nothing is allocated and nothing panics.
    ///
    /// Only the stored bytes (filter-type tag and filtered bytes) of rows outside `range` are
    /// guaranteed to stay unchanged. A decoder reconstructs each row from the one above, so
    /// the *decoded* pixels below `range` change too unless those rows use `None` or `Sub`
    /// (ADR 0002).
    ///
    /// # Examples
    /// ```
    /// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
    ///
    /// let frame = vec![0u8; 4 * 6 * 4];
    /// let mut png = PngGlitch::from_pixels(
    ///     PixelSource::rgba8(4, 6, &frame),
    ///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
    /// )?;
    /// let mut lines = png.scan_lines_mut();
    /// let mut part = lines.rows(2..4);
    /// assert_eq!(part.len(), 2);
    /// part.set_filter_type(0, FilterType::Up); // row 2 of the image
    /// assert!(part.get_mut(2).is_none());
    /// assert_eq!(lines.filter_type(2), Some(FilterType::Up));
    /// assert_eq!(lines.rows(9..12).len(), 0);
    /// # Ok::<(), png_glitch::PngError>(())
    /// ```
    pub fn rows(&mut self, range: std::ops::Range<u32>) -> ScanLines<'_> {
        let len = self.len();
        let start = (range.start as usize).min(len);
        let end = (range.end as usize).min(len).max(start);
        // `start..end` is within `len` rows, so the byte bounds are within `data`.
        let bytes = start * self.stride..end * self.stride;
        ScanLines {
            data: self.data.get_mut(bytes).unwrap_or_default(),
            stride: self.stride,
            color_type: self.color_type,
            bit_depth: self.bit_depth,
            width: self.width,
            scratch: &mut *self.scratch,
        }
    }

    /// Calls `f` for every row, top to bottom.
    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut ScanLine)) {
        if self.stride == 0 {
            return;
        }
        let (color_type, bit_depth, width) = (self.color_type, self.bit_depth, self.width);
        for chunk in self.data.chunks_exact_mut(self.stride) {
            f(&mut ScanLine::new(chunk, color_type, bit_depth, width));
        }
    }

    /// Calls `f` for every row, in parallel with the `parallel` feature (in no fixed order, so
    /// `f` must not use a shared RNG), sequentially in row order without it. Mirrors
    /// [`PngGlitch::par_foreach_scanline`](crate::PngGlitch::par_foreach_scanline).
    pub fn par_for_each(&mut self, f: impl Fn(&mut ScanLine) + Sync + Send) {
        if self.stride == 0 {
            return;
        }
        let (color_type, bit_depth, width) = (self.color_type, self.bit_depth, self.width);
        #[cfg(feature = "parallel")]
        self.data
            .par_chunks_exact_mut(self.stride)
            .for_each(|chunk| f(&mut ScanLine::new(chunk, color_type, bit_depth, width)));
        #[cfg(not(feature = "parallel"))]
        self.data
            .chunks_exact_mut(self.stride)
            .for_each(|chunk| f(&mut ScanLine::new(chunk, color_type, bit_depth, width)));
    }

    /// Swaps rows `a` and `b` (tags included). Returns `false` and does nothing if either index
    /// is out of range. `a == b` is a no-op that returns `true`.
    pub fn swap(&mut self, a: usize, b: usize) -> bool {
        self.swap_range(a, b, 1)
    }

    /// Swaps `lines` rows starting at `a` with `lines` rows starting at `b`. Returns `false` and
    /// does nothing if a range is out of range or the two ranges overlap (unless `a == b`).
    pub fn swap_range(&mut self, a: usize, b: usize, lines: usize) -> bool {
        let (Some(ra), Some(rb)) = (self.range(a, lines), self.range(b, lines)) else {
            return false;
        };
        if a == b || lines == 0 {
            return true;
        }
        let (lo, hi) = if ra.start < rb.start {
            (ra, rb)
        } else {
            (rb, ra)
        };
        if lo.end > hi.start {
            return false;
        }
        let (head, tail) = self.data.split_at_mut(hi.start);
        match (head.get_mut(lo), tail.get_mut(..hi.end - hi.start)) {
            (Some(x), Some(y)) => {
                x.swap_with_slice(y);
                true
            }
            _ => false,
        }
    }

    /// Copies row `src` over row `dst`, tag included. Returns `false` if either is out of range.
    pub fn copy(&mut self, src: usize, dst: usize) -> bool {
        self.copy_range(src, dst, 1)
    }

    /// Copies `lines` rows starting at `src` over the rows starting at `dst` (the ranges may
    /// overlap, like `copy_within`). Returns `false` and does nothing if a range is out of range.
    pub fn copy_range(&mut self, src: usize, dst: usize, lines: usize) -> bool {
        let (Some(rs), Some(rd)) = (self.range(src, lines), self.range(dst, lines)) else {
            return false;
        };
        self.data.copy_within(rs, rd.start);
        true
    }

    /// The filter-type tag of row `i`. `None` if `i` is out of range. A tag byte above 4 reads
    /// as [`FilterType::None`], like [`ScanLine::filter_type`].
    pub fn filter_type(&mut self, i: usize) -> Option<FilterType> {
        self.get_mut(i).map(|row| row.filter_type())
    }

    /// Overwrites the filter-type tag of row `i` (the filtered bytes stay). Does nothing if `i`
    /// is out of range.
    pub fn set_filter_type(&mut self, i: usize, filter_type: FilterType) {
        if let Some(mut row) = self.get_mut(i) {
            row.set_filter_type(filter_type);
        }
    }

    /// The scratch buffer: at least one scan line long, with unspecified contents. Use it for
    /// temporary rows instead of allocating.
    pub fn scratch(&mut self) -> &mut [u8] {
        self.scratch
    }

    /// Row `i` and the scratch buffer at once, for effects that stage a row in the scratch.
    pub fn row_and_scratch(&mut self, i: usize) -> Option<(ScanLine<'_>, &mut [u8])> {
        let range = self.range(i, 1)?;
        let (color_type, bit_depth, width) = (self.color_type, self.bit_depth, self.width);
        let chunk = self.data.get_mut(range)?;
        Some((
            ScanLine::new(chunk, color_type, bit_depth, width),
            &mut *self.scratch,
        ))
    }
}
