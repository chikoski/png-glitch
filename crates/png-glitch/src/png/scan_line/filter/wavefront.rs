//! Multi-row wavefront reconstruction (Paeth, Average) for 4-byte pixels (#152).
//!
//! Reconstructing a Paeth or Average row is one long dependency chain: every pixel needs its
//! reconstructed left neighbour, so the row can't be vectorised along its length, and a single
//! chain is bound by the latency of one predictor step. The row below only needs the pixel
//! *above* and *above left* of it, though. So rows `y` and `y + 1` are reconstructed together,
//! with row `y + 1` one pixel behind: step `i` computes pixel `i` of row `y` and pixel `i - 1` of
//! row `y + 1`. The two pixels don't depend on each other, so they go into one 8-lane vector
//! (4 channels × 2 rows) and every step produces two pixels for the latency of one.
//!
//! Lanes 0..4 ("lo") hold row `y`, lanes 4..8 ("hi") row `y + 1`. At step `i`:
//!
//! | operand | lo (row `y`, pixel `i`) | hi (row `y + 1`, pixel `i - 1`) |
//! |---|---|---|
//! | `a` (left) | `y[i - 1]` | `y1[i - 2]` |
//! | `b` (above) | `prev[i]` | `y[i - 1]` = lo of `a` |
//! | `c` (above left) | `prev[i - 1]` | `y[i - 2]` = hi of the previous `b` |
//!
//! so `b` is one shuffle of the loaded `prev[i]` with `a`, and `c` is the previous `b`.
//!
//! Four rows run as two such pairs, the second one two pixels behind the first: its "row
//! above" is the hi lanes of the first pair's previous result, still in a register. The two
//! pairs are independent chains, which hides more of the step latency. (1080p RGBA Paeth under
//! wasmtime, p50: one row 13.6 ms, two rows 7.0 ms, four rows 5 ms.)
//!
//! The per-pixel formula, the operands and the dependency order are those of the PNG
//! specification, so the output is byte-identical to reconstructing the rows one at a time; the
//! differential tests in `filter.rs` check it against the reference implementation.
//!
//! The vector type has two backends: safe `core::arch::wasm32` SIMD intrinsics with `simd128`
//! (no `unsafe`: only the pointer-based load/store intrinsics are unsafe, and they aren't used),
//! and a portable `[i16; 8]` everywhere else, so native tests run the same schedule.

use vector::V;

/// A predictor whose reconstruction can run as a wavefront: it reads the left (`a`), above
/// (`b`) and above-left (`c`) pixels only. Values are 0..=255 widened to `i16`.
pub(super) trait Predictor {
    fn predict(a: V, b: V, c: V) -> V;
}

/// Paeth, with the PNG specification's tie order (a, then b, then c). `a + b - 2c` needs 10
/// bits.
pub(super) struct Paeth;

impl Predictor for Paeth {
    #[inline(always)]
    fn predict(a: V, b: V, c: V) -> V {
        let pa = b.sub(c).abs(); // |p - a| with p = a + b - c
        let pb = a.sub(c).abs(); // |p - b|
        let pc = a.add(b).sub(c).sub(c).abs(); // |p - c|
        let use_a = pa.le(pb).and(pa.le(pc));
        let use_b = pb.le(pc);
        V::select(use_a, a, V::select(use_b, b, c))
    }
}

/// Average: `floor((a + b) / 2)`.
pub(super) struct Average;

impl Predictor for Average {
    #[inline(always)]
    fn predict(a: V, b: V, _c: V) -> V {
        a.add(b).shr1()
    }
}

/// One step: reconstructs `x` (filtered bytes, widened) with the predictor, modulo 256.
#[inline(always)]
fn step<P: Predictor>(x: V, a: V, b: V, c: V) -> V {
    x.add(P::predict(a, b, c)).low_byte()
}

/// The pixel at byte offset `at` of `block`, or zero for an out-of-range pixel (`None`).
#[inline(always)]
fn read(block: &[u8], at: Option<usize>) -> [u8; 4] {
    at.and_then(|o| block.get(o..o + 4))
        .and_then(|px| <[u8; 4]>::try_from(px).ok())
        .unwrap_or([0; 4])
}

/// Stores `px` at byte offset `at` of `block`; does nothing for an out-of-range pixel.
#[inline(always)]
fn write(block: &mut [u8], at: Option<usize>, px: [u8; 4]) {
    if let Some(dst) = at.and_then(|o| block.get_mut(o..o + 4)) {
        dst.copy_from_slice(&px);
    }
}

