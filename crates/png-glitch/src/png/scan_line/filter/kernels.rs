//! Slice-based PNG filter kernels (#97).
//!
//! Every function works on the row bytes **without** the filter-type byte. `prev` is the
//! previous row's *unfiltered* bytes (`None` for the first row, which behaves as an all-zero
//! row). The kernels are specialised per filter type and, through a const generic, per
//! bytes-per-pixel, and walk pixels as `[u8; BPP]` arrays with the left (and top-left) pixel
//! carried in a local. There is no per-byte bounds check and no `unsafe`; the loops are shaped
//! so that LLVM can unroll and vectorise them (with `-C target-feature=+simd128` on wasm).
//!
//! All arithmetic is modulo 256 (`wrapping_*`), identical to the PNG specification and to the
//! original per-byte code in `reference.rs`, including for arbitrary ("glitched") input.

use crate::FilterType;

/// Picks the const-generic instantiation once per row. Valid PNG headers only give 1, 2, 3, 4,
/// 6 or 8 bytes per pixel; anything else (or a row that is not a whole number of pixels) takes
/// the slower generic loop.
macro_rules! by_bpp {
    ($bpp:expr, $cur:expr, $prev:expr, $kernel:ident, $kernel4:ident, $generic:ident) => {{
        let (bpp, cur, prev): (usize, &mut [u8], Option<&[u8]>) = ($bpp, $cur, $prev);
        if bpp == 0 || cur.len() % bpp != 0 {
            $generic(bpp.max(1), cur, prev)
        } else {
            match bpp {
                1 => $kernel::<1>(cur, prev),
                2 => $kernel::<2>(cur, prev),
                3 => $kernel::<3>(cur, prev),
                4 => $kernel4(cur, prev),
                6 => $kernel::<6>(cur, prev),
                8 => $kernel::<8>(cur, prev),
                _ => $generic(bpp, cur, prev),
            }
        }
    }};
}

/// Reverses `ft` in place: turns filtered bytes into raw bytes.
pub fn unfilter(ft: FilterType, bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    match ft {
        FilterType::None => {}
        FilterType::Sub => by_bpp!(
            bpp,
            cur,
            prev,
            sub_unfilter_n,
            sub_unfilter_4,
            generic_sub_unfilter
        ),
        FilterType::Up => up_unfilter(cur, prev),
        FilterType::Average => {
            by_bpp!(
                bpp,
                cur,
                prev,
                average_unfilter_n,
                average_unfilter_4,
                generic_average_unfilter
            )
        }
        FilterType::Paeth => by_bpp!(
            bpp,
            cur,
            prev,
            paeth_unfilter_n,
            paeth_unfilter_4,
            generic_paeth_unfilter
        ),
    }
}

/// Applies `ft` in place: turns raw bytes into filtered bytes.
pub fn filter(ft: FilterType, bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    match ft {
        FilterType::None => {}
        FilterType::Sub => by_bpp!(
            bpp,
            cur,
            prev,
            sub_filter_n,
            sub_filter_4,
            generic_sub_filter
        ),
        FilterType::Up => up_filter(cur, prev),
        FilterType::Average => by_bpp!(
            bpp,
            cur,
            prev,
            average_filter_n,
            average_filter_4,
            generic_average_filter
        ),
        FilterType::Paeth => by_bpp!(
            bpp,
            cur,
            prev,
            paeth_filter_n,
            paeth_filter_4,
            generic_paeth_filter
        ),
    }
}

// ---- Heuristic cost (for adaptive filter selection) -------------------------------------------

/// libpng's "minimum sum of absolute differences" cost of filtering `cur` with `ft`: the sum of
/// `|filtered byte as i8|`. Nothing is written and nothing is allocated, so a caller can compare
/// all five filter types for a row without a scratch row.
pub fn filter_cost(ft: FilterType, bpp: usize, cur: &[u8], prev: Option<&[u8]>) -> u64 {
    match ft {
        FilterType::None => cost_by(bpp, cur, prev, |_, _, _| 0),
        FilterType::Sub => cost_by(bpp, cur, prev, |left, _, _| left),
        FilterType::Up => cost_by(bpp, cur, prev, |_, top, _| top),
        FilterType::Average => cost_by(bpp, cur, prev, |left, top, _| average(left, top)),
        FilterType::Paeth => cost_by(bpp, cur, prev, paeth),
    }
}

