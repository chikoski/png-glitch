//! The built-in randomized filters. Each one draws its choices from the RNG and resolves to a
//! deterministic `png-glitch` effect.

use crate::*;

/// Decides, for each row in order, whether it is selected with probability `magnitude`.
pub(crate) fn draw_rows(header: &ImageHeader, rng: &mut dyn RngCore, magnitude: f64) -> Vec<u32> {
    let magnitude = clamp_magnitude(magnitude);
    (0..header.height)
        .filter(|_| rng.random_bool(magnitude))
        .collect()
}

pub(crate) fn clamp_magnitude(m: f64) -> f64 {
    if m.is_nan() { 0.0 } else { m.clamp(0.0, 1.0) }
}

/// Filter that randomly overwrites the PNG filter type byte of scanlines without re-filtering pixel data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChangeFilterType {
    /// Probability (`0.0..=1.0`) of changing each scanline's filter type byte.
    pub magnitude: f64,
}

impl Default for ChangeFilterType {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl ChangeFilterType {
    /// Creates a new [`ChangeFilterType`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

impl Resolve for ChangeFilterType {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let magnitude = clamp_magnitude(self.magnitude);
        let filter_types = (0..header.height)
            .map(|_| {
                rng.random_bool(magnitude)
                    .then(|| match rng.random_range(0..5) {
                        0 => FilterType::None,
                        1 => FilterType::Sub,
                        2 => FilterType::Up,
                        3 => FilterType::Average,
                        _ => FilterType::Paeth,
                    })
            })
            .collect();
        ResolvedStep::SetFilterTypes(fx::SetFilterTypes::new(filter_types))
    }
}

/// Filter that re-encodes all scanlines with [`FilterType::None`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoveFilter;

impl RemoveFilter {
    /// Creates a new [`RemoveFilter`].
    pub fn new() -> Self {
        Self
    }
}

impl Resolve for RemoveFilter {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Refilter(fx::Refilter::new(FilterType::None))
    }
}

/// Filter that re-encodes all scanlines with [`FilterType::Sub`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubFilter;

impl SubFilter {
    /// Creates a new [`SubFilter`].
    pub fn new() -> Self {
        Self
    }
}

impl Resolve for SubFilter {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Refilter(fx::Refilter::new(FilterType::Sub))
    }
}

/// Filter that re-encodes all scanlines with [`FilterType::Up`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UpFilter;

impl UpFilter {
    /// Creates a new [`UpFilter`].
    pub fn new() -> Self {
        Self
    }
}

impl Resolve for UpFilter {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Refilter(fx::Refilter::new(FilterType::Up))
    }
}

/// Filter that re-encodes all scanlines with [`FilterType::Average`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AverageFilter;

impl AverageFilter {
    /// Creates a new [`AverageFilter`].
    pub fn new() -> Self {
        Self
    }
}

impl Resolve for AverageFilter {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Refilter(fx::Refilter::new(FilterType::Average))
    }
}

/// Filter that re-encodes all scanlines with [`FilterType::Paeth`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaethFilter;

impl PaethFilter {
    /// Creates a new [`PaethFilter`].
    pub fn new() -> Self {
        Self
    }
}

impl Resolve for PaethFilter {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Refilter(fx::Refilter::new(FilterType::Paeth))
    }
}

/// Filter that replaces random scanline bytes with random values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Replace {
    /// Probability (`0.0..=1.0`) of replacing a byte in each scanline.
    pub magnitude: f64,
}

impl Default for Replace {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl Replace {
    /// Creates a new [`Replace`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

impl Resolve for Replace {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let magnitude = clamp_magnitude(self.magnitude);
        let size = header.line_size();
        let mut edits = Vec::new();
        if size > 0 {
            for line in 0..header.height {
                if rng.random_bool(magnitude) {
                    let value: u8 = rng.random();
                    let index = rng.random_range(0..size);
                    edits.push(fx::ByteEdit { line, index, value });
                }
            }
        }
        ResolvedStep::SetBytes(fx::SetBytes::new(edits))
    }
}

/// Filter that swaps scanlines randomly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transpose {
    /// Probability (`0.0..=1.0`) of swapping each scanline with another random scanline.
    pub magnitude: f64,
}

