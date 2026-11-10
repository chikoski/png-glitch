//! Deterministic effects whose "where" and "how much" are explicit parameters.
//!
//! A randomized effect picks rows, bytes or offsets with an RNG. These effects take those
//! choices as values instead, so the same image and parameters always give the same bytes.
//! A caller that wants randomness draws the parameters from its own RNG.

use super::{Effect, ScanLinesEffect};
use crate::{ColorType, FilterType, Pixel, PngGlitch, ScanLine, ScanLines};

#[cfg(test)]
mod reference;

/// Runs `f` on each listed row, in the given order. Rows outside the image are ignored.
fn for_rows(lines: &mut ScanLines<'_>, rows: &[u32], mut f: impl FnMut(&mut ScanLine)) {
    for &row in rows {
        if let Some(mut line) = lines.get_mut(row as usize) {
            f(&mut line);
        }
    }
}

/// Re-encodes every scan line with the given filter type, baking earlier glitches into pixels.
///
/// This is the effect form of [`PngGlitch::reencode_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refilter {
    /// The filter type every scan line is re-encoded with.
    pub filter_type: FilterType,
}

impl Refilter {
    /// Creates a new [`Refilter`] effect.
    pub fn new(filter_type: FilterType) -> Self {
        Self { filter_type }
    }
}

impl Refilter {
    /// A form of this effect that recomputes only the rows in `range` (clamped to the image).
    ///
    /// The stored bytes of the other rows are unchanged; see [`RefilterRows`].
    pub fn within_rows(self, range: std::ops::Range<u32>) -> RefilterRows {
        RefilterRows {
            filter_type: self.filter_type,
            range,
        }
    }
}

impl Effect for Refilter {
    fn apply_to(&self, png: &mut PngGlitch) {
        png.reencode_with(self.filter_type);
    }
}

/// [`Refilter`] restricted to a range of rows, made by [`Refilter::within_rows`].
///
/// The rows in `range` are decoded (with everything above them) and re-encoded with
/// `filter_type`, using the decoded row above the range as the previous line. The tag and
/// filtered bytes of every row outside the range stay as they were. The *decoded* pixels below
/// the range can still change, because filters propagate downwards (ADR 0002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefilterRows {
    /// The filter type the rows are re-encoded with.
    pub filter_type: FilterType,
    /// The rows to re-encode (clamped to the image; empty means no-op).
    pub range: std::ops::Range<u32>,
}

impl Effect for RefilterRows {
    fn apply_to(&self, png: &mut PngGlitch) {
        let lines = self.range.end.saturating_sub(self.range.start);
        png.reencode_rows_with(self.filter_type, self.range.start, lines);
    }
}

/// Applies an effect to the rows in `range` only. Make one with
/// [`ScanLinesEffect::within_rows`].
///
/// Inside the effect, row indices are relative to the range: row 0 is row `range.start` of the
/// image, so effects with absolute row lists (such as [`SwapLines`]) work unchanged. The range
/// is clamped to the image; an empty range, or one past the last row, is a no-op.
///
/// The tag and filtered bytes of rows outside the range are never touched. The *decoded* pixels
/// below the range can still change, because a decoder rebuilds each row from the one above
/// (ADR 0002).
///
/// # Examples
/// ```
/// use png_glitch::effects::{Invert, ScanLinesEffect};
/// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
///
/// let frame = vec![10u8; 4 * 6 * 4];
/// let mut png = PngGlitch::from_pixels(
///     PixelSource::rgba8(4, 6, &frame),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
/// )?;
/// let before = png.data().to_vec();
/// png.apply(Invert::new().within_rows(2..4));
/// let stride = before.len() / 6;
/// assert_eq!(png.data()[..2 * stride], before[..2 * stride]);
/// assert_ne!(png.data()[2 * stride..4 * stride], before[2 * stride..4 * stride]);
/// assert_eq!(png.data()[4 * stride..], before[4 * stride..]);
/// # Ok::<(), png_glitch::PngError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithinRows<E> {
    /// The rows the effect is applied to.
    pub range: std::ops::Range<u32>,
    /// The effect.
    pub effect: E,
}

impl<E: ScanLinesEffect> ScanLinesEffect for WithinRows<E> {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        self.effect
            .apply_to_scan_lines(&mut lines.rows(self.range.clone()));
    }
}

/// Overwrites the filter-type byte of each scan line, without touching the filtered data.
///
/// `filter_types[i]` is the new filter type of row `i`; `None` leaves the row unchanged.
/// Extra entries are ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetFilterTypes {
    /// New filter type per row.
    pub filter_types: Vec<Option<FilterType>>,
}

impl SetFilterTypes {
    /// Creates a new [`SetFilterTypes`] effect.
    pub fn new(filter_types: Vec<Option<FilterType>>) -> Self {
        Self { filter_types }
    }
}

impl ScanLinesEffect for SetFilterTypes {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for (row, filter_type) in self.filter_types.iter().enumerate() {
            if let Some(filter_type) = filter_type {
                lines.set_filter_type(row, *filter_type);
            }
        }
    }
}

/// A single byte write: byte `index` (after the filter-type byte) of row `line` becomes `value`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByteEdit {
    /// Row index.
    pub line: u32,
    /// Byte offset within the row's filtered data.
    pub index: usize,
    /// New byte value.
    pub value: u8,
}

/// Writes individual bytes of the filtered scan-line data, in order.
///
/// Edits that point outside the image are ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetBytes {
    /// The writes to perform.
    pub edits: Vec<ByteEdit>,
}