/// Bytes per block of the adaptive cost loops: small enough that a block sum fits a `u32`
/// (`256 * 128`) and the early exit is checked often, large enough to vectorise.
const COST_BLOCK: usize = 256;

#[inline(always)]
fn abs_cost(c: u8, predicted: u8) -> u32 {
    u32::from((c.wrapping_sub(predicted) as i8).unsigned_abs())
}

/// Bytes per lane group of the adaptive cost loops: eight 16-bit lanes are exactly one 128-bit
/// vector (SSE2, NEON, wasm `simd128`). Measured at 1080p RGBA: the Paeth cost went from 23 ms to
/// 4 ms with 8 lanes, and stayed at 20 ms with 16 or 32.
const LANES: usize = 8;

/// Sum of `|c - predict(left, top, top_left) as i8|` over equally long slices, as fixed-size
/// lane groups plus a scalar tail, so that LLVM vectorises the groups.
#[inline(always)]
fn lane_cost(
    cur: &[u8],
    left: &[u8],
    top: &[u8],
    top_left: &[u8],
    predict: impl Fn(u8, u8, u8) -> u8,
) -> u32 {
    let (cur, cur_tail) = cur.as_chunks::<LANES>();
    let (left, left_tail) = left.as_chunks::<LANES>();
    let (top, top_tail) = top.as_chunks::<LANES>();
    let (top_left, top_left_tail) = top_left.as_chunks::<LANES>();
    let mut sum = 0u32;
    for (((c, l), t), tl) in cur.iter().zip(left).zip(top).zip(top_left) {
        let mut lanes = 0u32;
        for (((&c, &l), &t), &tl) in c.iter().zip(l).zip(t).zip(tl) {
            lanes += abs_cost(c, predict(l, t, tl));
        }
        sum += lanes;
    }
    let tails = cur_tail
        .iter()
        .zip(left_tail)
        .zip(top_tail)
        .zip(top_left_tail);
    for (((&c, &l), &t), &tl) in tails {
        sum += abs_cost(c, predict(l, t, tl));
    }
    sum
}

/// The same choice as taking the minimum of [`filter_cost`] over `None, Sub, Up, Average, Paeth`
/// in that order and keeping the first minimum (libpng's "minimum sum of absolute differences",
/// ties to the lowest type), computed faster and with the same result for every input:
///
/// 1. the sums are taken over eight-byte lane groups, which LLVM vectorises (see [`LANES`]);
/// 2. `Average` and `Paeth` stop as soon as a block ends with a partial sum that is already at
///    least the best complete sum so far: a candidate that reaches the best cost cannot be
///    chosen (a tie goes to the lower type), so the choice does not change.
///
/// Returns `None` when the fast path does not apply (no row above, or rows of different
/// lengths, or shorter than one pixel); the caller then compares [`filter_cost`] instead.
pub fn adaptive_filter(bpp: usize, cur: &[u8], prev: Option<&[u8]>) -> Option<FilterType> {
    let row = AdaptiveRow::new(bpp, cur, prev?)?;
    let (none, sub, up) = row.cost_none_sub_up();
    let mut best = (none, FilterType::None);
    if sub < best.0 {
        best = (sub, FilterType::Sub);
    }
    if up < best.0 {
        best = (up, FilterType::Up);
    }
    // Average and Paeth, with the early exit.
    let avg = row.cost(best.0, |l, t, _| average(l, t));
    if avg < best.0 {
        best = (avg, FilterType::Average);
    }
    let paeth = row.cost(best.0, paeth16);
    if paeth < best.0 {
        best = (paeth, FilterType::Paeth);
    }
    Some(best.1)
}

/// A row and the row above, split so that the left neighbour of byte `i` of `cur_rest` is byte
/// `i` of `cur_left` (the same shift by `bpp` for the top-left byte).
struct AdaptiveRow<'a> {
    cur_first: &'a [u8],
    top_first: &'a [u8],
    cur_rest: &'a [u8],
    cur_left: &'a [u8],
    top_rest: &'a [u8],
    top_left: &'a [u8],
}