impl Default for Transpose {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl Transpose {
    /// Creates a new [`Transpose`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

/// Old name of [`Transpose`].
#[deprecated(since = "0.3.0", note = "use `Transpose`; removed before 1.0")]
pub type RandomTranspose = Transpose;

impl Resolve for Transpose {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let magnitude = clamp_magnitude(self.magnitude);
        let height = header.height;
        if height <= 1 {
            return ResolvedStep::Noop;
        }
        let mut swaps = Vec::new();
        for i in 0..height {
            if rng.random_bool(magnitude) {
                let target = rng.random_range(0..height);
                swaps.push((i, target));
            }
        }
        ResolvedStep::SwapLines(fx::SwapLines::new(swaps))
    }
}

/// Filter that copies random scanlines onto other scanlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomCopy {
    /// Number of random scanline copy operations to perform.
    pub times: u32,
}

impl Default for RandomCopy {
    fn default() -> Self {
        Self { times: 1 }
    }
}

impl RandomCopy {
    /// Creates a new [`RandomCopy`] filter that copies scanlines `times` times.
    pub fn new(times: u32) -> Self {
        Self { times }
    }
}

impl Resolve for RandomCopy {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let height = header.height;
        if height <= 1 {
            return ResolvedStep::Noop;
        }
        let times = self.times.min(MAX_RANDOM_COPY_TIMES);
        let copies = (0..times)
            .map(|_| {
                let src = rng.random_range(0..height);
                let dst = rng.random_range(0..height);
                (src, dst)
            })
            .collect();
        ResolvedStep::CopyLines(fx::CopyLines::new(copies))
    }
}

/// Filter that sets a byte at a specific index to a specific value across all scanlines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Substitute {
    /// Byte offset within each scanline.
    pub index: usize,
    /// Replacement byte value.
    pub value: u8,
}

impl Substitute {
    /// Creates a new [`Substitute`] filter setting byte `index` to `value` on every scanline.
    pub fn new(index: usize, value: u8) -> Self {
        Self { index, value }
    }
}

impl Resolve for Substitute {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Substitute(fx::Substitute::new(self.index, self.value))
    }
}

/// Filter that sets random scanline bytes to zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetZero {
    /// Probability (`0.0..=1.0`) of zeroing a byte in each scanline.
    pub magnitude: f64,
}

impl Default for SetZero {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl SetZero {
    /// Creates a new [`SetZero`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

impl Resolve for SetZero {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let magnitude = clamp_magnitude(self.magnitude);
        let size = header.line_size();
        let mut edits = Vec::new();
        if size > 0 {
            for line in 0..header.height {
                if rng.random_bool(magnitude) {
                    let index = rng.random_range(0..size);
                    edits.push(fx::ByteEdit {
                        line,
                        index,
                        value: 0,
                    });
                }
            }
        }
        ResolvedStep::SetBytes(fx::SetBytes::new(edits))
    }
}

/// Criterion used by [`PixelSort`] to order pixels within a scanline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum SortCriterion {
    /// Sorts pixels by average RGB brightness `(r + g + b) / 3`.
    #[default]
    #[serde(alias = "Brightness")]
    Brightness,
    /// Sorts pixels by HSL hue angle (`0.0..=360.0`).
    #[serde(alias = "Hue")]
    Hue,
}

/// Filter that sorts pixels in each selected scanline by brightness or hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelSort {
    /// Criterion used to compare pixels.
    pub criterion: SortCriterion,
    /// Probability (`0.0..=1.0`) of sorting each scanline.
    pub magnitude: f64,
}

impl Default for PixelSort {
    fn default() -> Self {
        Self {
            criterion: SortCriterion::Brightness,
            magnitude: 0.5,
        }
    }
}