/// Reconstructs (`P`: Paeth or Average) `2 * K` consecutive rows of 4-byte pixels in place, as `K` two-row
/// wavefronts that are one more pixel behind each (row `r` is at pixel `s - r` at step `s`), so
/// every step runs `K` independent 8-lane chains. `K` is 1 or 2. `block` holds the rows,
/// `stride` bytes apart; each row's pixels start `off` bytes into it (1 for a stored scan line,
/// whose filter-type byte is left untouched; 0 for rows of pixels only, as in the output of
/// `Renderer`). `prev` is the reconstructed row above the first one, `stride - off` bytes.
///
/// Returns `false`, changing nothing, unless `K` is 1 or 2, `block` is exactly `2 * K` rows and
/// the rows are a whole number of pixels.
pub(super) fn unfilter_4_rows<P: Predictor, const K: usize>(
    prev: &[u8],
    block: &mut [u8],
    stride: usize,
    off: usize,
) -> bool {
    let len = prev.len();
    if !(K == 1 || K == 2)
        || stride != len + off
        || block.len() != stride * 2 * K
        || !len.is_multiple_of(4)
    {
        return false;
    }
    let n = len / 4;
    let tops = prev.as_chunks::<4>().0;
    let mut a = [V::zero(); K];
    let mut c = [V::zero(); K];

    // Steps `0..2K - 1` start the wavefronts (rows further down are still left of pixel 0) and
    // steps `n..n + 2K - 1` drain them (rows further up are past the end). They go through the
    // general, bounds-checked step; the steps in between, where every row is in range, through
    // the tight loops below.
    let total = n + 2 * K - 1;
    let head = (2 * K - 1).min(total);
    for s in 0..head {
        edge_step::<P, K>(tops, block, stride, off, n, s, &mut a, &mut c);
    }
    let mut rows = block.chunks_exact_mut(stride).map(|row| {
        row.get_mut(off..)
            .map_or(&mut [][..], |px| px.as_chunks_mut::<4>().0)
    });
    match (&mut a[..], &mut c[..]) {
        ([a0], [c0]) => {
            if let (Some(r0), Some(r1)) = (rows.next(), rows.next()) {
                steady_2_rows::<P>(tops, r0, r1, a0, c0);
            }
        }
        ([a0, a1], [c0, c1]) => {
            if let (Some(r0), Some(r1), Some(r2), Some(r3)) =
                (rows.next(), rows.next(), rows.next(), rows.next())
            {
                steady_4_rows::<P>(tops, [r0, r1, r2, r3], [a0, a1], [c0, c1]);
            }
        }
        _ => {}
    }
    for s in head.max(n)..total {
        edge_step::<P, K>(tops, block, stride, off, n, s, &mut a, &mut c);
    }
    true
}

/// Step `s` of [`unfilter_4_rows`] for any `s`: pixels out of range read as zero and are
/// not written. Before a row reaches pixel 0 its lanes stay zero, which is the zero "pixel -1"
/// that the PNG specification uses at the left edge.
#[expect(
    clippy::too_many_arguments,
    reason = "the step's whole state; a struct would only rename the same eight values"
)]
fn edge_step<P: Predictor, const K: usize>(
    tops: &[[u8; 4]],
    block: &mut [u8],
    stride: usize,
    off: usize,
    n: usize,
    s: usize,
    a: &mut [V; K],
    c: &mut [V; K],
) {
    // Byte offset of pixel `j` (if in range) of row `r`.
    let at = |r: usize, j: Option<usize>| j.filter(|&j| j < n).map(|j| r * stride + off + 4 * j);
    let zero = V::zero();
    // Pair k works on rows 2k (lo, pixel s - 2k) and 2k + 1 (hi, pixel s - 2k - 1). Its "row
    // above" lo is the hi of pair k - 1 from the previous step, which is still in `a[k - 1]`; so
    // go from the last pair to the first.
    for k in (0..K).rev() {
        let j0 = s.checked_sub(2 * k);
        let j1 = s.checked_sub(2 * k + 1);
        let (o0, o1) = (at(2 * k, j0), at(2 * k + 1, j1));
        let ak = a.get(k).copied().unwrap_or(zero);
        let b = match k.checked_sub(1).and_then(|p| a.get(p)) {
            Some(above) => V::hi_lo(*above, ak),
            None => {
                let top = j0.and_then(|j| tops.get(j)).copied().unwrap_or([0; 4]);
                V::lo_hi(V::widen(top, [0; 4]), ak)
            }
        };
        let ck = c.get(k).copied().unwrap_or(zero);
        let res = step::<P>(V::widen(read(block, o0), read(block, o1)), ak, b, ck);
        let (p0, p1) = res.narrow();
        write(block, o0, p0);
        write(block, o1, p1);
        if let (Some(ak), Some(ck)) = (a.get_mut(k), c.get_mut(k)) {
            *ak = res;
            *ck = b;
        }
    }
}