impl<'a> AdaptiveRow<'a> {
    fn new(bpp: usize, cur: &'a [u8], prev: &'a [u8]) -> Option<Self> {
        if prev.len() != cur.len() {
            return None;
        }
        let (cur_first, cur_rest) = cur.split_at_checked(bpp)?;
        let (top_first, top_rest) = prev.split_at_checked(bpp)?;
        let shifted = cur.len() - bpp;
        Some(Self {
            cur_first,
            top_first,
            cur_rest,
            cur_left: cur.get(..shifted)?,
            top_rest,
            top_left: prev.get(..shifted)?,
        })
    }

    /// The costs of `None`, `Sub` and `Up` in one pass: they share the loads of `cur`, its left
    /// neighbour and the row above.
    fn cost_none_sub_up(&self) -> (u64, u64, u64) {
        let (mut none, mut sub, mut up) = (0u64, 0u64, 0u64);
        // The first pixel: `left = 0`, so `Sub` equals `None`.
        for (&c, &t) in self.cur_first.iter().zip(self.top_first) {
            none += u64::from(abs_cost(c, 0));
            up += u64::from(abs_cost(c, t));
        }
        sub += none;
        for ((c, l), t) in self
            .cur_rest
            .chunks(COST_BLOCK)
            .zip(self.cur_left.chunks(COST_BLOCK))
            .zip(self.top_rest.chunks(COST_BLOCK))
        {
            let (c8, c_tail) = c.as_chunks::<LANES>();
            let (l8, l_tail) = l.as_chunks::<LANES>();
            let (t8, t_tail) = t.as_chunks::<LANES>();
            let (mut n, mut s, mut u) = (0u32, 0u32, 0u32);
            for ((c, l), t) in c8.iter().zip(l8).zip(t8) {
                let (mut ln, mut ls, mut lu) = (0u32, 0u32, 0u32);
                for ((&c, &l), &t) in c.iter().zip(l).zip(t) {
                    ln += abs_cost(c, 0);
                    ls += abs_cost(c, l);
                    lu += abs_cost(c, t);
                }
                n += ln;
                s += ls;
                u += lu;
            }
            for ((&c, &l), &t) in c_tail.iter().zip(l_tail).zip(t_tail) {
                n += abs_cost(c, 0);
                s += abs_cost(c, l);
                u += abs_cost(c, t);
            }
            none += u64::from(n);
            sub += u64::from(s);
            up += u64::from(u);
        }
        (none, sub, up)
    }

    /// The cost of filtering the row with `predict`. Once a block ends with a partial sum of at
    /// least `limit` the sum stops growing there, and the value is only known to be `>= limit`.
    /// `predict` is generic so that it inlines into the lane loop.
    #[inline(always)]
    fn cost(&self, limit: u64, predict: impl Fn(u8, u8, u8) -> u8 + Copy) -> u64 {
        // The first pixel has no left neighbour (`left = top_left = 0`).
        let mut total = 0u64;
        for (&c, &t) in self.cur_first.iter().zip(self.top_first) {
            total += u64::from(abs_cost(c, predict(0, t, 0)));
        }
        for (((c, l), t), tl) in self
            .cur_rest
            .chunks(COST_BLOCK)
            .zip(self.cur_left.chunks(COST_BLOCK))
            .zip(self.top_rest.chunks(COST_BLOCK))
            .zip(self.top_left.chunks(COST_BLOCK))
        {
            if total >= limit {
                return total;
            }
            total += u64::from(lane_cost(c, l, t, tl, predict));
        }
        total
    }
}

fn cost_by(bpp: usize, cur: &[u8], prev: Option<&[u8]>, predict: impl Fn(u8, u8, u8) -> u8) -> u64 {
    if let Some(prev) = prev
        && let Some(sum) = cost_flat(bpp, cur, prev, &predict)
    {
        return sum;
    }
    // First row (no row above) or rows of different lengths: the simple iterator form.
    let lefts = std::iter::repeat_n(0u8, bpp).chain(cur.iter().copied());
    match prev {
        Some(prev) => cost_sum(
            cur,
            lefts,
            prev.iter().copied(),
            std::iter::repeat_n(0u8, bpp).chain(prev.iter().copied()),
            predict,
        ),
        None => cost_sum(
            cur,
            lefts,
            std::iter::repeat(0u8),
            std::iter::repeat(0u8),
            predict,
        ),
    }
}

#[inline(always)]
fn cost_of(c: u8, predicted: u8) -> u64 {
    u64::from((c.wrapping_sub(predicted) as i8).unsigned_abs())
}