impl SetBytes {
    /// Creates a new [`SetBytes`] effect.
    pub fn new(edits: Vec<ByteEdit>) -> Self {
        Self { edits }
    }
}

impl ScanLinesEffect for SetBytes {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for edit in &self.edits {
            if let Some(mut line) = lines.get_mut(edit.line as usize) {
                line.update(edit.index, edit.value);
            }
        }
    }
}

/// Sets the byte at `index` to `value` on every scan line that is long enough.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Substitute {
    /// Byte offset within each scan line's filtered data.
    pub index: usize,
    /// Replacement byte value.
    pub value: u8,
}

impl Substitute {
    /// Creates a new [`Substitute`] effect.
    pub fn new(index: usize, value: u8) -> Self {
        Self { index, value }
    }
}

impl ScanLinesEffect for Substitute {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        lines.for_each_mut(|line| {
            if self.index < line.size() {
                line.update(self.index, self.value);
            }
        });
    }
}

/// Swaps pairs of scan lines, in order. Pairs with equal or out-of-range rows are skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwapLines {
    /// The `(a, b)` row pairs to swap.
    pub swaps: Vec<(u32, u32)>,
}

impl SwapLines {
    /// Creates a new [`SwapLines`] effect.
    pub fn new(swaps: Vec<(u32, u32)>) -> Self {
        Self { swaps }
    }
}

impl ScanLinesEffect for SwapLines {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for &(a, b) in &self.swaps {
            if a != b {
                lines.swap(a as usize, b as usize);
            }
        }
    }
}

/// Copies scan lines (data and filter type) onto other scan lines, in order.
///
/// Pairs with equal or out-of-range rows are skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyLines {
    /// The `(source, destination)` row pairs.
    pub copies: Vec<(u32, u32)>,
}

impl CopyLines {
    /// Creates a new [`CopyLines`] effect.
    pub fn new(copies: Vec<(u32, u32)>) -> Self {
        Self { copies }
    }
}

impl ScanLinesEffect for CopyLines {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for &(src, dst) in &self.copies {
            if src != dst {
                lines.copy(src as usize, dst as usize);
            }
        }
    }
}

/// A rotation of one scan line's filtered bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineShift {
    /// Row index.
    pub line: u32,
    /// Number of bytes the row's data moves to the right (wrapping around).
    pub shift: usize,
}

/// Rotates the filtered bytes of the listed scan lines to the right, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShiftLines {
    /// The rotations to perform.
    pub shifts: Vec<LineShift>,
}

impl ShiftLines {
    /// Creates a new [`ShiftLines`] effect.
    pub fn new(shifts: Vec<LineShift>) -> Self {
        Self { shifts }
    }
}

impl ScanLinesEffect for ShiftLines {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for s in &self.shifts {
            let Some(line) = lines.get_mut(s.line as usize) else {
                continue;
            };
            let size = line.size();
            if size <= 1 {
                continue;
            }
            let offset = line.pixel_data_offset();
            if let Some(data) = line.data.get_mut(offset..) {
                data.rotate_right(s.shift % size);
            }
        }
    }
}

/// Criterion used by [`PixelSort`] to order pixels within a scan line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortCriterion {
    /// Sorts pixels by average RGB brightness `(r + g + b) / 3`.
    #[default]
    Brightness,
    /// Sorts pixels by HSL hue angle (`0.0..=360.0`).
    Hue,
}

/// Sorts the pixels of the listed rows by brightness or hue.
///
/// The sort is stable: pixels with the same value keep their order. For 8 and 16 bits per
/// sample it is a merge sort in place, with the view's scratch buffer (one scan line) as the
/// other half, so it makes no heap allocation (#208). For 1, 2 and 4 bits per sample (packed
/// pixels, which the scratch buffer cannot hold one per byte) it still allocates a pixel list
/// per sorted row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PixelSort {
    /// Criterion used to compare pixels.
    pub criterion: SortCriterion,
    /// Rows to sort.
    pub rows: Vec<u32>,
}

impl PixelSort {
    /// Creates a new [`PixelSort`] effect.
    pub fn new(criterion: SortCriterion, rows: Vec<u32>) -> Self {
        Self { criterion, rows }
    }

    fn value(&self, p: &Pixel) -> f64 {
        match self.criterion {
            SortCriterion::Brightness => (p.r() as f64 + p.g() as f64 + p.b() as f64) / 3.0,
            SortCriterion::Hue => {
                let r = p.r() as f64 / 65535.0;
                let g = p.g() as f64 / 65535.0;
                let b = p.b() as f64 / 65535.0;
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                if max == min {
                    0.0
                } else if max == r {
                    (60.0 * ((g - b) / (max - min)) + 360.0) % 360.0
                } else if max == g {
                    (60.0 * ((b - r) / (max - min)) + 120.0) % 360.0
                } else {
                    (60.0 * ((r - g) / (max - min)) + 240.0) % 360.0
                }
            }
        }
    }

    /// The brightness order without floating point: `(r + g + b) / 3` as `f64` is exact for
    /// these sums, so comparing the sums gives the same order (and the same ties).
    fn brightness_key(p: &Pixel) -> u32 {
        u32::from(p.r()) + u32::from(p.g()) + u32::from(p.b())
    }

