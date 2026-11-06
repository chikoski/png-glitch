//! Deterministic effects whose "where" and "how much" are explicit parameters.
//!
//! A randomized effect picks rows, bytes or offsets with an RNG. These effects take those
//! choices as values instead, so the same image and parameters always give the same bytes.
//! A caller that wants randomness draws the parameters from its own RNG.

use super::Effect;
use crate::{FilterType, Pixel, PngGlitch, ScanLine};

/// Runs `f` on each listed row, in the given order. Rows outside the image are ignored.
fn for_rows(png: &mut PngGlitch, rows: &[u32], mut f: impl FnMut(&mut ScanLine)) {
    let mut lines = png.scan_lines();
    for &row in rows {
        if let Some(line) = lines.get_mut(row as usize) {
            f(line);
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

impl Effect for Refilter {
    fn apply_to(&self, png: &mut PngGlitch) {
        png.reencode_with(self.filter_type);
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

impl Effect for SetFilterTypes {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for (line, filter_type) in lines.iter_mut().zip(&self.filter_types) {
            if let Some(filter_type) = filter_type {
                line.set_filter_type(*filter_type);
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

impl Effect for SetBytes {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for edit in &self.edits {
            if let Some(line) = lines.get_mut(edit.line as usize) {
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

impl Effect for Substitute {
    fn apply_to(&self, png: &mut PngGlitch) {
        png.foreach_scanline(|line| {
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

impl Effect for SwapLines {
    fn apply_to(&self, png: &mut PngGlitch) {
        let height = png.height();
        for &(a, b) in &self.swaps {
            if a != b && a < height && b < height {
                png.transpose(a, b, 1);
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

impl Effect for CopyLines {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for &(src, dst) in &self.copies {
            let (src, dst) = (src as usize, dst as usize);
            if src == dst || src >= lines.len() || dst >= lines.len() {
                continue;
            }
            let filter_type = lines[src].filter_type();
            let buffer: Vec<u8> = (0..lines[src].size())
                .map(|i| lines[src].index(i).unwrap_or(0))
                .collect();
            let dest = &mut lines[dst];
            for (i, &byte) in buffer.iter().enumerate() {
                dest.update(i, byte);
            }
            dest.set_filter_type(filter_type);
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

impl Effect for ShiftLines {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for s in &self.shifts {
            let Some(line) = lines.get_mut(s.line as usize) else {
                continue;
            };
            let size = line.size();
            if size <= 1 {
                continue;
            }
            let shift = s.shift % size;
            let buffer: Vec<u8> = (0..size).map(|i| line.index(i).unwrap_or(0)).collect();
            for (i, &byte) in buffer.iter().enumerate() {
                line.update((i + shift) % size, byte);
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
}

impl Effect for PixelSort {
    fn apply_to(&self, png: &mut PngGlitch) {
        for_rows(png, &self.rows, |line| {
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
        });
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

impl Effect for Bitwise {
    fn apply_to(&self, png: &mut PngGlitch) {
        for_rows(png, &self.rows, |line| {
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

impl Effect for SwapChannels {
    fn apply_to(&self, png: &mut PngGlitch) {
        for_rows(png, &self.rows, |line| {
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

impl Effect for OffsetColors {
    fn apply_to(&self, png: &mut PngGlitch) {
        let mut lines = png.scan_lines();
        for entry in &self.lines {
            let Some(line) = lines.get_mut(entry.line as usize) else {
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

impl Effect for HslShift {
    fn apply_to(&self, png: &mut PngGlitch) {
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
        for_rows(png, &self.rows, |line| {
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

impl Effect for ChannelShift {
    fn apply_to(&self, png: &mut PngGlitch) {
        let width = png.width() as usize;
        if width == 0 {
            return;
        }
        for_rows(png, &self.rows, |line| {
            let mut r_channel = Vec::with_capacity(width);
            let mut g_channel = Vec::with_capacity(width);
            let mut b_channel = Vec::with_capacity(width);
            let mut alpha = Vec::with_capacity(width);

            for x in 0..width {
                if let Some(p) = line.get_pixel(x) {
                    r_channel.push(p.r());
                    g_channel.push(p.g());
                    b_channel.push(p.b());
                    alpha.push(p.a());
                }
            }

            line.process_pixels(|x, p| {
                let get_val = |chan: &[u16], offset: i32| {
                    let nx = (x as i64 + i64::from(offset)).rem_euclid(width as i64) as usize;
                    chan.get(nx).copied().unwrap_or(0)
                };
                match p {
                    Pixel::RGB(_, _, _) => Pixel::RGB(
                        get_val(&r_channel, self.r_offset),
                        get_val(&g_channel, self.g_offset),
                        get_val(&b_channel, self.b_offset),
                    ),
                    Pixel::RGBA(_, _, _, _) => Pixel::RGBA(
                        get_val(&r_channel, self.r_offset),
                        get_val(&g_channel, self.g_offset),
                        get_val(&b_channel, self.b_offset),
                        alpha.get(x).copied().unwrap_or(0),
                    ),
                    _ => p,
                }
            });
        });
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

impl Effect for SwapBlocks {
    fn apply_to(&self, png: &mut PngGlitch) {
        let width = png.width();
        let height = png.height();
        let block_size = self.block_size;
        if width == 0 || height == 0 || block_size == 0 {
            return;
        }
        let num_blocks_x = width / block_size;
        let num_blocks_y = height / block_size;
        if num_blocks_x == 0 || num_blocks_y == 0 {
            return;
        }

        let mut scan_lines = png.scan_lines();

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

                if src_y == dst_y {
                    let line = &mut scan_lines[src_y as usize];
                    for x_off in 0..block_size {
                        let src_x = src_bx * block_size + x_off;
                        let dst_x = dst_bx * block_size + x_off;
                        if src_x < width && dst_x < width {
                            let p_src = line.get_pixel(src_x as usize);
                            let p_dst = line.get_pixel(dst_x as usize);
                            if let (Some(ps), Some(pd)) = (p_src, p_dst) {
                                line.set_pixel(src_x as usize, pd);
                                line.set_pixel(dst_x as usize, ps);
                            }
                        }
                    }
                } else {
                    let (src_line, dst_line) = if src_y < dst_y {
                        let (left, right) = scan_lines.split_at_mut(dst_y as usize);
                        (&mut left[src_y as usize], &mut right[0])
                    } else {
                        let (left, right) = scan_lines.split_at_mut(src_y as usize);
                        (&mut right[0], &mut left[dst_y as usize])
                    };

                    for x_off in 0..block_size {
                        let src_x = src_bx * block_size + x_off;
                        let dst_x = dst_bx * block_size + x_off;
                        if src_x < width && dst_x < width {
                            let p_src = src_line.get_pixel(src_x as usize);
                            let p_dst = dst_line.get_pixel(dst_x as usize);
                            if let (Some(ps), Some(pd)) = (p_src, p_dst) {
                                src_line.set_pixel(src_x as usize, pd);
                                dst_line.set_pixel(dst_x as usize, ps);
                            }
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