impl PixelSort {
    /// Creates a new [`PixelSort`] filter with the given magnitude and default [`SortCriterion::Brightness`].
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the sort criterion.
    pub fn with_criterion(mut self, criterion: SortCriterion) -> Self {
        self.criterion = criterion;
        self
    }
}

impl Resolve for PixelSort {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let rows = draw_rows(header, rng, self.magnitude);
        let criterion = match self.criterion {
            SortCriterion::Brightness => fx::SortCriterion::Brightness,
            SortCriterion::Hue => fx::SortCriterion::Hue,
        };
        ResolvedStep::PixelSort(fx::PixelSort::new(criterion, rows))
    }
}

/// Bitwise operation performed by [`Bitwise`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum BitOp {
    /// Bitwise AND (`b & value`).
    #[serde(alias = "And", alias = "AND")]
    And,
    /// Bitwise OR (`b | value`).
    #[serde(alias = "Or", alias = "OR")]
    Or,
    /// Bitwise XOR (`b ^ value`).
    #[default]
    #[serde(alias = "Xor", alias = "XOR")]
    Xor,
}

/// Filter that performs bitwise operations on scanline data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bitwise {
    /// Bitwise operator to apply.
    pub op: BitOp,
    /// Right-hand byte operand.
    pub value: u8,
    /// Probability (`0.0..=1.0`) of applying the operation to each scanline.
    pub magnitude: f64,
}

impl Default for Bitwise {
    fn default() -> Self {
        Self {
            op: BitOp::Xor,
            value: 0,
            magnitude: 0.5,
        }
    }
}

impl Bitwise {
    /// Creates a new [`Bitwise`] filter with the given magnitude, default [`BitOp::Xor`], and value `0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the bitwise operation.
    pub fn with_op(mut self, op: BitOp) -> Self {
        self.op = op;
        self
    }

    /// Sets the operand byte value.
    pub fn with_value(mut self, value: u8) -> Self {
        self.value = value;
        self
    }
}

impl Resolve for Bitwise {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let rows = draw_rows(header, rng, self.magnitude);
        let op = match self.op {
            BitOp::And => fx::BitOp::And,
            BitOp::Or => fx::BitOp::Or,
            BitOp::Xor => fx::BitOp::Xor,
        };
        ResolvedStep::Bitwise(fx::Bitwise::new(op, self.value, rows))
    }
}

/// Pair of color channels to swap in [`ChannelSwap`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum SwapTarget {
    /// Swaps Red and Green channels.
    #[default]
    #[serde(alias = "Rg", alias = "RG")]
    Rg,
    /// Swaps Green and Blue channels.
    #[serde(alias = "Gb", alias = "GB")]
    Gb,
    /// Swaps Blue and Red channels.
    #[serde(alias = "Br", alias = "BR")]
    Br,
}

/// Filter that swaps a pair of color channels on randomly selected scanlines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSwap {
    /// Channel pair to swap.
    pub target: SwapTarget,
    /// Probability (`0.0..=1.0`) of swapping channels in each scanline.
    pub magnitude: f64,
}

impl Default for ChannelSwap {
    fn default() -> Self {
        Self {
            target: SwapTarget::Rg,
            magnitude: 0.5,
        }
    }
}

impl ChannelSwap {
    /// Creates a new [`ChannelSwap`] filter with the given magnitude and default [`SwapTarget::Rg`].
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the channel pair to swap.
    pub fn with_target(mut self, target: SwapTarget) -> Self {
        self.target = target;
        self
    }
}

impl Resolve for ChannelSwap {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let rows = draw_rows(header, rng, self.magnitude);
        let target = match self.target {
            SwapTarget::Rg => fx::SwapTarget::Rg,
            SwapTarget::Gb => fx::SwapTarget::Gb,
            SwapTarget::Br => fx::SwapTarget::Br,
        };
        ResolvedStep::SwapChannels(fx::SwapChannels::new(target, rows))
    }
}

/// Filter that shifts scanlines horizontally by a random byte offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizontalShift {
    /// Probability (`0.0..=1.0`) of shifting each scanline.
    pub magnitude: f64,
}