    /// Sorts the listed rows of `lines`, whose pixels are `BPP` bytes each and decode with
    /// `decode` (a colour type and bit depth fixed at compile time, so that it inlines).
    fn sort_rows<const BPP: usize>(
        &self,
        lines: &mut ScanLines<'_>,
        decode: impl Fn(&[u8; BPP]) -> Pixel + Copy,
    ) {
        match self.criterion {
            SortCriterion::Brightness => {
                self.sort_rows_by(lines, move |c| Self::brightness_key(&decode(c)))
            }
            SortCriterion::Hue => self.sort_rows_by(lines, move |c| self.value(&decode(c))),
        }
    }

    fn sort_rows_by<const BPP: usize, K: PartialOrd>(
        &self,
        lines: &mut ScanLines<'_>,
        key: impl Fn(&[u8; BPP]) -> K,
    ) {
        let width = lines.width() as usize;
        for &row in &self.rows {
            let Some((line, scratch)) = lines.row_and_scratch(row as usize) else {
                continue;
            };
            // The pixels the row holds: a malformed row may be shorter than `width` pixels.
            let start = line.pixel_data_offset();
            let data = line.data.get_mut(start..).unwrap_or_default();
            let (pixels, _) = data.as_chunks_mut::<BPP>();
            let (aux, _) = scratch.as_chunks_mut::<BPP>();
            let n = width.min(pixels.len());
            if let (Some(pixels), Some(aux)) = (pixels.get_mut(..n), aux.get_mut(..n)) {
                stable_sort_by_key(pixels, aux, &key);
            }
        }
    }

    /// The packed (1, 2 and 4 bits per sample) rows: a pixel list per row.
    fn sort_packed(&self, line: &mut ScanLine) {
        let mut pixels = Vec::with_capacity(line.pixels_count() as usize);
        for i in 0..line.pixels_count() as usize {
            if let Some(p) = line.get_pixel(i) {
                pixels.push(p);
            }
        }
        pixels.sort_by(|a, b| {
            self.value(a)
                .partial_cmp(&self.value(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, p) in pixels.into_iter().enumerate() {
            line.set_pixel(i, p);
        }
    }
}

impl ScanLinesEffect for PixelSort {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        use ColorType::*;
        let u16be = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
        // One instantiation per colour type and bit depth, so that decoding inlines into the
        // comparisons. The decoders are those of `ScanLine::get_pixel`, written out.
        match (lines.color_type(), lines.bit_depth()) {
            (_, 1 | 2 | 4) => for_rows(lines, &self.rows, |line| self.sort_packed(line)),
            (GrayScale, 8) => self.sort_rows(lines, |&[v]: &[u8; 1]| Pixel::Gray(v.into())),
            (GrayScale, 16) => {
                self.sort_rows(lines, move |&[a, b]: &[u8; 2]| Pixel::Gray(u16be(a, b)))
            }
            (IndexColor, 8) => self.sort_rows(lines, |&[v]: &[u8; 1]| Pixel::Indexed(v)),
            (GrayScaleAlpha, 8) => self.sort_rows(lines, |&[v, a]: &[u8; 2]| {
                Pixel::GrayAlpha(v.into(), a.into())
            }),
            (GrayScaleAlpha, 16) => self.sort_rows(lines, move |&[v0, v1, a0, a1]: &[u8; 4]| {
                Pixel::GrayAlpha(u16be(v0, v1), u16be(a0, a1))
            }),
            (TrueColor, 8) => self.sort_rows(lines, |&[r, g, b]: &[u8; 3]| {
                Pixel::RGB(r.into(), g.into(), b.into())
            }),
            (TrueColor, 16) => self.sort_rows(lines, move |&[r0, r1, g0, g1, b0, b1]: &[u8; 6]| {
                Pixel::RGB(u16be(r0, r1), u16be(g0, g1), u16be(b0, b1))
            }),
            (TrueColorAlpha, 8) => self.sort_rows(lines, |&[r, g, b, a]: &[u8; 4]| {
                Pixel::RGBA(r.into(), g.into(), b.into(), a.into())
            }),
            (TrueColorAlpha, 16) => {
                self.sort_rows(lines, move |&[r0, r1, g0, g1, b0, b1, a0, a1]: &[u8; 8]| {
                    Pixel::RGBA(u16be(r0, r1), u16be(g0, g1), u16be(b0, b1), u16be(a0, a1))
                })
            }
            // No such pixels: `get_pixel` decodes nothing, so nothing was sorted before either.
            _ => {}
        }
    }
}

/// Runs shorter than this are sorted by insertion before merging.
const SORT_RUN: usize = 16;

/// A stable sort of `v` by `key` (ascending, with `<`: incomparable keys count as equal, as with
/// `sort_by(partial_cmp)` and `unwrap_or(Equal)`), with `aux` (at least as long as `v`) as the
/// merge buffer: insertion-sorted runs, then bottom-up merges back and forth between `v` and
/// `aux`. Any stable sort gives the same result for the same order, so this matches
/// `slice::sort_by`, without its heap buffer.
fn stable_sort_by_key<T: Copy, K: PartialOrd>(v: &mut [T], aux: &mut [T], key: impl Fn(&T) -> K) {
    let n = v.len();
    let Some(aux) = aux.get_mut(..n) else {
        return;
    };
    for run in v.chunks_mut(SORT_RUN) {
        insertion_sort(run, &key);
    }
    let mut width = SORT_RUN;
    // `true` while the sorted runs are in `aux`.
    let mut in_aux = false;
    while width < n {
        let (src, dst): (&[T], &mut [T]) = if in_aux {
            (&*aux, &mut *v)
        } else {
            (&*v, &mut *aux)
        };
        for (src, dst) in src.chunks(2 * width).zip(dst.chunks_mut(2 * width)) {
            let (left, right) = src.split_at(width.min(src.len()));
            merge(left, right, dst, &key);
        }
        in_aux = !in_aux;
        width *= 2;
    }
    if in_aux {
        v.copy_from_slice(aux);
    }
}

fn insertion_sort<T: Copy, K: PartialOrd>(v: &mut [T], key: &impl Fn(&T) -> K) {
    for i in 1..v.len() {
        let Some(item) = v.get(i) else {
            return;
        };
        let k = key(item);
        let mut j = i;
        while let Some(prev) = j.checked_sub(1).and_then(|p| v.get(p))
            && k < key(prev)
        {
            j -= 1;
        }
        if let Some(part) = v.get_mut(j..=i) {
            part.rotate_right(1);
        }
    }
}

/// Merges the sorted `left` and `right` into `out` (`left.len() + right.len()` long). On a tie
/// the element of `left` comes first, which keeps the sort stable. Each element's key is
/// computed once.
fn merge<T: Copy, K: PartialOrd>(left: &[T], right: &[T], out: &mut [T], key: &impl Fn(&T) -> K) {
    // Already in order (common on smooth rows): copy.
    if let (Some(last), Some(first)) = (left.last(), right.first())
        && key(first).partial_cmp(&key(last)) != Some(std::cmp::Ordering::Less)
    {
        let (a, b) = out.split_at_mut(left.len().min(out.len()));
        a.copy_from_slice(left.get(..a.len()).unwrap_or_default());
        b.copy_from_slice(right.get(..b.len()).unwrap_or_default());
        return;
    }
    let (mut l, mut r) = (left.iter(), right.iter());
    let (mut a, mut b) = (l.next(), r.next());
    let (mut ka, mut kb) = (a.map(key), b.map(key));
    for o in out {
        match (a, b) {
            (Some(x), Some(y)) => {
                if kb.as_ref().zip(ka.as_ref()).is_some_and(|(kb, ka)| kb < ka) {
                    *o = *y;
                    b = r.next();
                    kb = b.map(key);
                } else {
                    *o = *x;
                    a = l.next();
                    ka = a.map(key);
                }
            }
            (Some(x), None) => {
                *o = *x;
                a = l.next();
            }
            (None, Some(y)) => {
                *o = *y;
                b = r.next();
            }
            (None, None) => return,
        }
    }
}

/// Bitwise operation performed by [`Bitwise`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitOp {
    /// Bitwise AND (`b & value`).
    And,
    /// Bitwise OR (`b | value`).
    Or,
    /// Bitwise XOR (`b ^ value`).
    #[default]
    Xor,
}

/// Applies a bitwise operation to every filtered byte of the listed rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bitwise {
    /// The operator.
    pub op: BitOp,
    /// Right-hand byte operand.
    pub value: u8,
    /// Rows to modify.
    pub rows: Vec<u32>,
}