/// The same cost as `cost_sum`, as flat zips over shifted slices (left = `cur` shifted by `bpp`,
/// top-left = `prev` shifted by `bpp`) so LLVM can vectorise it. `None` if the rows do not have
/// the same length or are shorter than one pixel.
fn cost_flat(
    bpp: usize,
    cur: &[u8],
    prev: &[u8],
    predict: &impl Fn(u8, u8, u8) -> u8,
) -> Option<u64> {
    if prev.len() != cur.len() {
        return None;
    }
    let (cur_first, cur_rest) = cur.split_at_checked(bpp)?;
    let (top_first, top_rest) = prev.split_at_checked(bpp)?;
    let shifted = cur.len() - bpp;
    let (cur_left, top_left) = (cur.get(..shifted)?, prev.get(..shifted)?);

    let first: u64 = cur_first
        .iter()
        .zip(top_first)
        .map(|(&c, &t)| cost_of(c, predict(0, t, 0)))
        .sum();
    let rest: u64 = cur_rest
        .iter()
        .zip(cur_left)
        .zip(top_rest)
        .zip(top_left)
        .map(|(((&c, &l), &t), &tl)| cost_of(c, predict(l, t, tl)))
        .sum();
    Some(first + rest)
}

fn cost_sum(
    cur: &[u8],
    lefts: impl Iterator<Item = u8>,
    tops: impl Iterator<Item = u8>,
    top_lefts: impl Iterator<Item = u8>,
    predict: impl Fn(u8, u8, u8) -> u8,
) -> u64 {
    let mut sum = 0u64;
    for (((c, l), t), tl) in cur.iter().zip(lefts).zip(tops).zip(top_lefts) {
        let filtered = c.wrapping_sub(predict(l, t, tl)) as i8;
        sum += u64::from(filtered.unsigned_abs());
    }
    sum
}

// ---- Up -------------------------------------------------------------------------------------

fn up_unfilter(cur: &mut [u8], prev: Option<&[u8]>) {
    if let Some(prev) = prev {
        for (c, p) in cur.iter_mut().zip(prev) {
            *c = c.wrapping_add(*p);
        }
    }
}

fn up_filter(cur: &mut [u8], prev: Option<&[u8]>) {
    if let Some(prev) = prev {
        for (c, p) in cur.iter_mut().zip(prev) {
            *c = c.wrapping_sub(*p);
        }
    }
}

// ---- Sub ------------------------------------------------------------------------------------

fn sub_unfilter_n<const BPP: usize>(cur: &mut [u8], _prev: Option<&[u8]>) {
    let (pixels, _) = cur.as_chunks_mut::<BPP>();
    let mut left = [0u8; BPP];
    for px in pixels {
        for (c, l) in px.iter_mut().zip(&left) {
            *c = c.wrapping_add(*l);
        }
        left = *px;
    }
}

fn sub_filter_n<const BPP: usize>(cur: &mut [u8], _prev: Option<&[u8]>) {
    let (pixels, _) = cur.as_chunks_mut::<BPP>();
    let mut left = [0u8; BPP];
    for px in pixels {
        let original = *px;
        for (c, l) in px.iter_mut().zip(&left) {
            *c = c.wrapping_sub(*l);
        }
        left = original;
    }
}

// ---- Average --------------------------------------------------------------------------------

#[inline(always)]
fn average(left: u8, top: u8) -> u8 {
    ((u16::from(left) + u16::from(top)) >> 1) as u8
}

fn average_unfilter_n<const BPP: usize>(cur: &mut [u8], prev: Option<&[u8]>) {
    let (pixels, _) = cur.as_chunks_mut::<BPP>();
    let mut left = [0u8; BPP];
    match prev {
        // First row: the row above is all zero, so the predictor is `left / 2`.
        None => {
            for px in pixels {
                for (c, l) in px.iter_mut().zip(&left) {
                    *c = c.wrapping_add(l >> 1);
                }
                left = *px;
            }
        }
        Some(prev) => {
            let (tops, _) = prev.as_chunks::<BPP>();
            for (px, top) in pixels.iter_mut().zip(tops) {
                for ((c, l), t) in px.iter_mut().zip(&left).zip(top) {
                    *c = c.wrapping_add(average(*l, *t));
                }
                left = *px;
            }
        }
    }
}