/// Steps `1..n` of the two-row wavefront: pixel `s` of row 0 and pixel `s - 1` of row 1.
#[inline(always)]
fn steady_2_rows<P: Predictor>(
    tops: &[[u8; 4]],
    r0: &mut [[u8; 4]],
    r1: &mut [[u8; 4]],
    a: &mut V,
    c: &mut V,
) {
    let (mut a0, mut c0) = (*a, *c);
    for ((top, x0), x1) in tops
        .iter()
        .skip(1)
        .zip(r0.iter_mut().skip(1))
        .zip(r1.iter_mut())
    {
        let b0 = V::lo_hi(V::widen(*top, [0; 4]), a0);
        a0 = step::<P>(V::widen(*x0, *x1), a0, b0, c0);
        (*x0, *x1) = a0.narrow();
        c0 = b0;
    }
    (*a, *c) = (a0, c0);
}

/// Steps `3..n` of the four-row wavefront: pixel `s - r` of row `r`. The two pairs are
/// independent within a step; pair 1 reads pair 0's previous result before it is replaced.
#[inline(always)]
fn steady_4_rows<P: Predictor>(
    tops: &[[u8; 4]],
    [r0, r1, r2, r3]: [&mut [[u8; 4]]; 4],
    [a0, a1]: [&mut V; 2],
    [c0, c1]: [&mut V; 2],
) {
    let (mut p0, mut p1, mut q0, mut q1) = (*a0, *a1, *c0, *c1);
    let pixels = tops
        .iter()
        .skip(3)
        .zip(r0.iter_mut().skip(3))
        .zip(r1.iter_mut().skip(2))
        .zip(r2.iter_mut().skip(1))
        .zip(r3.iter_mut());
    for ((((top, x0), x1), x2), x3) in pixels {
        let b1 = V::hi_lo(p0, p1);
        let b0 = V::lo_hi(V::widen(*top, [0; 4]), p0);
        p1 = step::<P>(V::widen(*x2, *x3), p1, b1, q1);
        p0 = step::<P>(V::widen(*x0, *x1), p0, b0, q0);
        (*x2, *x3) = p1.narrow();
        (*x0, *x1) = p0.narrow();
        (q0, q1) = (b0, b1);
    }
    (*a0, *a1, *c0, *c1) = (p0, p1, q0, q1);
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod vector {
    use core::arch::wasm32::*;

    /// Eight `i16` lanes in a `v128`.
    #[derive(Clone, Copy)]
    pub(in crate::png::scan_line::filter) struct V(v128);

    impl V {
        #[inline(always)]
        pub(super) fn zero() -> V {
            V(i16x8_splat(0))
        }
        /// Two pixels, zero-extended: `lo` into lanes 0..4, `hi` into lanes 4..8.
        #[inline(always)]
        pub(super) fn widen(lo: [u8; 4], hi: [u8; 4]) -> V {
            let bytes = u32x4(u32::from_le_bytes(lo), u32::from_le_bytes(hi), 0, 0);
            V(u16x8_extend_low_u8x16(bytes))
        }
        /// The low bytes of the lanes, as the `lo` and `hi` pixels. Lanes must be 0..=255.
        #[inline(always)]
        pub(super) fn narrow(self) -> ([u8; 4], [u8; 4]) {
            let bytes = u8x16_narrow_i16x8(self.0, self.0);
            (
                u32x4_extract_lane::<0>(bytes).to_le_bytes(),
                u32x4_extract_lane::<1>(bytes).to_le_bytes(),
            )
        }
        /// Lanes 0..4 of `lo` followed by lanes 0..4 of `hi`.
        #[inline(always)]
        pub(super) fn lo_hi(lo: V, hi: V) -> V {
            V(i16x8_shuffle::<0, 1, 2, 3, 8, 9, 10, 11>(lo.0, hi.0))
        }
        /// Lanes 4..8 of `hi` followed by lanes 0..4 of `lo`.
        #[inline(always)]
        pub(super) fn hi_lo(hi: V, lo: V) -> V {
            V(i16x8_shuffle::<4, 5, 6, 7, 8, 9, 10, 11>(hi.0, lo.0))
        }
        #[inline(always)]
        pub(super) fn add(self, o: V) -> V {
            V(i16x8_add(self.0, o.0))
        }
        #[inline(always)]
        pub(super) fn sub(self, o: V) -> V {
            V(i16x8_sub(self.0, o.0))
        }
        #[inline(always)]
        pub(super) fn abs(self) -> V {
            V(i16x8_abs(self.0))
        }
        /// All-ones lanes where `self <= o`.
        #[inline(always)]
        pub(super) fn le(self, o: V) -> V {
            V(i16x8_le(self.0, o.0))
        }
        #[inline(always)]
        pub(super) fn and(self, o: V) -> V {
            V(v128_and(self.0, o.0))
        }
        /// `t` where `mask` is set, `f` elsewhere.
        #[inline(always)]
        pub(super) fn select(mask: V, t: V, f: V) -> V {
            V(v128_bitselect(t.0, f.0, mask.0))
        }
        /// Each lane shifted right by one bit (lanes are non-negative).
        #[inline(always)]
        pub(super) fn shr1(self) -> V {
            V(u16x8_shr(self.0, 1))
        }
        /// Each lane modulo 256.
        #[inline(always)]
        pub(super) fn low_byte(self) -> V {
            V(v128_and(self.0, i16x8_splat(0xff)))
        }
    }
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
mod vector {
    /// Eight `i16` lanes, portable. Same semantics as the `simd128` backend.
    #[derive(Clone, Copy)]
    pub(in crate::png::scan_line::filter) struct V([i16; 8]);

    impl V {
        #[inline(always)]
        fn map2(self, o: V, f: impl Fn(i16, i16) -> i16) -> V {
            let mut out = [0; 8];
            for ((r, x), y) in out.iter_mut().zip(self.0).zip(o.0) {
                *r = f(x, y);
            }
            V(out)
        }
        #[inline(always)]
        pub(super) fn zero() -> V {
            V([0; 8])
        }
        #[inline(always)]
        pub(super) fn widen(lo: [u8; 4], hi: [u8; 4]) -> V {
            let mut out = [0; 8];
            for (r, v) in out.iter_mut().zip(lo.iter().chain(&hi)) {
                *r = i16::from(*v);
            }
            V(out)
        }
        #[inline(always)]
        pub(super) fn narrow(self) -> ([u8; 4], [u8; 4]) {
            let mut lo = [0; 4];
            let mut hi = [0; 4];
            for (r, v) in lo.iter_mut().chain(hi.iter_mut()).zip(self.0) {
                *r = v as u8;
            }
            (lo, hi)
        }
        #[inline(always)]
        pub(super) fn lo_hi(lo: V, hi: V) -> V {
            let mut out = [0; 8];
            for (r, v) in out
                .iter_mut()
                .zip(lo.0.iter().take(4).chain(hi.0.iter().take(4)))
            {
                *r = *v;
            }
            V(out)
        }
        #[inline(always)]
        pub(super) fn hi_lo(hi: V, lo: V) -> V {
            let mut out = [0; 8];
            for (r, v) in out
                .iter_mut()
                .zip(hi.0.iter().skip(4).chain(lo.0.iter().take(4)))
            {
                *r = *v;
            }
            V(out)
        }
        #[inline(always)]
        pub(super) fn add(self, o: V) -> V {
            self.map2(o, i16::wrapping_add)
        }
        #[inline(always)]
        pub(super) fn sub(self, o: V) -> V {
            self.map2(o, i16::wrapping_sub)
        }
        #[inline(always)]
        pub(super) fn abs(self) -> V {
            self.map2(self, |x, _| x.wrapping_abs())
        }
        #[inline(always)]
        pub(super) fn le(self, o: V) -> V {
            self.map2(o, |x, y| if x <= y { -1 } else { 0 })
        }
        #[inline(always)]
        pub(super) fn and(self, o: V) -> V {
            self.map2(o, |x, y| x & y)
        }
        #[inline(always)]
        pub(super) fn select(mask: V, t: V, f: V) -> V {
            let picked = t.and(mask);
            picked.map2(f.map2(mask, |x, m| x & !m), |x, y| x | y)
        }
        #[inline(always)]
        pub(super) fn shr1(self) -> V {
            self.map2(self, |x, _| ((x as u16) >> 1) as i16)
        }
        #[inline(always)]
        pub(super) fn low_byte(self) -> V {
            self.map2(self, |x, _| x & 0xff)
        }
    }
}