impl Bitwise {
    /// Creates a new [`Bitwise`] effect.
    pub fn new(op: BitOp, value: u8, rows: Vec<u32>) -> Self {
        Self { op, value, rows }
    }
}

impl ScanLinesEffect for Bitwise {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for_rows(lines, &self.rows, |line| {
            for i in 0..line.size() {
                if let Some(b) = line.index(i) {
                    let new_b = match self.op {
                        BitOp::And => b & self.value,
                        BitOp::Or => b | self.value,
                        BitOp::Xor => b ^ self.value,
                    };
                    line.update(i, new_b);
                }
            }
        });
    }
}

/// Pair of color channels to swap in [`SwapChannels`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwapTarget {
    /// Swaps red and green.
    #[default]
    Rg,
    /// Swaps green and blue.
    Gb,
    /// Swaps blue and red.
    Br,
}

/// Swaps a pair of color channels in every pixel of the listed rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwapChannels {
    /// The channel pair.
    pub target: SwapTarget,
    /// Rows to modify.
    pub rows: Vec<u32>,
}

impl SwapChannels {
    /// Creates a new [`SwapChannels`] effect.
    pub fn new(target: SwapTarget, rows: Vec<u32>) -> Self {
        Self { target, rows }
    }
}

impl ScanLinesEffect for SwapChannels {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for_rows(lines, &self.rows, |line| {
            line.process_pixels(|_, p| match p {
                Pixel::RGB(r, g, b) => match self.target {
                    SwapTarget::Rg => Pixel::RGB(g, r, b),
                    SwapTarget::Gb => Pixel::RGB(r, b, g),
                    SwapTarget::Br => Pixel::RGB(b, g, r),
                },
                Pixel::RGBA(r, g, b, a) => match self.target {
                    SwapTarget::Rg => Pixel::RGBA(g, r, b, a),
                    SwapTarget::Gb => Pixel::RGBA(r, b, g, a),
                    SwapTarget::Br => Pixel::RGBA(b, g, r, a),
                },
                _ => p,
            });
        });
    }
}

/// Per-pixel color offsets for one row, used by [`OffsetColors`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineOffsets {
    /// Row index.
    pub line: u32,
    /// `[r, g, b]` offsets for pixel `x`, added with wrapping. Missing entries mean no change.
    pub offsets: Vec<[i16; 3]>,
}