fn average_filter_n<const BPP: usize>(cur: &mut [u8], prev: Option<&[u8]>) {
    let (pixels, _) = cur.as_chunks_mut::<BPP>();
    let mut left = [0u8; BPP];
    match prev {
        None => {
            for px in pixels {
                let original = *px;
                for (c, l) in px.iter_mut().zip(&left) {
                    *c = c.wrapping_sub(l >> 1);
                }
                left = original;
            }
        }
        Some(prev) => {
            let (tops, _) = prev.as_chunks::<BPP>();
            for (px, top) in pixels.iter_mut().zip(tops) {
                let original = *px;
                for ((c, l), t) in px.iter_mut().zip(&left).zip(top) {
                    *c = c.wrapping_sub(average(*l, *t));
                }
                left = original;
            }
        }
    }
}

// ---- Paeth ----------------------------------------------------------------------------------

/// Arithmetic type of the Paeth predictor when reconstructing. `a + b - 2c` needs 10 bits, so
/// `i16` is enough; wasm measured faster with 32-bit lanes for reconstruction (four of them fill
/// a `v128`), native with 16-bit lanes. Filtering always uses `i16` (see [`paeth_filter_n`]).
#[cfg(target_arch = "wasm32")]
type Lane = i32;
#[cfg(not(target_arch = "wasm32"))]
type Lane = i16;

/// Defines a Paeth predictor `$name(a, b, c)` computed in `$lane` arithmetic. The comparisons are
/// written as selects (no data-dependent branches) so LLVM can lower them to `select` /
/// `bitselect`.
macro_rules! paeth_predictor {
    ($name:ident, $lane:ty) => {
        #[inline(always)]
        fn $name(a: $lane, b: $lane, c: $lane) -> $lane {
            let pa = (b - c).abs(); // |p - a| with p = a + b - c
            let pb = (a - c).abs(); // |p - b|
            let pc = (a + b - c - c).abs(); // |p - c|
            // Sequential minimum with ties going to the earlier candidate (a, then b, then c),
            // which is the order the PNG specification requires.
            let (best, pr) = if pb < pa { (pb, b) } else { (pa, a) };
            if pc < best { c } else { pr }
        }
    };
}
paeth_predictor!(paeth_lane, Lane);
paeth_predictor!(paeth_i16, i16);
paeth_predictor!(paeth_i32, i32);

/// The Paeth predictor for reconstruction.
#[inline(always)]
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    paeth_lane(Lane::from(a), Lane::from(b), Lane::from(c)) as u8
}

/// The Paeth predictor for filtering. 16-bit lanes put eight bytes in a `v128`; with `simd128`
/// this was 4x faster than 32-bit lanes for the flat filter loop.
#[inline(always)]
fn paeth16(a: u8, b: u8, c: u8) -> u8 {
    paeth_i16(i16::from(a), i16::from(b), i16::from(c)) as u8
}

fn paeth_unfilter_n<const BPP: usize>(cur: &mut [u8], prev: Option<&[u8]>) {
    match prev {
        // First row: with `b = c = 0` the predictor is `a`, so Paeth degenerates to Sub.
        None => sub_unfilter_n::<BPP>(cur, None),
        Some(prev) => {
            let (pixels, _) = cur.as_chunks_mut::<BPP>();
            let (tops, _) = prev.as_chunks::<BPP>();
            let mut left = [0u8; BPP];
            let mut top_left = [0u8; BPP];
            for (px, top) in pixels.iter_mut().zip(tops) {
                for (((c, l), t), tl) in px.iter_mut().zip(&left).zip(top).zip(&top_left) {
                    *c = c.wrapping_add(paeth(*l, *t, *tl));
                }
                left = *px;
                top_left = *top;
            }
        }
    }
}

/// Bytes per block in [`paeth_filter_n`]. A multiple of every specialised pixel size, small
/// enough for the two scratch arrays to stay on the stack.
const PAETH_BLOCK: usize = 384;

