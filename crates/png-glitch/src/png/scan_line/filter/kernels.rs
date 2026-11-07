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