/// Adds explicit per-pixel offsets to the color channels of the listed rows.
///
/// Gray pixels use the red offset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OffsetColors {
    /// The rows and their offsets.
    pub lines: Vec<LineOffsets>,
}

impl OffsetColors {
    /// Creates a new [`OffsetColors`] effect.
    pub fn new(lines: Vec<LineOffsets>) -> Self {
        Self { lines }
    }
}

impl ScanLinesEffect for OffsetColors {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        for entry in &self.lines {
            let Some(mut line) = lines.get_mut(entry.line as usize) else {
                continue;
            };
            line.process_pixels(|x, p| {
                let [r_off, g_off, b_off] = entry.offsets.get(x).copied().unwrap_or([0; 3]);
                match p {
                    Pixel::RGB(r, g, b) => Pixel::RGB(
                        r.wrapping_add_signed(r_off),
                        g.wrapping_add_signed(g_off),
                        b.wrapping_add_signed(b_off),
                    ),
                    Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                        r.wrapping_add_signed(r_off),
                        g.wrapping_add_signed(g_off),
                        b.wrapping_add_signed(b_off),
                        a,
                    ),
                    Pixel::Gray(v) => Pixel::Gray(v.wrapping_add_signed(r_off)),
                    Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(v.wrapping_add_signed(r_off), a),
                    _ => p,
                }
            });
        }
    }
}

/// Rotates hue and scales saturation and lightness in HSL space for the listed rows.
#[derive(Clone, Debug, PartialEq)]
pub struct HslShift {
    /// Hue rotation in degrees. Non-finite values are treated as `0.0`.
    pub hue_shift: f64,
    /// Saturation multiplier. Negative or non-finite values are treated as `1.0`.
    pub saturation_mult: f64,
    /// Lightness multiplier. Negative or non-finite values are treated as `1.0`.
    pub lightness_mult: f64,
    /// Rows to modify.
    pub rows: Vec<u32>,
}

impl Default for HslShift {
    fn default() -> Self {
        Self {
            hue_shift: 0.0,
            saturation_mult: 1.0,
            lightness_mult: 1.0,
            rows: Vec::new(),
        }
    }
}

impl HslShift {
    /// Creates a new [`HslShift`] effect.
    pub fn new(hue_shift: f64, saturation_mult: f64, lightness_mult: f64, rows: Vec<u32>) -> Self {
        Self {
            hue_shift,
            saturation_mult,
            lightness_mult,
            rows,
        }
    }
}

impl ScanLinesEffect for HslShift {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        let hue_shift = if self.hue_shift.is_finite() {
            self.hue_shift
        } else {
            0.0
        };
        let sat_mult = if self.saturation_mult.is_finite() && self.saturation_mult >= 0.0 {
            self.saturation_mult
        } else {
            1.0
        };
        let light_mult = if self.lightness_mult.is_finite() && self.lightness_mult >= 0.0 {
            self.lightness_mult
        } else {
            1.0
        };
        let transform = |r, g, b| {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let h = (h + hue_shift).rem_euclid(360.0);
            let s = (s * sat_mult).clamp(0.0, 1.0);
            let l = (l * light_mult).clamp(0.0, 1.0);
            hsl_to_rgb(h, s, l)
        };
        for_rows(lines, &self.rows, |line| {
            line.process_pixels(|_, p| match p {
                Pixel::RGB(r, g, b) => {
                    let (r, g, b) = transform(r, g, b);
                    Pixel::RGB(r, g, b)
                }
                Pixel::RGBA(r, g, b, a) => {
                    let (r, g, b) = transform(r, g, b);
                    Pixel::RGBA(r, g, b, a)
                }
                _ => p,
            });
        });
    }
}

fn rgb_to_hsl(r: u16, g: u16, b: u16) -> (f64, f64, f64) {
    let r = r as f64 / 65535.0;
    let g = g as f64 / 65535.0;
    let b = b as f64 / 65535.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if max == min {
        (0.0, 0.0, l)
    } else {
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + (if g < b { 6.0 } else { 0.0 })
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u16, u16, u16) {
    let h = h / 360.0;
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        (
            hue_to_rgb(p, q, h + 1.0 / 3.0),
            hue_to_rgb(p, q, h),
            hue_to_rgb(p, q, h - 1.0 / 3.0),
        )
    };
    (
        (r * 65535.0) as u16,
        (g * 65535.0) as u16,
        (b * 65535.0) as u16,
    )
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Shifts the red, green and blue channels horizontally by different amounts (wrapping) in the
/// listed rows, producing chromatic aberration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelShift {
    /// Horizontal offset in pixels for the red channel.
    pub r_offset: i32,
    /// Horizontal offset in pixels for the green channel.
    pub g_offset: i32,
    /// Horizontal offset in pixels for the blue channel.
    pub b_offset: i32,
    /// Rows to modify.
    pub rows: Vec<u32>,
}

impl Default for ChannelShift {
    fn default() -> Self {
        Self {
            r_offset: 2,
            g_offset: 0,
            b_offset: -2,
            rows: Vec::new(),
        }
    }
}

impl ChannelShift {
    /// Creates a new [`ChannelShift`] effect.
    pub fn new(r_offset: i32, g_offset: i32, b_offset: i32, rows: Vec<u32>) -> Self {
        Self {
            r_offset,
            g_offset,
            b_offset,
            rows,
        }
    }
}