/// Paeth filtering. The predictor only reads *original* bytes, so unlike reconstruction there is
/// no dependency along the row. The row is processed in blocks: the left and top-left
/// neighbours of a block are copied into stack arrays first, and then one flat loop over the
/// block computes every byte, which LLVM vectorises along the row (16 bytes at a time with
/// `simd128`). No allocation.
fn paeth_filter_n<const BPP: usize>(cur: &mut [u8], prev: Option<&[u8]>) {
    let Some(prev) = prev else {
        return sub_filter_n::<BPP>(cur, None);
    };
    if prev.len() != cur.len() {
        // Not reachable through `filter::apply`, which pads the previous row.
        return paeth_filter_pixels::<BPP>(cur, prev);
    }
    // Original bytes of the last pixel before the current block, in the current and the
    // previous row.
    let mut left = [0u8; BPP];
    let mut top_left = [0u8; BPP];
    // `lefts[k]` / `top_lefts[k]` are the bytes `BPP` positions before byte `k` of the block.
    let mut lefts = [0u8; PAETH_BLOCK];
    let mut top_lefts = [0u8; PAETH_BLOCK];
    for (block, top) in cur.chunks_mut(PAETH_BLOCK).zip(prev.chunks(PAETH_BLOCK)) {
        // `cur.len()` is a whole number of pixels and `PAETH_BLOCK` a multiple of `BPP`, so
        // every block holds at least one pixel.
        let Some(body_len) = block.len().checked_sub(BPP) else {
            break;
        };
        let (body, last) = block.split_at(body_len);
        let (top_body, top_last) = top.split_at(body_len);
        let (lefts_head, lefts_tail) = lefts.split_at_mut(BPP);
        let (top_lefts_head, top_lefts_tail) = top_lefts.split_at_mut(BPP);
        copy(lefts_head, &left);
        copy(lefts_tail, body);
        copy(top_lefts_head, &top_left);
        copy(top_lefts_tail, top_body);
        copy(&mut left, last);
        copy(&mut top_left, top_last);
        for (((c, l), t), tl) in block.iter_mut().zip(&lefts).zip(top).zip(&top_lefts) {
            *c = c.wrapping_sub(paeth16(*l, *t, *tl));
        }
    }
}

/// Copies as many bytes as both slices hold (no length assertion, unlike `copy_from_slice`).
#[inline(always)]
fn copy(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = *s;
    }
}

/// Per-pixel Paeth filtering for a previous row of another length: pixels past the shorter
/// row are left unchanged, as in the other kernels.
fn paeth_filter_pixels<const BPP: usize>(cur: &mut [u8], prev: &[u8]) {
    let (pixels, _) = cur.as_chunks_mut::<BPP>();
    let (tops, _) = prev.as_chunks::<BPP>();
    let mut left = [0u8; BPP];
    let mut top_left = [0u8; BPP];
    for (px, top) in pixels.iter_mut().zip(tops) {
        let original = *px;
        for (((c, l), t), tl) in px.iter_mut().zip(&left).zip(top).zip(&top_left) {
            *c = c.wrapping_sub(paeth(*l, *t, *tl));
        }
        left = original;
        top_left = *top;
    }
}

// ---- 4 bytes per pixel (RGBA 8-bit, the real-time case) ------------------------------------------
//
// A pixel is loaded and stored as one `u32`, and the four channels are processed with SWAR
// (SIMD within a register) byte arithmetic. In scalar wasm this replaces four byte-wide loads,
// ops and stores by one 32-bit load and store per pixel. The results are identical to the
// per-channel code; the differential tests cover it.

const LOW7: u32 = 0x7f7f_7f7f;
const HIGH: u32 = 0x8080_8080;

/// Lane-wise `x + y` modulo 256.
#[inline(always)]
fn swar_add(x: u32, y: u32) -> u32 {
    ((x & LOW7).wrapping_add(y & LOW7)) ^ ((x ^ y) & HIGH)
}

/// Lane-wise `x - y` modulo 256.
#[inline(always)]
fn swar_sub(x: u32, y: u32) -> u32 {
    ((x | HIGH).wrapping_sub(y & LOW7)) ^ (!(x ^ y) & HIGH)
}

/// Lane-wise `floor((x + y) / 2)`.
#[inline(always)]
fn swar_avg(x: u32, y: u32) -> u32 {
    (x & y).wrapping_add(((x ^ y) >> 1) & LOW7)
}

#[inline(always)]
fn load(px: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*px)
}

fn sub_unfilter_4(cur: &mut [u8], _prev: Option<&[u8]>) {
    let mut left = 0u32;
    for px in cur.as_chunks_mut::<4>().0 {
        left = swar_add(load(px), left);
        *px = left.to_le_bytes();
    }
}