impl Default for HorizontalShift {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl HorizontalShift {
    /// Creates a new [`HorizontalShift`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

impl Resolve for HorizontalShift {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let magnitude = clamp_magnitude(self.magnitude);
        let size = header.line_size();
        let mut shifts = Vec::new();
        if size > 1 {
            for line in 0..header.height {
                if rng.random_bool(magnitude) {
                    let shift = rng.random_range(0..size);
                    shifts.push(fx::LineShift { line, shift });
                }
            }
        }
        ResolvedStep::ShiftLines(fx::ShiftLines::new(shifts))
    }
}

/// Filter that scrambles square blocks of pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockScramble {
    /// Fraction (`0.0..=1.0`) of blocks to swap.
    pub magnitude: f64,
    /// Edge length of each square block in pixels (defaults to `16`).
    pub block_size: u32,
}

impl Default for BlockScramble {
    fn default() -> Self {
        Self {
            magnitude: 0.5,
            block_size: 16,
        }
    }
}

impl BlockScramble {
    /// Creates a new [`BlockScramble`] filter with the given magnitude and default `block_size = 16`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the block size in pixels.
    pub fn with_block_size(mut self, block_size: u32) -> Self {
        self.block_size = block_size;
        self
    }
}

impl Resolve for BlockScramble {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let width = header.width;
        let height = header.height;
        let block_size = self.block_size;

        if width == 0 || height == 0 || block_size == 0 {
            return ResolvedStep::Noop;
        }

        let num_blocks_x = width / block_size;
        let num_blocks_y = height / block_size;

        if num_blocks_x == 0 || num_blocks_y == 0 {
            return ResolvedStep::Noop;
        }

        let total_blocks = (num_blocks_x * num_blocks_y) as usize;
        let num_scrambles = (total_blocks as f64 * clamp_magnitude(self.magnitude)) as usize;

        let swaps = (0..num_scrambles)
            .map(|_| {
                let src_bx = rng.random_range(0..num_blocks_x);
                let src_by = rng.random_range(0..num_blocks_y);
                let dst_bx = rng.random_range(0..num_blocks_x);
                let dst_by = rng.random_range(0..num_blocks_y);
                fx::BlockSwap {
                    src: (src_bx, src_by),
                    dst: (dst_bx, dst_by),
                }
            })
            .collect();
        ResolvedStep::SwapBlocks(fx::SwapBlocks::new(block_size, swaps))
    }
}

/// Filter that adds random per-channel color distortion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorDistortion {
    /// Probability (`0.0..=1.0`) of distorting each scanline.
    pub magnitude: f64,
    /// Maximum signed offset added to each color channel (defaults to `20`).
    pub strength: i16,
}

impl Default for ColorDistortion {
    fn default() -> Self {
        Self {
            magnitude: 0.5,
            strength: 20,
        }
    }
}

impl ColorDistortion {
    /// Creates a new [`ColorDistortion`] filter with the given magnitude and default `strength = 20`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the distortion strength.
    pub fn with_strength(mut self, strength: i16) -> Self {
        self.strength = strength;
        self
    }
}

impl Resolve for ColorDistortion {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let strength = self.strength.saturating_abs();
        let magnitude = clamp_magnitude(self.magnitude);
        let pixels = if header.bit_depth == 0 {
            0
        } else {
            header.width as usize
        };
        let mut lines = Vec::new();
        for line in 0..header.height {
            if rng.random_bool(magnitude) {
                let offsets = (0..pixels)
                    .map(|_| {
                        let r = rng.random_range(-strength..=strength);
                        let g = rng.random_range(-strength..=strength);
                        let b = rng.random_range(-strength..=strength);
                        [r, g, b]
                    })
                    .collect();
                lines.push(fx::LineOffsets { line, offsets });
            }
        }
        ResolvedStep::OffsetColors(fx::OffsetColors::new(lines))
    }
}