impl ScanLinesEffect for ChannelShift {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        let width = lines.width() as usize;
        let channels = match lines.color_type() {
            ColorType::TrueColor => 3,
            ColorType::TrueColorAlpha => 4,
            // The effect only touches RGB and RGBA pixels.
            _ => return,
        };
        let sample = usize::from(lines.bit_depth() / 8);
        if width == 0 || sample == 0 {
            return;
        }
        let offsets = [self.r_offset, self.g_offset, self.b_offset];
        for &row in &self.rows {
            let Some((line, scratch)) = lines.row_and_scratch(row as usize) else {
                continue;
            };
            // The original row, copied into the scratch buffer, is what the shifted channels read.
            let Some(original) = scratch.get_mut(..line.data.len()) else {
                continue;
            };
            original.copy_from_slice(line.data);
            let start = line.pixel_data_offset();
            for x in 0..width {
                for (channel, offset) in offsets.iter().enumerate() {
                    let nx = (x as i64 + i64::from(*offset)).rem_euclid(width as i64) as usize;
                    let from = start + (nx * channels + channel) * sample;
                    let to = start + (x * channels + channel) * sample;
                    if let (Some(src), Some(dst)) = (
                        original.get(from..from + sample),
                        line.data.get_mut(to..to + sample),
                    ) {
                        dst.copy_from_slice(src);
                    }
                }
            }
        }
    }
}

/// A swap of two square blocks, in block coordinates `(x, y)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockSwap {
    /// First block.
    pub src: (u32, u32),
    /// Second block.
    pub dst: (u32, u32),
}

/// Swaps pairs of square pixel blocks, in order.
///
/// The image is divided into whole `block_size` × `block_size` blocks (a partial block at the
/// right or bottom edge is not addressable). Swaps that are out of range or between a block and
/// itself are skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwapBlocks {
    /// Edge length of each block in pixels.
    pub block_size: u32,
    /// The swaps to perform.
    pub swaps: Vec<BlockSwap>,
}

impl Default for SwapBlocks {
    fn default() -> Self {
        Self {
            block_size: 16,
            swaps: Vec::new(),
        }
    }
}

impl SwapBlocks {
    /// Creates a new [`SwapBlocks`] effect.
    pub fn new(block_size: u32, swaps: Vec<BlockSwap>) -> Self {
        Self { block_size, swaps }
    }
}

impl ScanLinesEffect for SwapBlocks {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        let width = lines.width();
        let height = lines.len() as u32;
        let block_size = self.block_size;
        if width == 0 || height == 0 || block_size == 0 {
            return;
        }
        let num_blocks_x = width / block_size;
        let num_blocks_y = height / block_size;
        if num_blocks_x == 0 || num_blocks_y == 0 {
            return;
        }