fn sub_filter_4(cur: &mut [u8], _prev: Option<&[u8]>) {
    let mut left = 0u32;
    for px in cur.as_chunks_mut::<4>().0 {
        let original = load(px);
        *px = swar_sub(original, left).to_le_bytes();
        left = original;
    }
}

fn average_unfilter_4(cur: &mut [u8], prev: Option<&[u8]>) {
    let mut left = 0u32;
    let pixels = cur.as_chunks_mut::<4>().0;
    match prev {
        None => {
            for px in pixels {
                left = swar_add(load(px), (left >> 1) & LOW7);
                *px = left.to_le_bytes();
            }
        }
        Some(prev) => {
            for (px, top) in pixels.iter_mut().zip(prev.as_chunks::<4>().0) {
                left = swar_add(load(px), swar_avg(left, load(top)));
                *px = left.to_le_bytes();
            }
        }
    }
}

fn average_filter_4(cur: &mut [u8], prev: Option<&[u8]>) {
    let mut left = 0u32;
    let pixels = cur.as_chunks_mut::<4>().0;
    match prev {
        None => {
            for px in pixels {
                let original = load(px);
                *px = swar_sub(original, (left >> 1) & LOW7).to_le_bytes();
                left = original;
            }
        }
        Some(prev) => {
            for (px, top) in pixels.iter_mut().zip(prev.as_chunks::<4>().0) {
                let original = load(px);
                *px = swar_sub(original, swar_avg(left, load(top))).to_le_bytes();
                left = original;
            }
        }
    }
}

/// Paeth reconstruction for 4-byte pixels. On wasm this uses [`paeth_unfilter_4_lanes`]. Other
/// targets keep the per-channel loop: on x86-64, baseline SSE2 has no 32-bit `abs` / `min` and
/// the lane form measured several times slower; other native targets were not measured.
fn paeth_unfilter_4(cur: &mut [u8], prev: Option<&[u8]>) {
    if cfg!(target_arch = "wasm32") {
        paeth_unfilter_4_lanes(cur, prev)
    } else {
        paeth_unfilter_n::<4>(cur, prev)
    }
}

/// Paeth reconstruction for 4-byte pixels, the four channels of a pixel computed together. The
/// left-neighbour dependency rules out vectorising along the row, and the per-channel loop in
/// [`paeth_unfilter_n`] is not vectorised across channels either. Here every step is a
/// whole-array operation on `[i32; 4]`, which the SLP vectoriser turns into one `i32x4`
/// instruction with `simd128`. The left pixel is carried widened, so it is not re-extended for
/// every pixel. Compiled on every target so the native tests check it.
pub(super) fn paeth_unfilter_4_lanes(cur: &mut [u8], prev: Option<&[u8]>) {
    type Px = [i32; 4];
    #[inline(always)]
    fn widen(px: &[u8; 4]) -> Px {
        let mut out: Px = [0; 4];
        for (o, v) in out.iter_mut().zip(px) {
            *o = i32::from(*v);
        }
        out
    }

    let Some(prev) = prev else {
        return sub_unfilter_4(cur, None);
    };
    let mut left: Px = [0; 4];
    let mut top_left: Px = [0; 4];
    for (px, top) in cur
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(prev.as_chunks::<4>().0)
    {
        let top = widen(top);
        let mut raw: Px = [0; 4];
        for ((((r, x), a), b), c) in raw
            .iter_mut()
            .zip(px.iter())
            .zip(&left)
            .zip(&top)
            .zip(&top_left)
        {
            *r = (i32::from(*x) + paeth_i32(*a, *b, *c)) & 0xff;
        }
        *px = raw.map(|v| v as u8);
        left = raw;
        top_left = top;
    }
}

fn paeth_filter_4(cur: &mut [u8], prev: Option<&[u8]>) {
    paeth_filter_n::<4>(cur, prev)
}

// ---- Generic fallbacks (unreachable from valid headers) --------------------------------------
//
// These handle any pixel size and rows that are not a whole number of pixels. They allocate a
// few small buffers per row and are not meant to be fast.