/// Filter that manipulates colors in HSL space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorSpaceGlitch {
    /// Probability (`0.0..=1.0`) of transforming each scanline.
    pub magnitude: f64,
    /// Hue rotation in degrees (`0.0..=360.0`, defaults to `0.0`).
    pub hue_shift: f64,
    /// Saturation multiplier (defaults to `1.0`).
    pub saturation_mult: f64,
    /// Lightness multiplier (defaults to `1.0`).
    pub lightness_mult: f64,
}

impl Default for ColorSpaceGlitch {
    fn default() -> Self {
        Self {
            magnitude: 0.5,
            hue_shift: 0.0,
            saturation_mult: 1.0,
            lightness_mult: 1.0,
        }
    }
}

impl ColorSpaceGlitch {
    /// Creates a new [`ColorSpaceGlitch`] filter with the given magnitude and neutral HSL modifiers.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the hue shift in degrees (`0.0..=360.0`).
    pub fn with_hue_shift(mut self, hue_shift: f64) -> Self {
        self.hue_shift = hue_shift;
        self
    }

    /// Sets the saturation multiplier.
    pub fn with_saturation_mult(mut self, saturation_mult: f64) -> Self {
        self.saturation_mult = saturation_mult;
        self
    }

    /// Sets the lightness multiplier.
    pub fn with_lightness_mult(mut self, lightness_mult: f64) -> Self {
        self.lightness_mult = lightness_mult;
        self
    }
}

impl Resolve for ColorSpaceGlitch {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        let rows = draw_rows(header, rng, self.magnitude);
        ResolvedStep::HslShift(fx::HslShift::new(
            self.hue_shift,
            self.saturation_mult,
            self.lightness_mult,
            rows,
        ))
    }
}

/// Filter that creates chromatic aberration by horizontally shifting R, G, and B channels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromaticAberration {
    /// Probability (`0.0..=1.0`) of applying chromatic aberration to each scanline.
    pub magnitude: f64,
    /// Horizontal offset in pixels for the red channel (defaults to `2`).
    pub r_offset: i32,
    /// Horizontal offset in pixels for the green channel (defaults to `0`).
    pub g_offset: i32,
    /// Horizontal offset in pixels for the blue channel (defaults to `-2`).
    pub b_offset: i32,
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self {
            magnitude: 0.5,
            r_offset: 2,
            g_offset: 0,
            b_offset: -2,
        }
    }
}

impl ChromaticAberration {
    /// Creates a new [`ChromaticAberration`] filter with the given magnitude and default offsets `(2, 0, -2)`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the horizontal pixel offsets for the R, G, and B channels.
    pub fn with_offsets(mut self, r_offset: i32, g_offset: i32, b_offset: i32) -> Self {
        self.r_offset = r_offset;
        self.g_offset = g_offset;
        self.b_offset = b_offset;
        self
    }
}

impl Resolve for ChromaticAberration {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
        if header.width == 0 {
            return ResolvedStep::Noop;
        }
        let rows = draw_rows(header, rng, self.magnitude);
        ResolvedStep::ChannelShift(fx::ChannelShift::new(
            self.r_offset,
            self.g_offset,
            self.b_offset,
            rows,
        ))
    }
}

impl Resolve for Invert {
    fn resolve(&self, __header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Invert(*self)
    }
}

impl Resolve for Brighten {
    fn resolve(&self, __header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Brighten(*self)
    }
}

impl Resolve for Darken {
    fn resolve(&self, __header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::Darken(*self)
    }
}

impl Resolve for ShiftChannels {
    fn resolve(&self, __header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedStep {
        ResolvedStep::ShiftChannels(*self)
    }
}

impl_glitch_filter_via_resolve!(
    ChangeFilterType,
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
    Replace,
    Transpose,
    RandomCopy,
    Substitute,
    SetZero,
    PixelSort,
    Bitwise,
    ChannelSwap,
    HorizontalShift,
    BlockScramble,
    ColorDistortion,
    ColorSpaceGlitch,
    ChromaticAberration,
    Invert,
    Brighten,
    Darken,
    ShiftChannels,
);

// PixelFilter implementations for the shared presets

// PixelFilterAdapter: applies a PixelFilter to PNG images