        for swap in &self.swaps {
            let (src_bx, src_by) = swap.src;
            let (dst_bx, dst_by) = swap.dst;
            if src_bx >= num_blocks_x
                || dst_bx >= num_blocks_x
                || src_by >= num_blocks_y
                || dst_by >= num_blocks_y
                || (src_bx == dst_bx && src_by == dst_by)
            {
                continue;
            }

            for y_off in 0..block_size {
                let src_y = src_by * block_size + y_off;
                let dst_y = dst_by * block_size + y_off;
                if src_y >= height || dst_y >= height {
                    continue;
                }
                let columns = (0..block_size).filter_map(|x_off| {
                    let src_x = (src_bx * block_size + x_off) as usize;
                    let dst_x = (dst_bx * block_size + x_off) as usize;
                    (src_x < width as usize && dst_x < width as usize).then_some((src_x, dst_x))
                });

                if src_y == dst_y {
                    let Some(mut line) = lines.get_mut(src_y as usize) else {
                        continue;
                    };
                    for (src_x, dst_x) in columns {
                        if let (Some(ps), Some(pd)) = (line.get_pixel(src_x), line.get_pixel(dst_x))
                        {
                            line.set_pixel(src_x, pd);
                            line.set_pixel(dst_x, ps);
                        }
                    }
                } else {
                    let Some((mut src_line, mut dst_line)) =
                        lines.pair_mut(src_y as usize, dst_y as usize)
                    else {
                        continue;
                    };
                    for (src_x, dst_x) in columns {
                        if let (Some(ps), Some(pd)) =
                            (src_line.get_pixel(src_x), dst_line.get_pixel(dst_x))
                        {
                            src_line.set_pixel(src_x, pd);
                            dst_line.set_pixel(dst_x, ps);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, codecs::png::PngEncoder};

    fn sample() -> PngGlitch {
        let (w, h) = (8u32, 8u32);
        let data: Vec<u8> = (0..w * h * 3).map(|i| (i * 7 % 251) as u8).collect();
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(&data, w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        PngGlitch::from_bytes(out).unwrap()
    }

    fn bytes_after(effect: impl Effect) -> Vec<u8> {
        let mut png = sample();
        png.apply(effect);
        png.data().to_vec()
    }

    #[test]
    fn effects_are_deterministic_and_change_selected_rows_only() {
        let original = sample().data().to_vec();
        let stride = original.len() / 8;
        let out = bytes_after(Bitwise::new(BitOp::Xor, 0xFF, vec![2]));
        assert_eq!(bytes_after(Bitwise::new(BitOp::Xor, 0xFF, vec![2])), out);
        for row in 0..8 {
            let range = row * stride..(row + 1) * stride;
            if row == 2 {
                assert_ne!(original[range.clone()], out[range]);
            } else {
                assert_eq!(original[range.clone()], out[range]);
            }
        }
    }

    #[test]
    fn empty_parameters_are_no_ops() {
        let original = sample().data().to_vec();
        assert_eq!(bytes_after(SwapLines::default()), original);
        assert_eq!(bytes_after(CopyLines::default()), original);
        assert_eq!(bytes_after(SetBytes::default()), original);
        assert_eq!(bytes_after(ShiftLines::default()), original);
        assert_eq!(bytes_after(PixelSort::default()), original);
        assert_eq!(bytes_after(SwapBlocks::default()), original);
        assert_eq!(bytes_after(OffsetColors::default()), original);
    }

    #[test]
    fn out_of_range_parameters_are_ignored() {
        let original = sample().data().to_vec();
        assert_eq!(
            bytes_after(SwapLines::new(vec![(0, 99), (99, 1)])),
            original
        );
        assert_eq!(bytes_after(CopyLines::new(vec![(0, 99)])), original);
        assert_eq!(
            bytes_after(SetBytes::new(vec![ByteEdit {
                line: 99,
                index: 0,
                value: 1
            }])),
            original
        );
        assert_eq!(bytes_after(Bitwise::new(BitOp::Xor, 1, vec![99])), original);
        let swap = BlockSwap {
            src: (0, 0),
            dst: (9, 9),
        };
        assert_eq!(bytes_after(SwapBlocks::new(4, vec![swap])), original);
    }

    #[test]
    fn swap_lines_exchanges_rows() {
        let original = sample().data().to_vec();
        let stride = original.len() / 8;
        let out = bytes_after(SwapLines::new(vec![(0, 7)]));
        assert_eq!(out[..stride], original[7 * stride..]);
        assert_eq!(out[7 * stride..], original[..stride]);
    }

    #[test]
    fn set_filter_types_overwrites_only_the_tag() {
        let original = sample().data().to_vec();
        let stride = original.len() / 8;
        let out = bytes_after(SetFilterTypes::new(vec![None, Some(FilterType::Paeth)]));
        assert_eq!(out[..stride], original[..stride]);
        assert_eq!(out[stride], 4);
        assert_eq!(out[stride + 1..], original[stride + 1..]);
    }
}

#[cfg(test)]
mod oracle_tests {
    //! Each ported effect must give the same bytes as its pre-port implementation in
    //! `reference`, on random images of every colour type and bit depth, with random parameters
    //! and out-of-range rows.

    use super::*;
    use proptest::prelude::*;

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
    }

    fn build(width: u32, height: u32, color_type: u8, bit_depth: u8, raw: &[u8]) -> Vec<u8> {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[bit_depth, color_type, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        if color_type == 3 {
            chunk(&mut png, b"PLTE", &[0u8; 768]);
        }
        let mut compressor = fdeflate::Compressor::new(Vec::new()).unwrap();
        compressor.write_data(raw).unwrap();
        chunk(&mut png, b"IDAT", &compressor.finish().unwrap());
        chunk(&mut png, b"IEND", &[]);
        png
    }

    const FORMATS: [(u8, u8); 15] = [
        (0, 1),
        (0, 2),
        (0, 4),
        (0, 8),
        (0, 16),
        (2, 8),
        (2, 16),
        (3, 1),
        (3, 2),
        (3, 4),
        (3, 8),
        (4, 8),
        (4, 16),
        (6, 8),
        (6, 16),
    ];

    fn image() -> impl Strategy<Value = Vec<u8>> {
        (0..FORMATS.len(), 1u32..24, 1u32..24).prop_flat_map(|(f, w, h)| {
            let (color_type, bit_depth) = FORMATS[f];
            let channels = match color_type {
                0 | 3 => 1,
                2 => 3,
                4 => 2,
                _ => 4,
            };
            let row = (w as usize * channels * bit_depth as usize).div_ceil(8) + 1;
            proptest::collection::vec(any::<u8>(), row * h as usize).prop_map(move |mut raw| {
                for tag in raw.chunks_exact_mut(row).map(|r| &mut r[0]) {
                    *tag %= 5;
                }
                build(w, h, color_type, bit_depth, &raw)
            })
        })
    }

    fn rows() -> impl Strategy<Value = Vec<u32>> {
        proptest::collection::vec(0u32..30, 0..8)
    }

    fn pairs() -> impl Strategy<Value = Vec<(u32, u32)>> {
        proptest::collection::vec((0u32..30, 0u32..30), 0..8)
    }

    fn filter_type() -> impl Strategy<Value = FilterType> {
        prop_oneof![
            Just(FilterType::None),
            Just(FilterType::Sub),
            Just(FilterType::Up),
            Just(FilterType::Average),
            Just(FilterType::Paeth),
        ]
    }

    /// Applies `effect` through `PngGlitch::apply` and through a `ScanLines` view, and
    /// `reference` to a third copy; all three must agree.
    fn check<E: ScanLinesEffect>(
        file: Vec<u8>,
        effect: E,
        reference: impl Fn(&E, &mut PngGlitch),
    ) -> Result<(), TestCaseError> {
        let parse = || PngGlitch::from_bytes(file.clone()).unwrap();
        let mut expected = parse();
        reference(&effect, &mut expected);
        let mut applied = parse();
        Effect::apply_to(&effect, &mut applied);
        let mut viewed = parse();
        effect.apply_to_scan_lines(&mut viewed.scan_lines_mut());
        prop_assert_eq!(applied.data(), expected.data());
        prop_assert_eq!(viewed.data(), expected.data());
        Ok(())
    }

    proptest! {
        #[test]
        fn set_filter_types(png in image(), types in proptest::collection::vec(proptest::option::of(filter_type()), 0..30)) {
            check(png, SetFilterTypes::new(types), reference::set_filter_types)?;
        }

        #[test]
        fn set_bytes(png in image(), edits in proptest::collection::vec((0u32..30, 0usize..200, any::<u8>()), 0..10)) {
            let edits = edits.into_iter().map(|(line, index, value)| ByteEdit { line, index, value }).collect();
            check(png, SetBytes::new(edits), reference::set_bytes)?;
        }

        #[test]
        fn substitute(png in image(), index in 0usize..200, value in any::<u8>()) {
            check(png, Substitute::new(index, value), reference::substitute)?;
        }

        #[test]
        fn swap_lines(png in image(), swaps in pairs()) {
            check(png, SwapLines::new(swaps), reference::swap_lines)?;
        }

        #[test]
        fn copy_lines(png in image(), copies in pairs()) {
            check(png, CopyLines::new(copies), reference::copy_lines)?;
        }

        #[test]
        fn shift_lines(png in image(), shifts in proptest::collection::vec((0u32..30, 0usize..400), 0..8)) {
            let shifts = shifts.into_iter().map(|(line, shift)| LineShift { line, shift }).collect();
            check(png, ShiftLines::new(shifts), reference::shift_lines)?;
        }

        #[test]
        fn pixel_sort(png in image(), hue in any::<bool>(), rows in rows()) {
            let criterion = if hue { SortCriterion::Hue } else { SortCriterion::Brightness };
            check(png, PixelSort::new(criterion, rows), reference::pixel_sort)?;
        }

        /// Rows wide enough for several merge passes (`SORT_RUN` is 16), with few distinct byte
        /// values so that many pixels tie and the stability shows.
        #[test]
        fn pixel_sort_wide_rows_with_ties(
            (f, w, raw) in (0..FORMATS.len(), 1u32..300).prop_flat_map(|(f, w)| {
                let (color_type, bit_depth) = FORMATS[f];
                let channels = match color_type { 0 | 3 => 1, 2 => 3, 4 => 2, _ => 4 };
                let row = (w as usize * channels * bit_depth as usize).div_ceil(8) + 1;
                (Just(f), Just(w), proptest::collection::vec(0u8..4, row * 3))
            }),
            hue in any::<bool>(),
        ) {
            let (color_type, bit_depth) = FORMATS[f];
            let criterion = if hue { SortCriterion::Hue } else { SortCriterion::Brightness };
            let png = build(w, 3, color_type, bit_depth, &raw);
            check(png, PixelSort::new(criterion, vec![0, 1, 2, 2, 7]), reference::pixel_sort)?;
        }

        #[test]
        fn bitwise(png in image(), op in 0..3usize, value in any::<u8>(), rows in rows()) {
            let op = [BitOp::And, BitOp::Or, BitOp::Xor][op];
            check(png, Bitwise::new(op, value, rows), reference::bitwise)?;
        }

        #[test]
        fn swap_channels(png in image(), target in 0..3usize, rows in rows()) {
            let target = [SwapTarget::Rg, SwapTarget::Gb, SwapTarget::Br][target];
            check(png, SwapChannels::new(target, rows), reference::swap_channels)?;
        }

        #[test]
        fn offset_colors(
            png in image(),
            lines in proptest::collection::vec(
                (0u32..30, proptest::collection::vec(any::<[i16; 3]>(), 0..30)), 0..6),
        ) {
            let lines = lines.into_iter().map(|(line, offsets)| LineOffsets { line, offsets }).collect();
            check(png, OffsetColors::new(lines), reference::offset_colors)?;
        }

        #[test]
        fn hsl_shift(png in image(), hue in -720.0f64..720.0, sat in 0.0f64..3.0, light in 0.0f64..3.0, rows in rows()) {
            check(png, HslShift::new(hue, sat, light, rows), reference::hsl_shift)?;
        }

        #[test]
        fn channel_shift(png in image(), r in -40i32..40, g in -40i32..40, b in -40i32..40, rows in rows()) {
            check(png, ChannelShift::new(r, g, b, rows), reference::channel_shift)?;
        }

        #[test]
        fn swap_blocks(
            png in image(),
            block_size in 0u32..6,
            swaps in proptest::collection::vec(((0u32..6, 0u32..6), (0u32..6, 0u32..6)), 0..8),
        ) {
            let swaps = swaps.into_iter().map(|(src, dst)| BlockSwap { src, dst }).collect();
            check(png, SwapBlocks::new(block_size, swaps), reference::swap_blocks)?;
        }
    }
}

#[cfg(test)]
mod stable_sort_tests {
    use super::stable_sort_by_key;

    #[test]
    fn stable_sort_equals_sort_by_with_ties() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for len in 0..=300 {
            for keys in [1, 3, 50] {
                // (key, original position): the position shows whether ties kept their order.
                let v: Vec<(u64, usize)> = (0..len).map(|i| (next() % keys, i)).collect();
                let mut expected = v.clone();
                expected.sort_by_key(|a| a.0); // stable
                let mut got = v.clone();
                let mut aux = vec![(0, 0); len];
                stable_sort_by_key(&mut got, &mut aux, |a| a.0);
                assert_eq!(got, expected, "len {len}, keys {keys}");
            }
        }
    }
}