/// Calls `f(byte, left, top, top_left)` for each byte, left to right, where `left` / `top_left`
/// are the *final* neighbours (raw values when unfiltering) or the *original* ones (when
/// filtering); `f` returns `(new_byte, value_to_remember_as_left)`.
fn generic_walk(
    bpp: usize,
    cur: &mut [u8],
    prev: Option<&[u8]>,
    mut f: impl FnMut(u8, u8, u8, u8) -> (u8, u8),
) {
    let mut left = vec![0u8; bpp];
    let mut top_left = vec![0u8; bpp];
    for (i, chunk) in cur.chunks_mut(bpp).enumerate() {
        let tops = prev
            .and_then(|p| p.chunks(bpp).nth(i))
            .unwrap_or(&[])
            .to_vec();
        let mut next_top_left = vec![0u8; bpp];
        for (k, byte) in chunk.iter_mut().enumerate() {
            let l = left.get(k).copied().unwrap_or(0);
            let t = tops.get(k).copied().unwrap_or(0);
            let tl = top_left.get(k).copied().unwrap_or(0);
            let (new, remember) = f(*byte, l, t, tl);
            *byte = new;
            if let Some(slot) = left.get_mut(k) {
                *slot = remember;
            }
            if let Some(slot) = next_top_left.get_mut(k) {
                *slot = t;
            }
        }
        top_left = next_top_left;
    }
}

fn generic_sub_unfilter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, _, _| {
        let v = c.wrapping_add(l);
        (v, v)
    });
}
fn generic_sub_filter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, _, _| (c.wrapping_sub(l), c));
}
fn generic_average_unfilter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, t, _| {
        let v = c.wrapping_add(average(l, t));
        (v, v)
    });
}
fn generic_average_filter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, t, _| {
        (c.wrapping_sub(average(l, t)), c)
    });
}
fn generic_paeth_unfilter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, t, tl| {
        let v = c.wrapping_add(paeth(l, t, tl));
        (v, v)
    });
}
fn generic_paeth_filter(bpp: usize, cur: &mut [u8], prev: Option<&[u8]>) {
    generic_walk(bpp, cur, prev, |c, l, t, tl| {
        (c.wrapping_sub(paeth(l, t, tl)), c)
    });
}

#[cfg(test)]
mod adaptive_tests {
    use super::*;

    const ORDER: [FilterType; 5] = [
        FilterType::None,
        FilterType::Sub,
        FilterType::Up,
        FilterType::Average,
        FilterType::Paeth,
    ];

    /// The oracle: the minimum of `filter_cost`, the first minimum winning.
    fn oracle(bpp: usize, cur: &[u8], prev: &[u8]) -> FilterType {
        let mut best = (u64::MAX, FilterType::None);
        for t in ORDER {
            let cost = filter_cost(t, bpp, cur, Some(prev));
            if cost < best.0 {
                best = (cost, t);
            }
        }
        best.1
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// Rows of several characters, so that every filter type wins sometimes and ties happen:
    /// noise, smooth gradients, constants, and a row equal to the one above.
    fn row(rng: &mut Rng, len: usize, kind: u64, above: &[u8]) -> Vec<u8> {
        match kind % 5 {
            0 => (0..len).map(|_| rng.next() as u8).collect(),
            1 => (0..len).map(|i| (i / 3) as u8).collect(),
            2 => vec![(rng.next() % 4) as u8 * 85; len],
            3 => above.to_vec(),
            _ => above
                .iter()
                .map(|&b| b.wrapping_add((rng.next() % 5) as u8).wrapping_sub(2))
                .collect(),
        }
    }

    #[test]
    fn the_fast_choice_equals_the_minimum_of_filter_cost() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for bpp in [1, 2, 3, 4, 6, 8] {
            for width in (1..=64).chain([100, 255, 256, 257, 1000]) {
                let len = width * bpp;
                for round in 0..12 {
                    let above: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
                    let cur = row(&mut rng, len, round, &above);
                    assert_eq!(
                        adaptive_filter(bpp, &cur, Some(&above)),
                        Some(oracle(bpp, &cur, &above)),
                        "bpp {bpp}, width {width}, round {round}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_fast_path_declines_what_it_cannot_do() {
        assert_eq!(adaptive_filter(4, &[1, 2, 3, 4], None), None);
        assert_eq!(adaptive_filter(4, &[1, 2, 3, 4], Some(&[1, 2, 3])), None);
        assert_eq!(adaptive_filter(4, &[1, 2], Some(&[1, 2])), None);
    }
}
