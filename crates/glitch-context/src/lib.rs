//! High-level context and built-in glitch filters (effects) for PNG and WebP images.
//!
//! `glitch-context` orchestrates sequential filter pipelines on [`PngGlitch`] (`GlitchContext`),
//! [`WebpGlitch`] (`WebpGlitchContext`, behind the default `webp` feature), or format-autodetected
//! images via [`AnyGlitchContext`] / [`open_any`].
#![warn(missing_docs)]

use anyhow::{bail, Context, Result};
#[cfg(feature = "clap")]
use clap::ValueEnum;
pub use png_glitch::presets::{Brighten, Darken, Invert, ShiftChannels};
pub use png_glitch::{
    FilterType, Limits, ParseOptions, Pixel, PngError, PngGlitch, PngWarning,
};
pub use rand;
use rand::{Rng, RngCore, SeedableRng};
pub use rand_chacha;
pub use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::path::Path;

/// Maximum number of scanline copies allowed in a single [`RandomCopy`] invocation (`100_000`).
pub const MAX_RANDOM_COPY_TIMES: u32 = 100_000;

/// A trait for glitch filters (effects) operating on PNG images.
pub trait GlitchFilter {
    /// Applies the filter to the [`PngGlitch`] context using the provided RNG.
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore);
}

/// Alias for [`GlitchFilter`] to distinguish high-level glitch effects from PNG prediction filters ([`FilterType`]).
pub use GlitchFilter as Effect;

/// A pixel-level filter that operates on normalized RGBA `u8` values.
///
/// Implement this trait to create a filter that works on both PNG and WebP images.
/// Use [`PixelFilterAdapter`] to apply it to either format.
pub trait PixelFilter: Send + Sync {
    /// Transforms a single RGBA pixel `(r, g, b, a)` in `0..=255` range.
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8);
}

/// Wraps a [`PixelFilter`] so it can be used as either a [`GlitchFilter`] (PNG)
/// or a [`WebpGlitchFilter`] (WebP).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelFilterAdapter<F: PixelFilter>(pub F);

/// A struct that manages the glitch context for PNG images.
pub struct GlitchContext {
    png: PngGlitch,
    filters: Vec<Box<dyn GlitchFilter>>,
    rng: ChaCha8Rng,
}

impl GlitchContext {
    /// Creates a new [`GlitchContext`] from a file path.
    pub fn open(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self> {
        Self::open_with_options(path, seed, ParseOptions::default())
    }

    /// Creates a new [`GlitchContext`] from a file path using custom PNG [`ParseOptions`].
    pub fn open_with_options(
        path: impl AsRef<Path>,
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        let path = path.as_ref();
        let png = PngGlitch::open_with_options(path, options)
            .with_context(|| format!("failed to open '{}'", path.display()))?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
        Ok(Self {
            png,
            filters: Vec::new(),
            rng,
        })
    }

    /// Creates a new [`GlitchContext`] from a byte slice.
    pub fn new(data: &[u8], seed: Option<u64>) -> Result<Self> {
        Self::new_with_options(data, seed, ParseOptions::default())
    }

    /// Creates a new [`GlitchContext`] from a byte slice using custom PNG [`ParseOptions`].
    pub fn new_with_options(
        data: &[u8],
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        let png =
            PngGlitch::from_bytes_with_options(data, options).context("Failed to parse PNG data")?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
        Ok(Self {
            png,
            filters: Vec::new(),
            rng,
        })
    }

    /// Returns any non-fatal PNG integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        self.png.warnings()
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    pub fn set_seed(&mut self, seed: Option<u64>) {
        self.rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
    }

    /// Adds a filter to the execution queue.
    pub fn add_filter(&mut self, filter: impl GlitchFilter + 'static) {
        self.filters.push(Box::new(filter));
    }

    /// Clears all queued filters without executing them.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    /// Executes all queued filters on the image in registration order and drains the filter queue.
    ///
    /// Calling `execute()` a second time without adding new filters is a no-op.
    pub fn execute(&mut self) {
        for filter in self.filters.drain(..) {
            filter.apply(&mut self.png, &mut self.rng);
        }
    }

    /// Returns the encoded glitched image as a byte vector.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        self.png.to_bytes().context("Failed to encode PNG")
    }

    /// Saves the glitched image to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.png
            .save(path)
            .with_context(|| format!("failed to save '{}'", path.display()))?;
        Ok(())
    }

    /// Returns the width of the image in pixels.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// Re-encodes all scanlines using the specified PNG prediction filter type.
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.png.change_filter_type(filter_type);
    }

    /// Queues a pre-process filter that re-encodes all scanlines with a uniform PNG filter type.
    pub fn pre_process(&mut self, pre_process: PreProcess) {
        match pre_process {
            PreProcess::RemoveFilter => self.add_filter(RemoveFilter),
            PreProcess::SubFilter => self.add_filter(SubFilter),
            PreProcess::UpFilter => self.add_filter(UpFilter),
            PreProcess::AverageFilter => self.add_filter(AverageFilter),
            PreProcess::PaethFilter => self.add_filter(PaethFilter),
        }
    }

    /// Adds a filter from a [`FilterConfig`] definition.
    ///
    /// Returns an error if the filter configuration is not supported for PNG images
    /// (for example, WebP-only filters such as `MacroblockGlitch`, `AlphaGlitch`, or `LossyArtifact`).
    pub fn add_from_config(&mut self, config: FilterConfig) -> Result<()> {
        match config {
            FilterConfig::ChangeFilterType { magnitude } => {
                self.add_filter(ChangeFilterType::new(magnitude));
            }
            FilterConfig::Replace { magnitude } => {
                self.add_filter(Replace::new(magnitude));
            }
            FilterConfig::Transpose { magnitude } => {
                self.add_filter(Transpose::new(magnitude));
            }
            FilterConfig::SetZero { magnitude } => {
                self.add_filter(SetZero::new(magnitude));
            }
            FilterConfig::RandomCopy { times } => {
                if times > MAX_RANDOM_COPY_TIMES {
                    bail!(
                        "random-copy `times` must be <= {MAX_RANDOM_COPY_TIMES}, got {times}"
                    );
                }
                self.add_filter(RandomCopy::new(times));
            }
            FilterConfig::Substitute { index, value } => {
                self.add_filter(Substitute::new(index, value));
            }
            FilterConfig::PixelSort {
                magnitude,
                criterion,
            } => {
                let mut f = PixelSort::new(magnitude);
                if let Some(c) = criterion {
                    f.criterion = c;
                }
                self.add_filter(f);
            }
            FilterConfig::Bitwise {
                magnitude,
                op,
                value,
            } => {
                let mut f = Bitwise::new(magnitude);
                if let Some(o) = op {
                    f.op = o;
                }
                if let Some(v) = value {
                    f.value = v;
                }
                self.add_filter(f);
            }
            FilterConfig::ChannelSwap { magnitude, target } => {
                let mut f = ChannelSwap::new(magnitude);
                if let Some(t) = target {
                    f.target = t;
                }
                self.add_filter(f);
            }
            FilterConfig::HorizontalShift { magnitude } => {
                self.add_filter(HorizontalShift::new(magnitude));
            }
            FilterConfig::BlockScramble {
                magnitude,
                block_size,
            } => {
                let mut f = BlockScramble::new(magnitude);
                if let Some(bs) = block_size {
                    f.block_size = bs;
                }
                self.add_filter(f);
            }
            FilterConfig::ColorDistortion {
                magnitude,
                strength,
            } => {
                let mut f = ColorDistortion::new(magnitude);
                if let Some(s) = strength {
                    if s < 0 {
                        bail!("color-distortion `strength` must be >= 0, got {s}");
                    }
                    f.strength = s;
                }
                self.add_filter(f);
            }
            FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift,
                saturation_mult,
                lightness_mult,
            } => {
                let mut f = ColorSpaceGlitch::new(magnitude);
                if let Some(h) = hue_shift {
                    if !h.is_finite() {
                        bail!("color-space-glitch `hue_shift` must be a finite number, got {h}");
                    }
                    f.hue_shift = h;
                }
                if let Some(s) = saturation_mult {
                    if !s.is_finite() || s < 0.0 {
                        bail!(
                            "color-space-glitch `saturation_mult` must be a finite number >= 0.0, got {s}"
                        );
                    }
                    f.saturation_mult = s;
                }
                if let Some(l) = lightness_mult {
                    if !l.is_finite() || l < 0.0 {
                        bail!(
                            "color-space-glitch `lightness_mult` must be a finite number >= 0.0, got {l}"
                        );
                    }
                    f.lightness_mult = l;
                }
                self.add_filter(f);
            }
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            } => {
                let mut f = ChromaticAberration::new(magnitude);
                if let Some(r) = r_offset {
                    f.r_offset = r;
                }
                if let Some(g) = g_offset {
                    f.g_offset = g;
                }
                if let Some(b) = b_offset {
                    f.b_offset = b;
                }
                self.add_filter(f);
            }
            FilterConfig::Invert => {
                self.add_filter(Invert);
            }
            FilterConfig::Brighten { strength } => {
                self.add_filter(Brighten::new(strength));
            }
            FilterConfig::Darken { strength } => {
                self.add_filter(Darken::new(strength));
            }
            FilterConfig::ShiftChannels { r, g, b } => {
                self.add_filter(ShiftChannels::new(r, g, b));
            }
            FilterConfig::RemoveFilter => {
                self.add_filter(RemoveFilter);
            }
            FilterConfig::SubFilter => {
                self.add_filter(SubFilter);
            }
            FilterConfig::UpFilter => {
                self.add_filter(UpFilter);
            }
            FilterConfig::AverageFilter => {
                self.add_filter(AverageFilter);
            }
            FilterConfig::PaethFilter => {
                self.add_filter(PaethFilter);
            }
            FilterConfig::MacroblockGlitch { .. }
            | FilterConfig::AlphaGlitch { .. }
            | FilterConfig::LossyArtifact { .. } => {
                bail!(
                    "Filter `{:?}` is a WebP-only filter and is not supported for PNG images",
                    config
                );
            }
        }
        Ok(())
    }
}

fn clamp_magnitude(m: f64) -> f64 {
    if m.is_nan() {
        0.0
    } else {
        m.clamp(0.0, 1.0)
    }
}

fn deserialize_magnitude<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = f64::deserialize(deserializer)?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err(serde::de::Error::custom(format!(
            "magnitude must be between 0.0 and 1.0, got {val}"
        )))
    }
}

/// Pre-process options for re-encoding PNG scanlines with a uniform prediction filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum PreProcess {
    /// Re-encodes all scanlines with [`FilterType::None`].
    #[default]
    #[serde(alias = "RemoveFilter", alias = "removefilter", alias = "none")]
    RemoveFilter,
    /// Re-encodes all scanlines with [`FilterType::Sub`].
    #[serde(alias = "SubFilter", alias = "subfilter", alias = "sub")]
    SubFilter,
    /// Re-encodes all scanlines with [`FilterType::Up`].
    #[serde(alias = "UpFilter", alias = "upfilter", alias = "up")]
    UpFilter,
    /// Re-encodes all scanlines with [`FilterType::Average`].
    #[serde(alias = "AverageFilter", alias = "averagefilter", alias = "average")]
    AverageFilter,
    /// Re-encodes all scanlines with [`FilterType::Paeth`].
    #[serde(alias = "PaethFilter", alias = "paethfilter", alias = "paeth")]
    PaethFilter,
}

/// Strategy for corrupting the alpha channel in [`AlphaGlitch`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum AlphaGlitchStrategy {
    /// Inverts each alpha byte (`255 - a`).
    #[default]
    #[serde(alias = "Invert")]
    Invert,
    /// Replaces each alpha byte with a random `u8`.
    #[serde(alias = "Randomize")]
    Randomize,
    /// Sets each alpha byte to `0` (fully transparent).
    #[serde(alias = "Zero")]
    Zero,
    /// Sets each alpha byte to `255` (fully opaque).
    #[serde(alias = "One")]
    One,
}

/// Declarative configuration for a single glitch filter step (used in YAML config files and `-f/--filter`).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FilterConfig {
    /// Randomly overwrites the PNG filter type byte of scanlines.
    #[serde(alias = "change-filter-type", alias = "change_filter_type")]
    ChangeFilterType {
        /// Probability (`0.0..=1.0`) of changing each scanline's filter type byte.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Replaces random scanline bytes with random values.
    #[serde(alias = "replace")]
    Replace {
        /// Probability (`0.0..=1.0`) of replacing a byte in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Swaps random scanlines.
    #[serde(alias = "transpose", alias = "random-transpose", alias = "RandomTranspose")]
    Transpose {
        /// Probability (`0.0..=1.0`) of swapping each scanline with another random scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Sets random scanline bytes to `0`.
    #[serde(alias = "set-zero", alias = "set_zero")]
    SetZero {
        /// Probability (`0.0..=1.0`) of zeroing a byte in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Copies random scanlines onto other scanlines `times` times.
    #[serde(alias = "random-copy", alias = "random_copy")]
    RandomCopy {
        /// Number of random scanline copy operations to perform.
        times: u32,
    },
    /// Sets the byte at `index` to `value` on every scanline.
    #[serde(alias = "substitute")]
    Substitute {
        /// Byte offset within each scanline.
        index: usize,
        /// Replacement byte value.
        value: u8,
    },
    /// Sorts pixels within randomly selected scanlines.
    #[serde(alias = "pixel-sort", alias = "pixel_sort")]
    PixelSort {
        /// Probability (`0.0..=1.0`) of sorting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional sorting criterion (defaults to [`SortCriterion::Brightness`]).
        criterion: Option<SortCriterion>,
    },
    /// Applies a bitwise operation to bytes in randomly selected scanlines.
    #[serde(alias = "bitwise")]
    Bitwise {
        /// Probability (`0.0..=1.0`) of modifying each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional bitwise operator (defaults to [`BitOp::Xor`]).
        op: Option<BitOp>,
        /// Optional byte operand (defaults to `0`).
        value: Option<u8>,
    },
    /// Swaps two color channels in randomly selected scanlines.
    #[serde(alias = "channel-swap", alias = "channel_swap")]
    ChannelSwap {
        /// Probability (`0.0..=1.0`) of swapping channels in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional channel pair to swap (defaults to [`SwapTarget::Rg`]).
        target: Option<SwapTarget>,
    },
    /// Horizontally rotates scanline bytes by a random offset.
    #[serde(alias = "horizontal-shift", alias = "horizontal_shift")]
    HorizontalShift {
        /// Probability (`0.0..=1.0`) of shifting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Scrambles square blocks of pixels across the image.
    #[serde(alias = "block-scramble", alias = "block_scramble")]
    BlockScramble {
        /// Fraction (`0.0..=1.0`) of blocks to swap.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional block edge length in pixels (defaults to `16`).
        block_size: Option<u32>,
    },
    /// Adds random per-pixel color noise on randomly selected scanlines.
    #[serde(alias = "color-distortion", alias = "color_distortion")]
    ColorDistortion {
        /// Probability (`0.0..=1.0`) of distorting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional maximum noise amplitude (defaults to `20`).
        strength: Option<i16>,
    },
    /// Shifts hue and scales saturation/lightness in HSL color space.
    #[serde(alias = "color-space-glitch", alias = "color_space_glitch")]
    ColorSpaceGlitch {
        /// Probability (`0.0..=1.0`) of transforming each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional hue shift in degrees (defaults to `0.0`).
        hue_shift: Option<f64>,
        /// Optional saturation multiplier (defaults to `1.0`).
        saturation_mult: Option<f64>,
        /// Optional lightness multiplier (defaults to `1.0`).
        lightness_mult: Option<f64>,
    },
    /// Horizontally offsets R, G, and B channels on randomly selected scanlines.
    #[serde(alias = "chromatic-aberration", alias = "chromatic_aberration")]
    ChromaticAberration {
        /// Probability (`0.0..=1.0`) of applying chromatic aberration to each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional red channel horizontal offset in pixels (defaults to `2`).
        r_offset: Option<i32>,
        /// Optional green channel horizontal offset in pixels (defaults to `0`).
        g_offset: Option<i32>,
        /// Optional blue channel horizontal offset in pixels (defaults to `-2`).
        b_offset: Option<i32>,
    },
    /// Inverts all color channels.
    #[serde(alias = "invert")]
    Invert,
    /// Increases brightness by `strength` (saturating at the bit-depth maximum).
    #[serde(alias = "brighten")]
    Brighten {
        /// Amount added to each color channel.
        strength: u16,
    },
    /// Decreases brightness by `strength` (saturating at `0`).
    #[serde(alias = "darken")]
    Darken {
        /// Amount subtracted from each color channel.
        strength: u16,
    },
    /// Adds fixed signed offsets `(r, g, b)` to color channels.
    #[serde(alias = "shift-channels", alias = "shift_channels")]
    ShiftChannels {
        /// Red channel offset.
        r: i16,
        /// Green channel offset.
        g: i16,
        /// Blue channel offset.
        b: i16,
    },
    /// Re-encodes all scanlines with [`FilterType::None`].
    #[serde(alias = "remove-filter", alias = "remove_filter", alias = "none")]
    RemoveFilter,
    /// Re-encodes all scanlines with [`FilterType::Sub`].
    #[serde(alias = "sub-filter", alias = "sub_filter", alias = "sub")]
    SubFilter,
    /// Re-encodes all scanlines with [`FilterType::Up`].
    #[serde(alias = "up-filter", alias = "up_filter", alias = "up")]
    UpFilter,
    /// Re-encodes all scanlines with [`FilterType::Average`].
    #[serde(alias = "average-filter", alias = "average_filter", alias = "average")]
    AverageFilter,
    /// Re-encodes all scanlines with [`FilterType::Paeth`].
    #[serde(alias = "paeth-filter", alias = "paeth_filter", alias = "paeth")]
    PaethFilter,
    /// Swaps 16×16 macroblocks along VP8 block boundaries (WebP only).
    #[serde(alias = "macroblock-glitch", alias = "macroblock_glitch")]
    MacroblockGlitch {
        /// Fraction (`0.0..=1.0`) of macroblocks to swap.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Selectively corrupts the alpha channel (WebP only).
    #[serde(alias = "alpha-glitch", alias = "alpha_glitch")]
    AlphaGlitch {
        /// Probability (`0.0..=1.0`) of corrupting each scanline's alpha channel (defaults to `1.0`).
        #[serde(default = "default_alpha_magnitude", deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional alpha corruption strategy (defaults to [`AlphaGlitchStrategy::Invert`]).
        strategy: Option<AlphaGlitchStrategy>,
    },
    /// Lowers WebP re-encoding quality (`0.0..=100.0`) to introduce lossy compression artifacts (WebP only).
    #[serde(alias = "lossy-artifact", alias = "lossy_artifact", alias = "lossy-quality", alias = "lossy_quality")]
    LossyArtifact {
        /// Re-encoding quality in `0.0..=100.0`.
        quality: f32,
    },
}

fn default_alpha_magnitude() -> f64 {
    1.0
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

impl GlitchFilter for ChangeFilterType {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                let filter_type = match rng.random_range(0..5) {
                    0 => FilterType::None,
                    1 => FilterType::Sub,
                    2 => FilterType::Up,
                    3 => FilterType::Average,
                    _ => FilterType::Paeth,
                };
                scan_line.set_filter_type(filter_type);
            }
        });
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

impl GlitchFilter for RemoveFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.change_filter_type(FilterType::None);
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

impl GlitchFilter for SubFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.change_filter_type(FilterType::Sub);
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

impl GlitchFilter for UpFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.change_filter_type(FilterType::Up);
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

impl GlitchFilter for AverageFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.change_filter_type(FilterType::Average);
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

impl GlitchFilter for PaethFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.change_filter_type(FilterType::Paeth);
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

impl GlitchFilter for Replace {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            let size = scan_line.size();
            if size == 0 {
                return;
            }
            if rng.random_bool(magnitude) {
                let val: u8 = rng.random();
                let index = rng.random_range(0..size);
                scan_line.update(index, val);
            }
        });
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

/// Alias for [`Transpose`] to distinguish the randomized glitch effect from [`png_glitch::Transpose`].
pub type RandomTranspose = Transpose;

impl GlitchFilter for Transpose {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        let height = png.height();
        if height <= 1 {
            return;
        }
        for i in 0..height {
            if rng.random_bool(magnitude) {
                let target = rng.random_range(0..height);
                if i != target {
                    png.transpose(i, target, 1);
                }
            }
        }
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

impl GlitchFilter for RandomCopy {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let height = png.height();
        if height <= 1 {
            return;
        }
        let times = self.times.min(MAX_RANDOM_COPY_TIMES);
        let mut scan_lines = png.scan_lines();
        let index_range = 0..height as usize;
        for _ in 0..times {
            let src_idx = rng.random_range(index_range.clone());
            let dest_idx = rng.random_range(index_range.clone());

            if src_idx == dest_idx {
                continue;
            }

            let (filter_type, buffer) = {
                let src = &mut scan_lines[src_idx];
                let filter_type = src.filter_type();
                let mut buffer = vec![0; src.size()];
                use std::io::Read;
                src.read_exact(&mut buffer).unwrap();
                (filter_type, buffer)
            };

            let dest = &mut scan_lines[dest_idx];
            use std::io::Write;
            dest.write_all(&buffer).unwrap();
            dest.set_filter_type(filter_type);
        }
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

impl GlitchFilter for Substitute {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.foreach_scanline(|scanline| {
            if self.index < scanline.size() {
                scanline.update(self.index, self.value);
            }
        });
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

impl GlitchFilter for SetZero {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            let size = scan_line.size();
            if size == 0 {
                return;
            }
            if rng.random_bool(magnitude) {
                let index = rng.random_range(0..size);
                scan_line.update(index, 0);
            }
        });
    }
}

/// Criterion used by [`PixelSort`] to order pixels within a scanline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
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

impl GlitchFilter for PixelSort {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                let mut pixels = Vec::with_capacity(scan_line.pixels_count() as usize);
                for i in 0..scan_line.pixels_count() as usize {
                    if let Some(p) = scan_line.get_pixel(i) {
                        pixels.push(p);
                    }
                }

                pixels.sort_by(|a, b| {
                    let val_a = self.get_value(a);
                    let val_b = self.get_value(b);
                    val_a.partial_cmp(&val_b).unwrap_or(std::cmp::Ordering::Equal)
                });

                for (i, p) in pixels.into_iter().enumerate() {
                    scan_line.set_pixel(i, p);
                }
            }
        });
    }
}

impl PixelSort {
    fn get_value(&self, p: &Pixel) -> f64 {
        match self.criterion {
            SortCriterion::Brightness => {
                (p.r() as f64 + p.g() as f64 + p.b() as f64) / 3.0
            }
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

/// Bitwise operation performed by [`Bitwise`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
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

impl GlitchFilter for Bitwise {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                for i in 0..scan_line.size() {
                    if let Some(b) = scan_line.index(i) {
                        let new_b = match self.op {
                            BitOp::And => b & self.value,
                            BitOp::Or => b | self.value,
                            BitOp::Xor => b ^ self.value,
                        };
                        scan_line.update(i, new_b);
                    }
                }
            }
        });
    }
}

/// Pair of color channels to swap in [`ChannelSwap`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
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

impl GlitchFilter for ChannelSwap {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                scan_line.process_pixels(|_, p| match p {
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
            }
        });
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

impl GlitchFilter for HorizontalShift {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            let size = scan_line.size();
            if size <= 1 {
                return;
            }
            if rng.random_bool(magnitude) {
                let shift = rng.random_range(0..size);
                let mut buffer = vec![0; size];
                for (i, byte) in buffer.iter_mut().enumerate().take(size) {
                    *byte = scan_line.index(i).unwrap();
                }

                for (i, &byte) in buffer.iter().enumerate().take(size) {
                    let new_idx = (i + shift) % size;
                    scan_line.update(new_idx, byte);
                }
            }
        });
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

impl GlitchFilter for BlockScramble {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
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

        let total_blocks = (num_blocks_x * num_blocks_y) as usize;
        let num_scrambles = (total_blocks as f64 * clamp_magnitude(self.magnitude)) as usize;

        let mut scan_lines = png.scan_lines();

        for _ in 0..num_scrambles {
            let src_bx = rng.random_range(0..num_blocks_x);
            let src_by = rng.random_range(0..num_blocks_y);
            let dst_bx = rng.random_range(0..num_blocks_x);
            let dst_by = rng.random_range(0..num_blocks_y);

            if src_bx == dst_bx && src_by == dst_by {
                continue;
            }

            // Swap blocks line by line
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

impl GlitchFilter for ColorDistortion {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let strength = self.strength.saturating_abs();
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                scan_line.process_pixels(|_, p| {
                    let r_off = rng.random_range(-strength..=strength);
                    let g_off = rng.random_range(-strength..=strength);
                    let b_off = rng.random_range(-strength..=strength);
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
        });
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

impl GlitchFilter for ColorSpaceGlitch {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
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
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                scan_line.process_pixels(|_, p| {
                    match p {
                        Pixel::RGB(r, g, b) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + hue_shift).rem_euclid(360.0);
                            let s = (s * sat_mult).clamp(0.0, 1.0);
                            let l = (l * light_mult).clamp(0.0, 1.0);
                            let (r, g, b) = hsl_to_rgb(h, s, l);
                            Pixel::RGB(r, g, b)
                        }
                        Pixel::RGBA(r, g, b, a) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + hue_shift).rem_euclid(360.0);
                            let s = (s * sat_mult).clamp(0.0, 1.0);
                            let l = (l * light_mult).clamp(0.0, 1.0);
                            let (r, g, b) = hsl_to_rgb(h, s, l);
                            Pixel::RGBA(r, g, b, a)
                        }
                        _ => p,
                    }
                });
            }
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
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
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
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
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
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    p
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

impl GlitchFilter for ChromaticAberration {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let width = png.width() as usize;
        if width == 0 {
            return;
        }
        let magnitude = clamp_magnitude(self.magnitude);
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                let mut r_channel = Vec::with_capacity(width);
                let mut g_channel = Vec::with_capacity(width);
                let mut b_channel = Vec::with_capacity(width);
                let mut alpha = Vec::with_capacity(width);

                for x in 0..width {
                    if let Some(p) = scan_line.get_pixel(x) {
                        r_channel.push(p.r());
                        g_channel.push(p.g());
                        b_channel.push(p.b());
                        alpha.push(p.a());
                    }
                }

                scan_line.process_pixels(|x, p| {
                    let get_val = |chan: &Vec<u16>, offset: i32| {
                        let nx = (x as i32 + offset).rem_euclid(width as i32) as usize;
                        chan[nx]
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
                            alpha[x],
                        ),
                        _ => p,
                    }
                });
            }
        });
    }
}

impl GlitchFilter for Invert {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.apply(*self);
    }
}

impl GlitchFilter for Brighten {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.apply(*self);
    }
}

impl GlitchFilter for Darken {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.apply(*self);
    }
}

impl GlitchFilter for ShiftChannels {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.apply(*self);
    }
}

// PixelFilter implementations for the shared presets

impl PixelFilter for Invert {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        (255 - r, 255 - g, 255 - b, a)
    }
}

impl PixelFilter for Brighten {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let s = self.strength.min(255) as u8;
        (r.saturating_add(s), g.saturating_add(s), b.saturating_add(s), a)
    }
}

impl PixelFilter for Darken {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let s = self.strength.min(255) as u8;
        (r.saturating_sub(s), g.saturating_sub(s), b.saturating_sub(s), a)
    }
}

impl PixelFilter for ShiftChannels {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
        (shift(r, self.r), shift(g, self.g), shift(b, self.b), a)
    }
}

// PixelFilterAdapter: applies a PixelFilter to PNG images

impl<F: PixelFilter> GlitchFilter for PixelFilterAdapter<F> {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.foreach_scanline(|line| {
            let max_val = ((1u32 << line.bit_depth() as u32) - 1) as u16;
            line.process_pixels(|_, pixel| {
                let scale_down = |v: u16| -> u8 {
                    if max_val == 255 { v as u8 } else { (v as u32 * 255 / max_val as u32) as u8 }
                };
                let scale_up = |v: u8| -> u16 {
                    if max_val == 255 { v as u16 } else { (v as u32 * max_val as u32 / 255) as u16 }
                };
                match pixel {
                    Pixel::RGB(r, g, b) => {
                        let (nr, ng, nb, _) = self.0.apply_pixel(scale_down(r), scale_down(g), scale_down(b), 255);
                        Pixel::RGB(scale_up(nr), scale_up(ng), scale_up(nb))
                    }
                    Pixel::RGBA(r, g, b, a) => {
                        let (nr, ng, nb, na) = self.0.apply_pixel(scale_down(r), scale_down(g), scale_down(b), scale_down(a));
                        Pixel::RGBA(scale_up(nr), scale_up(ng), scale_up(nb), scale_up(na))
                    }
                    Pixel::Gray(v) => {
                        let gv = scale_down(v);
                        let (nr, _, _, _) = self.0.apply_pixel(gv, gv, gv, 255);
                        Pixel::Gray(scale_up(nr))
                    }
                    Pixel::GrayAlpha(v, a) => {
                        let gv = scale_down(v);
                        let (nr, _, _, na) = self.0.apply_pixel(gv, gv, gv, scale_down(a));
                        Pixel::GrayAlpha(scale_up(nr), scale_up(na))
                    }
                    other => other,
                }
            });
        });
    }
}

// ============================================================
// WebP filter implementations (requires `webp` feature)
// ============================================================

#[cfg(feature = "webp")]
pub use webp_glitch::{WebpError, WebpGlitch, WebpGlitchFilter, WebpPixel};

/// Glitch context for WebP images.
#[cfg(feature = "webp")]
pub struct WebpGlitchContext {
    webp: WebpGlitch,
    filters: Vec<Box<dyn WebpGlitchFilter>>,
    rng: ChaCha8Rng,
}

#[cfg(feature = "webp")]
impl WebpGlitchContext {
    /// Creates a new [`WebpGlitchContext`] from a file path.
    pub fn open(path: impl AsRef<std::path::Path>, seed: Option<u64>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let webp = WebpGlitch::open(path)
            .with_context(|| format!("failed to open '{}'", path.display()))?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
        Ok(Self { webp, filters: Vec::new(), rng })
    }

    /// Creates a new [`WebpGlitchContext`] from a byte slice.
    pub fn new(data: &[u8], seed: Option<u64>) -> anyhow::Result<Self> {
        let webp = WebpGlitch::new(data.to_vec())?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
        Ok(Self { webp, filters: Vec::new(), rng })
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    pub fn set_seed(&mut self, seed: Option<u64>) {
        self.rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => ChaCha8Rng::from_os_rng(),
        };
    }

    /// Adds a filter to the execution queue.
    pub fn add_filter(&mut self, filter: impl WebpGlitchFilter + 'static) {
        self.filters.push(Box::new(filter));
    }

    /// Clears all queued filters without executing them.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    /// Executes all queued filters on the image in registration order and drains the filter queue.
    ///
    /// Calling `execute()` a second time without adding new filters is a no-op.
    pub fn execute(&mut self) {
        for filter in self.filters.drain(..) {
            filter.apply(&mut self.webp, &mut self.rng);
        }
    }

    /// Returns the encoded glitched WebP image as a byte vector.
    pub fn buffer(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.webp.encode()?)
    }

    /// Saves the glitched WebP image to a file.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let data = self.buffer()?;
        std::fs::write(path, data)
            .with_context(|| format!("failed to save '{}'", path.display()))?;
        Ok(())
    }

    /// Returns the width of the image in pixels.
    pub fn width(&self) -> u32 { self.webp.width() }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> u32 { self.webp.height() }

    /// Adds a filter from a [`FilterConfig`] definition.
    ///
    /// Returns an error if the filter configuration is not supported for WebP images
    /// (for example, PNG scanline filter manipulations).
    pub fn add_from_config(&mut self, config: FilterConfig) -> anyhow::Result<()> {
        match config {
            FilterConfig::Invert => self.add_filter(PixelFilterAdapter(Invert)),
            FilterConfig::Brighten { strength } => {
                self.add_filter(PixelFilterAdapter(Brighten::new(strength)));
            }
            FilterConfig::Darken { strength } => {
                self.add_filter(PixelFilterAdapter(Darken::new(strength)));
            }
            FilterConfig::ShiftChannels { r, g, b } => {
                self.add_filter(PixelFilterAdapter(ShiftChannels::new(r, g, b)));
            }
            FilterConfig::PixelSort {
                magnitude,
                criterion,
            } => {
                let mut f = PixelSort::new(magnitude);
                if let Some(c) = criterion {
                    f.criterion = c;
                }
                self.add_filter(f);
            }
            FilterConfig::Bitwise {
                magnitude,
                op,
                value,
            } => {
                let mut f = Bitwise::new(magnitude);
                if let Some(o) = op {
                    f.op = o;
                }
                if let Some(v) = value {
                    f.value = v;
                }
                self.add_filter(f);
            }
            FilterConfig::ChannelSwap { magnitude, target } => {
                let mut f = ChannelSwap::new(magnitude);
                if let Some(t) = target {
                    f.target = t;
                }
                self.add_filter(f);
            }
            FilterConfig::HorizontalShift { magnitude } => {
                self.add_filter(HorizontalShift::new(magnitude));
            }
            FilterConfig::ColorDistortion {
                magnitude,
                strength,
            } => {
                let mut f = ColorDistortion::new(magnitude);
                if let Some(s) = strength {
                    if s < 0 {
                        bail!("color-distortion `strength` must be >= 0, got {s}");
                    }
                    f.strength = s;
                }
                self.add_filter(f);
            }
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            } => {
                let mut f = ChromaticAberration::new(magnitude);
                if let Some(r) = r_offset {
                    f.r_offset = r;
                }
                if let Some(g) = g_offset {
                    f.g_offset = g;
                }
                if let Some(b) = b_offset {
                    f.b_offset = b;
                }
                self.add_filter(f);
            }
            FilterConfig::MacroblockGlitch { magnitude } => {
                self.add_filter(MacroblockGlitch::new(magnitude));
            }
            FilterConfig::AlphaGlitch {
                magnitude,
                strategy,
            } => {
                let mut f = AlphaGlitch::new(magnitude);
                if let Some(s) = strategy {
                    f.strategy = s;
                }
                self.add_filter(f);
            }
            FilterConfig::LossyArtifact { quality } => {
                self.add_filter(LossyArtifact::new(quality));
            }
            other => {
                bail!(
                    "Filter `{:?}` is a PNG-only filter and is not supported for WebP images",
                    other
                );
            }
        }
        Ok(())
    }
}

// PixelFilterAdapter: applies a PixelFilter to WebP images

#[cfg(feature = "webp")]
impl<F: PixelFilter> WebpGlitchFilter for PixelFilterAdapter<F> {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn RngCore) {
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| {
                match p {
                    WebpPixel::RGB(r, g, b) => {
                        let (nr, ng, nb, _) = self.0.apply_pixel(r, g, b, 255);
                        WebpPixel::RGB(nr, ng, nb)
                    }
                    WebpPixel::RGBA(r, g, b, a) => {
                        let (nr, ng, nb, na) = self.0.apply_pixel(r, g, b, a);
                        WebpPixel::RGBA(nr, ng, nb, na)
                    }
                }
            });
        });
    }
}

/// WebP version of the [`Invert`] filter.
#[cfg(feature = "webp")]
#[deprecated(note = "Use `PixelFilterAdapter(Invert)` instead, which works on both PNG and WebP.")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpInvert;

#[cfg(feature = "webp")]
#[allow(deprecated)]
impl WebpGlitchFilter for WebpInvert {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn RngCore) {
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        });
    }
}

/// WebP version of the [`Brighten`] filter.
#[cfg(feature = "webp")]
#[deprecated(note = "Use `PixelFilterAdapter(Brighten { strength })` instead, which works on both PNG and WebP.")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpBrighten {
    /// Brightness increment added to each channel (saturating at `255`).
    pub strength: u8,
}

#[cfg(feature = "webp")]
#[allow(deprecated)]
impl WebpGlitchFilter for WebpBrighten {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn RngCore) {
        let s = self.strength;
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(
                    r.saturating_add(s),
                    g.saturating_add(s),
                    b.saturating_add(s),
                ),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(
                    r.saturating_add(s),
                    g.saturating_add(s),
                    b.saturating_add(s),
                    a,
                ),
            });
        });
    }
}

/// WebP version of the [`ShiftChannels`] filter.
#[cfg(feature = "webp")]
#[deprecated(note = "Use `PixelFilterAdapter(ShiftChannels { r, g, b })` instead, which works on both PNG and WebP.")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpShiftChannels {
    /// Signed offset added to the red channel.
    pub r: i16,
    /// Signed offset added to the green channel.
    pub g: i16,
    /// Signed offset added to the blue channel.
    pub b: i16,
}

#[cfg(feature = "webp")]
#[allow(deprecated)]
impl WebpGlitchFilter for WebpShiftChannels {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn RngCore) {
        let r_off = self.r;
        let g_off = self.g;
        let b_off = self.b;
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| {
                let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
                match p {
                    WebpPixel::RGB(r, g, b) => {
                        WebpPixel::RGB(shift(r, r_off), shift(g, g_off), shift(b, b_off))
                    }
                    WebpPixel::RGBA(r, g, b, a) => {
                        WebpPixel::RGBA(shift(r, r_off), shift(g, g_off), shift(b, b_off), a)
                    }
                }
            });
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for PixelSort {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                let count = line.pixels_count() as usize;
                let mut pixels: Vec<WebpPixel> = (0..count)
                    .filter_map(|x| line.get_pixel(x))
                    .collect();
                pixels.sort_by(|a, b| {
                    let val = |p: &WebpPixel| match self.criterion {
                        SortCriterion::Brightness => {
                            (p.r() as f64 + p.g() as f64 + p.b() as f64) / 3.0
                        }
                        SortCriterion::Hue => {
                            let r = p.r() as f64 / 255.0;
                            let g = p.g() as f64 / 255.0;
                            let b = p.b() as f64 / 255.0;
                            let max = r.max(g).max(b);
                            let min = r.min(g).min(b);
                            if max == min {
                                0.0
                            } else if max == r {
                                (60.0 * ((g - b) / (max - min)) + 360.0) % 360.0
                            } else if max == g {
                                60.0 * ((b - r) / (max - min)) + 120.0
                            } else {
                                60.0 * ((r - g) / (max - min)) + 240.0
                            }
                        }
                    };
                    val(a)
                        .partial_cmp(&val(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for (x, p) in pixels.into_iter().enumerate() {
                    line.set_pixel(x, p);
                }
            }
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for Bitwise {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
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
            }
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for ChannelSwap {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| match p {
                    WebpPixel::RGB(r, g, b) => match self.target {
                        SwapTarget::Rg => WebpPixel::RGB(g, r, b),
                        SwapTarget::Gb => WebpPixel::RGB(r, b, g),
                        SwapTarget::Br => WebpPixel::RGB(b, g, r),
                    },
                    WebpPixel::RGBA(r, g, b, a) => match self.target {
                        SwapTarget::Rg => WebpPixel::RGBA(g, r, b, a),
                        SwapTarget::Gb => WebpPixel::RGBA(r, b, g, a),
                        SwapTarget::Br => WebpPixel::RGBA(b, g, r, a),
                    },
                });
            }
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for HorizontalShift {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            let size = line.size();
            if size <= 1 {
                return;
            }
            if rng.random_bool(magnitude) {
                let shift = rng.random_range(0..size);
                let mut buf: Vec<u8> = (0..size).filter_map(|i| line.index(i)).collect();
                buf.rotate_left(shift);
                for (i, b) in buf.into_iter().enumerate() {
                    line.update(i, b);
                }
            }
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for ColorDistortion {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let strength = self.strength.saturating_abs().min(255);
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| {
                    let r_off = rng.random_range(-strength..=strength);
                    let g_off = rng.random_range(-strength..=strength);
                    let b_off = rng.random_range(-strength..=strength);
                    let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
                    match p {
                        WebpPixel::RGB(r, g, b) => {
                            WebpPixel::RGB(shift(r, r_off), shift(g, g_off), shift(b, b_off))
                        }
                        WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(
                            shift(r, r_off),
                            shift(g, g_off),
                            shift(b, b_off),
                            a,
                        ),
                    }
                });
            }
        });
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for ChromaticAberration {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        let width = webp.width() as usize;
        if width == 0 {
            return;
        }
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                let mut r_ch = Vec::with_capacity(width);
                let mut g_ch = Vec::with_capacity(width);
                let mut b_ch = Vec::with_capacity(width);
                let mut a_ch = Vec::with_capacity(width);
                for x in 0..width {
                    if let Some(p) = line.get_pixel(x) {
                        r_ch.push(p.r());
                        g_ch.push(p.g());
                        b_ch.push(p.b());
                        a_ch.push(p.a());
                    }
                }
                let get = |ch: &Vec<u8>, off: i32, x: usize| {
                    let nx = (x as i32 + off).rem_euclid(width as i32) as usize;
                    ch[nx]
                };
                line.process_pixels(|x, p| {
                    let r = get(&r_ch, self.r_offset, x);
                    let g = get(&g_ch, self.g_offset, x);
                    let b = get(&b_ch, self.b_offset, x);
                    match p {
                        WebpPixel::RGB(_, _, _) => WebpPixel::RGB(r, g, b),
                        WebpPixel::RGBA(_, _, _, _) => WebpPixel::RGBA(r, g, b, a_ch[x]),
                    }
                });
            }
        });
    }
}

// ============================================================
// WebP-specific filters
// ============================================================

/// Filter that swaps 16×16 macroblocks along VP8 block boundaries in WebP images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroblockGlitch {
    /// Fraction (`0.0..=1.0`) of 16×16 macroblocks to swap.
    pub magnitude: f64,
}

impl Default for MacroblockGlitch {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl MacroblockGlitch {
    /// Creates a new [`MacroblockGlitch`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for MacroblockGlitch {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        const MB: u32 = 16;
        let width = webp.width();
        let height = webp.height();
        let num_x = width / MB;
        let num_y = height / MB;
        if num_x == 0 || num_y == 0 {
            return;
        }
        let total = (num_x * num_y) as usize;
        let swaps = (total as f64 * clamp_magnitude(self.magnitude)) as usize;
        let mut lines = webp.scan_lines();

        for _ in 0..swaps {
            let sx = rng.random_range(0..num_x);
            let sy = rng.random_range(0..num_y);
            let dx = rng.random_range(0..num_x);
            let dy = rng.random_range(0..num_y);
            if sx == dx && sy == dy {
                continue;
            }
            for row_off in 0..MB {
                let src_y = sy * MB + row_off;
                let dst_y = dy * MB + row_off;
                if src_y >= height || dst_y >= height {
                    continue;
                }
                if src_y == dst_y {
                    let line = &mut lines[src_y as usize];
                    for col_off in 0..MB {
                        let src_x = (sx * MB + col_off) as usize;
                        let dst_x = (dx * MB + col_off) as usize;
                        if src_x < width as usize && dst_x < width as usize {
                            if let (Some(ps), Some(pd)) =
                                (line.get_pixel(src_x), line.get_pixel(dst_x))
                            {
                                line.set_pixel(src_x, pd);
                                line.set_pixel(dst_x, ps);
                            }
                        }
                    }
                } else {
                    let (src_line, dst_line) = if src_y < dst_y {
                        let (left, right) = lines.split_at_mut(dst_y as usize);
                        (&mut left[src_y as usize], &mut right[0])
                    } else {
                        let (left, right) = lines.split_at_mut(src_y as usize);
                        (&mut right[0], &mut left[dst_y as usize])
                    };
                    for col_off in 0..MB {
                        let src_x = (sx * MB + col_off) as usize;
                        let dst_x = (dx * MB + col_off) as usize;
                        if src_x < width as usize && dst_x < width as usize {
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
}

/// Filter that selectively corrupts the alpha channel of WebP images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaGlitch {
    /// Probability (`0.0..=1.0`) of corrupting each scanline's alpha channel.
    pub magnitude: f64,
    /// Strategy used to modify alpha channel values.
    pub strategy: AlphaGlitchStrategy,
}

impl Default for AlphaGlitch {
    fn default() -> Self {
        Self {
            magnitude: 1.0,
            strategy: AlphaGlitchStrategy::Invert,
        }
    }
}

impl AlphaGlitch {
    /// Creates a new [`AlphaGlitch`] filter with the given magnitude and default [`AlphaGlitchStrategy::Invert`].
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the alpha corruption strategy.
    pub fn with_strategy(mut self, strategy: AlphaGlitchStrategy) -> Self {
        self.strategy = strategy;
        self
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for AlphaGlitch {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn RngCore) {
        if !webp.has_alpha() {
            return;
        }
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| match p {
                    WebpPixel::RGBA(r, g, b, a) => {
                        let new_a = match self.strategy {
                            AlphaGlitchStrategy::Invert => 255 - a,
                            AlphaGlitchStrategy::Randomize => rng.random(),
                            AlphaGlitchStrategy::Zero => 0,
                            AlphaGlitchStrategy::One => 255,
                        };
                        WebpPixel::RGBA(r, g, b, new_a)
                    }
                    other => other,
                });
            }
        });
    }
}

/// Filter that lowers WebP re-encoding quality (`0.0..=100.0`) to introduce lossy compression artifacts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossyArtifact {
    /// Target WebP re-encoding quality in `0.0..=100.0`.
    pub quality: f32,
}

impl Default for LossyArtifact {
    fn default() -> Self {
        Self { quality: 10.0 }
    }
}

impl LossyArtifact {
    /// Creates a new [`LossyArtifact`] filter with the given quality (`0.0..=100.0`).
    pub fn new(quality: f32) -> Self {
        Self { quality }
    }
}

#[cfg(feature = "webp")]
impl WebpGlitchFilter for LossyArtifact {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn RngCore) {
        webp.set_quality(self.quality);
    }
}

// ============================================================
// Format-agnostic unified pipeline
// ============================================================

/// Image format detected from magic bytes or file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Portable Network Graphics (`.png`).
    Png,
    /// WebP image format (`.webp`).
    Webp,
}

/// Detects the [`ImageFormat`] of an in-memory image buffer from its magic bytes.
///
/// - PNG: starts with `\x89PNG\r\n\x1a\n`
/// - WebP: starts with `RIFF` (bytes `0..4`) and `WEBP` (bytes `8..12`)
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

/// Format-agnostic glitch context that dispatches to either [`GlitchContext`] (PNG)
/// or [`WebpGlitchContext`] (WebP).
pub enum AnyGlitchContext {
    /// PNG glitch context.
    Png(GlitchContext),
    /// WebP glitch context (available when the `webp` feature is enabled).
    #[cfg(feature = "webp")]
    Webp(WebpGlitchContext),
}

/// Opens a PNG or WebP image file by inspecting its magic bytes (falling back to file extension)
/// and returns a unified [`AnyGlitchContext`].
pub fn open_any(path: impl AsRef<Path>, seed: Option<u64>) -> Result<AnyGlitchContext> {
    AnyGlitchContext::open(path, seed)
}

impl AnyGlitchContext {
    /// Opens an image file, detecting PNG vs WebP from its magic bytes (with extension fallback).
    pub fn open(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self> {
        Self::open_with_options(path, seed, ParseOptions::default())
    }

    /// Opens an image file with custom PNG [`ParseOptions`].
    pub fn open_with_options(
        path: impl AsRef<Path>,
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read(path)
            .with_context(|| format!("failed to open '{}'", path.display()))?;
        let format = detect_format(&data).or_else(|| {
            let ext = path.extension()?.to_str()?;
            if ext.eq_ignore_ascii_case("webp") {
                Some(ImageFormat::Webp)
            } else if ext.eq_ignore_ascii_case("png") {
                Some(ImageFormat::Png)
            } else {
                None
            }
        });
        match format {
            Some(ImageFormat::Png) | None => {
                let ctx = GlitchContext::new_with_options(&data, seed, options)
                    .with_context(|| format!("failed to open '{}'", path.display()))?;
                Ok(Self::Png(ctx))
            }
            Some(ImageFormat::Webp) => {
                #[cfg(feature = "webp")]
                {
                    let ctx = WebpGlitchContext::new(&data, seed)
                        .with_context(|| format!("failed to open '{}'", path.display()))?;
                    Ok(Self::Webp(ctx))
                }
                #[cfg(not(feature = "webp"))]
                {
                    bail!(
                        "failed to open '{}': WebP support requires the `webp` feature on `glitch-context`",
                        path.display()
                    )
                }
            }
        }
    }

    /// Creates an [`AnyGlitchContext`] from an in-memory image buffer by detecting its magic bytes.
    pub fn from_bytes(data: &[u8], seed: Option<u64>) -> Result<Self> {
        Self::from_bytes_with_options(data, seed, ParseOptions::default())
    }

    /// Creates an [`AnyGlitchContext`] from an in-memory image buffer with custom PNG [`ParseOptions`].
    pub fn from_bytes_with_options(
        data: &[u8],
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        match detect_format(data) {
            Some(ImageFormat::Png) => Ok(Self::Png(GlitchContext::new_with_options(
                data, seed, options,
            )?)),
            Some(ImageFormat::Webp) => {
                #[cfg(feature = "webp")]
                {
                    Ok(Self::Webp(WebpGlitchContext::new(data, seed)?))
                }
                #[cfg(not(feature = "webp"))]
                {
                    bail!("WebP support requires the `webp` feature on `glitch-context`")
                }
            }
            None => bail!("unrecognized image format (expected PNG or WebP magic bytes)"),
        }
    }

    /// Returns any non-fatal PNG integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        match self {
            Self::Png(ctx) => ctx.warnings(),
            #[cfg(feature = "webp")]
            Self::Webp(_) => &[],
        }
    }

    /// Returns the detected [`ImageFormat`] of this context.
    pub fn format(&self) -> ImageFormat {
        match self {
            Self::Png(_) => ImageFormat::Png,
            #[cfg(feature = "webp")]
            Self::Webp(_) => ImageFormat::Webp,
        }
    }

    /// Returns `true` if the underlying image is a WebP image.
    pub fn is_webp(&self) -> bool {
        self.format() == ImageFormat::Webp
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    pub fn set_seed(&mut self, seed: Option<u64>) {
        match self {
            Self::Png(ctx) => ctx.set_seed(seed),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.set_seed(seed),
        }
    }

    /// Adds a [`PixelFilter`] (wrapped in [`PixelFilterAdapter`]) that works on both PNG and WebP.
    pub fn add_pixel_filter(&mut self, filter: impl PixelFilter + 'static) {
        match self {
            Self::Png(ctx) => ctx.add_filter(PixelFilterAdapter(filter)),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.add_filter(PixelFilterAdapter(filter)),
        }
    }

    /// Adds a filter from a [`FilterConfig`] definition, returning an error if the filter is not
    /// supported for the underlying image format.
    pub fn add_from_config(&mut self, config: FilterConfig) -> Result<()> {
        match self {
            Self::Png(ctx) => ctx.add_from_config(config),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.add_from_config(config),
        }
    }

    /// Applies a PNG pre-process filter, or returns an error if the underlying image is WebP.
    pub fn pre_process(&mut self, pre_process: PreProcess) -> Result<()> {
        match self {
            Self::Png(ctx) => {
                ctx.pre_process(pre_process);
                Ok(())
            }
            #[cfg(feature = "webp")]
            Self::Webp(_) => bail!("Pre-process filter `{pre_process:?}` is only supported for PNG images"),
        }
    }

    /// Executes all queued filters and drains the filter queue.
    pub fn execute(&mut self) {
        match self {
            Self::Png(ctx) => ctx.execute(),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.execute(),
        }
    }

    /// Encodes the glitched image into a byte vector.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        match self {
            Self::Png(ctx) => ctx.buffer(),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.buffer(),
        }
    }

    /// Saves the glitched image to `path`.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        match self {
            Self::Png(ctx) => ctx.save(path),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.save(path),
        }
    }

    /// Returns the image width in pixels.
    pub fn width(&self) -> u32 {
        match self {
            Self::Png(ctx) => ctx.width(),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.width(),
        }
    }

    /// Returns the image height in pixels.
    pub fn height(&self) -> u32 {
        match self {
            Self::Png(ctx) => ctx.height(),
            #[cfg(feature = "webp")]
            Self::Webp(ctx) => ctx.height(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filters() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");

        // Test SubFilter
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(SubFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Sub, line.filter_type());
        }

        // Test UpFilter
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(UpFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Up, line.filter_type());
        }

        // Test AverageFilter
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(AverageFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Average, line.filter_type());
        }

        // Test PaethFilter
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(PaethFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Paeth, line.filter_type());
        }

        // Test RemoveFilter (None)
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(SubFilter);
        ctx.add_filter(RemoveFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::None, line.filter_type());
        }

        // Test Substitute
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(Substitute::new(0, 255));
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(line.index(0), Some(255));
        }

        // Test RandomCopy
        let mut ctx = GlitchContext::new(bytes, Some(12345))?;
        ctx.add_filter(RandomCopy::new(10));
        ctx.execute();
        let buffer = ctx.buffer()?;
        assert!(!buffer.is_empty());

        Ok(())
    }

    #[test]
    fn test_pixel_filter_adapter_on_png() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");

        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(PixelFilterAdapter(Invert));
        ctx.execute();
        let buffer = ctx.buffer()?;
        assert!(!buffer.is_empty(), "PixelFilterAdapter(Invert) should produce output");

        let mut ctx2 = GlitchContext::new(bytes, None)?;
        ctx2.add_filter(PixelFilterAdapter(Brighten::new(20)));
        ctx2.execute();
        let buffer2 = ctx2.buffer()?;
        assert!(!buffer2.is_empty(), "PixelFilterAdapter(Brighten) should produce output");

        Ok(())
    }

    #[test]
    fn test_execute_drains_filter_queue() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");
        let mut ctx = GlitchContext::new(bytes, Some(42))?;
        ctx.add_filter(Invert);
        ctx.execute();
        let after_first = ctx.buffer()?;

        // Calling execute() a second time without adding filters must be a no-op
        // (if Invert ran again, it would revert back to original bytes).
        ctx.execute();
        let after_second = ctx.buffer()?;
        assert_eq!(after_first, after_second);
        Ok(())
    }

    #[test]
    fn test_filter_defaults_and_constructors() {
        let bs = BlockScramble::new(0.3);
        assert_eq!(bs.block_size, 16);
        assert_eq!(bs.with_block_size(32).block_size, 32);

        let cd = ColorDistortion::new(0.4);
        assert_eq!(cd.strength, 20);
        assert_eq!(cd.with_strength(50).strength, 50);

        let ca = ChromaticAberration::new(0.25);
        assert_eq!((ca.r_offset, ca.g_offset, ca.b_offset), (2, 0, -2));

        let csg = ColorSpaceGlitch::new(0.5);
        assert_eq!(
            (csg.hue_shift, csg.saturation_mult, csg.lightness_mult),
            (0.0, 1.0, 1.0)
        );
    }

    #[test]
    fn test_any_glitch_context_detects_png_and_rejects_webp_filter() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");
        let mut ctx = AnyGlitchContext::from_bytes(bytes, Some(1))?;
        assert_eq!(ctx.format(), ImageFormat::Png);
        assert!(!ctx.is_webp());

        ctx.add_from_config(FilterConfig::Invert)?;
        let err = ctx
            .add_from_config(FilterConfig::MacroblockGlitch { magnitude: 0.5 })
            .unwrap_err();
        assert!(err.to_string().contains("WebP-only"));
        Ok(())
    }

    fn build_synthetic_png(
        width: u32,
        height: u32,
        color_type: u8,
        bit_depth: u8,
        plte: Option<&[u8]>,
    ) -> Vec<u8> {
        let samples_per_pixel: usize = match color_type {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => unreachable!(),
        };
        let bits_per_pixel = samples_per_pixel * (bit_depth as usize);
        let row_bytes = (width as usize * bits_per_pixel).div_ceil(8);
        let mut raw = Vec::with_capacity(height as usize * (1 + row_bytes));
        for y in 0..height as usize {
            raw.push(0); // FilterType::None
            for x in 0..row_bytes {
                raw.push(((x * 37 + y * 13 + 7) & 0xFF) as u8);
            }
        }
        let compressed = fdeflate::compress_to_vec(&raw);

        let write_chunk = |out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(tag);
            out.extend_from_slice(data);
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(tag);
            hasher.update(data);
            out.extend_from_slice(&hasher.finalize().to_be_bytes());
        };

        let mut png = Vec::from(&b"\x89PNG\r\n\x1a\n"[..]);
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(bit_depth);
        ihdr.push(color_type);
        ihdr.push(0); // compression
        ihdr.push(0); // filter
        ihdr.push(0); // interlace
        write_chunk(&mut png, b"IHDR", &ihdr);
        if let Some(palette) = plte {
            write_chunk(&mut png, b"PLTE", palette);
        }
        write_chunk(&mut png, b"IDAT", &compressed);
        write_chunk(&mut png, b"IEND", &[]);
        png
    }

    fn all_png_filter_configs(magnitude: f64) -> Vec<FilterConfig> {
        vec![
            FilterConfig::ChangeFilterType { magnitude },
            FilterConfig::Replace { magnitude },
            FilterConfig::Transpose { magnitude },
            FilterConfig::SetZero { magnitude },
            FilterConfig::RandomCopy {
                times: if magnitude == 0.0 { 0 } else { 4 },
            },
            FilterConfig::Substitute {
                index: 1,
                value: 42,
            },
            FilterConfig::PixelSort {
                magnitude,
                criterion: Some(SortCriterion::Brightness),
            },
            FilterConfig::PixelSort {
                magnitude,
                criterion: Some(SortCriterion::Hue),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::Xor),
                value: Some(0xAA),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::And),
                value: Some(0xF0),
            },
            FilterConfig::Bitwise {
                magnitude,
                op: Some(BitOp::Or),
                value: Some(0x0F),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Rg),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Gb),
            },
            FilterConfig::ChannelSwap {
                magnitude,
                target: Some(SwapTarget::Br),
            },
            FilterConfig::HorizontalShift { magnitude },
            FilterConfig::BlockScramble {
                magnitude,
                block_size: Some(4),
            },
            FilterConfig::ColorDistortion {
                magnitude,
                strength: Some(15),
            },
            FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift: Some(120.0),
                saturation_mult: Some(1.2),
                lightness_mult: Some(0.9),
            },
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset: Some(2),
                g_offset: Some(0),
                b_offset: Some(-2),
            },
            FilterConfig::Invert,
            FilterConfig::Brighten { strength: 10 },
            FilterConfig::Darken { strength: 10 },
            FilterConfig::ShiftChannels {
                r: 5,
                g: -5,
                b: 10,
            },
            FilterConfig::RemoveFilter,
            FilterConfig::SubFilter,
            FilterConfig::UpFilter,
            FilterConfig::AverageFilter,
            FilterConfig::PaethFilter,
        ]
    }

    #[test]
    fn test_all_png_filters_across_color_types_and_bit_depths() -> Result<()> {
        let palette: Vec<u8> = (0..=255).flat_map(|i: u8| [i, 255 - i, i / 2]).collect();
        let formats: &[(u8, u8, Option<&[u8]>)] = &[
            (0, 1, None),             // Grayscale 1-bit
            (0, 2, None),             // Grayscale 2-bit
            (0, 4, None),             // Grayscale 4-bit
            (0, 8, None),             // Grayscale 8-bit
            (0, 16, None),            // Grayscale 16-bit
            (2, 8, None),             // RGB 8-bit
            (2, 16, None),            // RGB 16-bit
            (3, 1, Some(&palette)),   // Indexed 1-bit
            (3, 2, Some(&palette)),   // Indexed 2-bit
            (3, 4, Some(&palette)),   // Indexed 4-bit
            (3, 8, Some(&palette)),   // Indexed 8-bit
            (4, 8, None),             // GrayAlpha 8-bit
            (4, 16, None),            // GrayAlpha 16-bit
            (6, 8, None),             // RGBA 8-bit
            (6, 16, None),            // RGBA 16-bit
        ];

        for &(color_type, bit_depth, plte) in formats {
            let png_bytes = build_synthetic_png(16, 16, color_type, bit_depth, plte);
            for cfg in all_png_filter_configs(1.0) {
                let is_raw_filter_override =
                    matches!(cfg, FilterConfig::ChangeFilterType { .. });
                let mut ctx = GlitchContext::new(&png_bytes, Some(99))?;
                ctx.add_from_config(cfg)?;
                ctx.execute();
                let out = ctx.buffer()?;
                assert!(!out.is_empty());

                // Verify round-trip decoding with standard `image` decoder for all filters
                // that preserve valid PNG filter type bytes.
                if !is_raw_filter_override {
                    let decoded = image::load_from_memory_with_format(&out, image::ImageFormat::Png)
                        .expect("glitched PNG should decode cleanly");
                    assert_eq!((decoded.width(), decoded.height()), (16, 16));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_boundary_magnitude_zero_leaves_scanlines_unchanged() -> Result<()> {
        let png_bytes = build_synthetic_png(16, 16, 6, 8, None);
        let baseline = GlitchContext::new(&png_bytes, Some(42))?.buffer()?;

        let magnitude_filters = vec![
            FilterConfig::ChangeFilterType { magnitude: 0.0 },
            FilterConfig::Replace { magnitude: 0.0 },
            FilterConfig::Transpose { magnitude: 0.0 },
            FilterConfig::SetZero { magnitude: 0.0 },
            FilterConfig::RandomCopy { times: 0 },
            FilterConfig::PixelSort {
                magnitude: 0.0,
                criterion: Some(SortCriterion::Brightness),
            },
            FilterConfig::Bitwise {
                magnitude: 0.0,
                op: Some(BitOp::Xor),
                value: Some(255),
            },
            FilterConfig::ChannelSwap {
                magnitude: 0.0,
                target: Some(SwapTarget::Rg),
            },
            FilterConfig::HorizontalShift { magnitude: 0.0 },
            FilterConfig::BlockScramble {
                magnitude: 0.0,
                block_size: Some(4),
            },
            FilterConfig::ColorDistortion {
                magnitude: 0.0,
                strength: Some(20),
            },
            FilterConfig::ColorSpaceGlitch {
                magnitude: 0.0,
                hue_shift: Some(90.0),
                saturation_mult: Some(1.5),
                lightness_mult: Some(1.5),
            },
            FilterConfig::ChromaticAberration {
                magnitude: 0.0,
                r_offset: Some(4),
                g_offset: Some(0),
                b_offset: Some(-4),
            },
        ];

        for cfg in magnitude_filters {
            let mut ctx = GlitchContext::new(&png_bytes, Some(42))?;
            ctx.add_from_config(cfg.clone())?;
            ctx.execute();
            assert_eq!(
                ctx.buffer()?,
                baseline,
                "magnitude 0.0 should leave image unchanged for {cfg:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_seed_reproducibility() -> Result<()> {
        let png_bytes = build_synthetic_png(32, 32, 6, 8, None);

        let run_with_seed = |seed: u64| -> Result<Vec<u8>> {
            let mut ctx = GlitchContext::new(&png_bytes, Some(seed))?;
            for cfg in all_png_filter_configs(0.5) {
                ctx.add_from_config(cfg)?;
            }
            ctx.execute();
            ctx.buffer()
        };

        let a1 = run_with_seed(2026)?;
        let a2 = run_with_seed(2026)?;
        let b = run_with_seed(2027)?;
        assert_eq!(a1, a2, "same seed must produce identical output bytes");
        assert_ne!(a1, b, "different seeds should produce different output bytes");
        Ok(())
    }

    #[test]
    fn test_yaml_deserialization_kebab_and_pascal_case() {
        let yaml = r#"
- type: change-filter-type
  magnitude: 0.1
- type: ChangeFilterType
  magnitude: 0.1
- type: replace
  magnitude: 0.2
- type: transpose
  magnitude: 0.3
- type: set-zero
  magnitude: 0.4
- type: random-copy
  times: 3
- type: substitute
  index: 2
  value: 128
- type: pixel-sort
  magnitude: 0.5
  criterion: hue
- type: bitwise
  magnitude: 0.5
  op: xor
  value: 255
- type: channel-swap
  magnitude: 0.5
  target: br
- type: horizontal-shift
  magnitude: 0.5
- type: block-scramble
  magnitude: 0.5
  block_size: 8
- type: color-distortion
  magnitude: 0.5
  strength: 30
- type: color-space-glitch
  magnitude: 0.5
  hue_shift: 45.0
  saturation_mult: 1.2
  lightness_mult: 0.8
- type: chromatic-aberration
  magnitude: 0.5
  r_offset: 3
  g_offset: 0
  b_offset: -3
- type: invert
- type: brighten
  strength: 15
- type: darken
  strength: 15
- type: shift-channels
  r: 10
  g: -10
  b: 5
- type: remove-filter
- type: sub-filter
- type: up-filter
- type: average-filter
- type: paeth-filter
- type: macroblock-glitch
  magnitude: 0.25
- type: alpha-glitch
  magnitude: 0.75
  strategy: randomize
- type: lossy-artifact
  quality: 15.0
"#;
        let configs: Vec<FilterConfig> =
            serde_yml::from_str(yaml).expect("all filter configs should deserialize");
        assert_eq!(configs.len(), 27);

        // Verify PreProcess accepts kebab-case, PascalCase, and lowercase aliases
        for s in ["remove-filter", "RemoveFilter", "removefilter", "none"] {
            let p: PreProcess = serde_yml::from_str(s).unwrap();
            assert_eq!(p, PreProcess::RemoveFilter);
        }
        for s in ["sub-filter", "SubFilter", "subfilter", "sub"] {
            let p: PreProcess = serde_yml::from_str(s).unwrap();
            assert_eq!(p, PreProcess::SubFilter);
        }
    }

    #[cfg(feature = "webp")]
    #[test]
    fn test_all_webp_filters_and_reproducibility() -> Result<()> {
        let img = image::RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([(x * 7) as u8, (y * 11) as u8, ((x + y) * 5) as u8, 200])
        });
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let webp_bytes = webp::Encoder::from_image(&dyn_img)
            .unwrap()
            .encode(90.0)
            .to_vec();

        let webp_configs = vec![
            FilterConfig::Invert,
            FilterConfig::Brighten { strength: 10 },
            FilterConfig::Darken { strength: 10 },
            FilterConfig::ShiftChannels { r: 5, g: -5, b: 0 },
            FilterConfig::PixelSort {
                magnitude: 0.5,
                criterion: Some(SortCriterion::Hue),
            },
            FilterConfig::Bitwise {
                magnitude: 0.5,
                op: Some(BitOp::Xor),
                value: Some(0x55),
            },
            FilterConfig::ChannelSwap {
                magnitude: 0.5,
                target: Some(SwapTarget::Gb),
            },
            FilterConfig::HorizontalShift { magnitude: 0.5 },
            FilterConfig::ColorDistortion {
                magnitude: 0.5,
                strength: Some(20),
            },
            FilterConfig::ChromaticAberration {
                magnitude: 0.5,
                r_offset: Some(2),
                g_offset: Some(0),
                b_offset: Some(-2),
            },
            FilterConfig::MacroblockGlitch { magnitude: 0.5 },
            FilterConfig::AlphaGlitch {
                magnitude: 0.5,
                strategy: Some(AlphaGlitchStrategy::Invert),
            },
            FilterConfig::LossyArtifact { quality: 30.0 },
        ];

        let run = |seed: u64| -> Result<Vec<u8>> {
            let mut ctx = AnyGlitchContext::from_bytes(&webp_bytes, Some(seed))?;
            assert!(ctx.is_webp());
            for cfg in &webp_configs {
                ctx.add_from_config(cfg.clone())?;
            }
            ctx.execute();
            ctx.buffer()
        };

        let out1 = run(777)?;
        let out2 = run(777)?;
        assert_eq!(out1, out2);
        assert_eq!(detect_format(&out1), Some(ImageFormat::Webp));
        Ok(())
    }

    #[test]
    fn test_1x1_and_adversarial_filter_parameters_never_panic() -> Result<()> {
        // 1x1 1-bit grayscale (1 byte per row)
        let png_1x1 = build_synthetic_png(1, 1, 0, 1, None);
        let mut ctx = GlitchContext::new(&png_1x1, Some(1))?;
        for cfg in all_png_filter_configs(1.0) {
            ctx.add_from_config(cfg)?;
        }
        // Also test negative ColorDistortion::strength (including i16::MIN), NaN ColorSpaceGlitch, and huge RandomCopy::times via direct struct construction
        ctx.add_filter(ColorDistortion {
            magnitude: 1.0,
            strength: -5,
        });
        ctx.add_filter(ColorDistortion {
            magnitude: 1.0,
            strength: i16::MIN,
        });
        ctx.add_filter(ColorSpaceGlitch {
            magnitude: 1.0,
            hue_shift: f64::NAN,
            saturation_mult: f64::INFINITY,
            lightness_mult: -1.0,
        });
        ctx.add_filter(RandomCopy::new(u32::MAX));
        ctx.execute();
        assert!(!ctx.buffer()?.is_empty());

        // Config validation rejects negative strength, NaN/negative multipliers, and excessive RandomCopy::times
        assert!(
            ctx.add_from_config(FilterConfig::ColorDistortion {
                magnitude: 0.5,
                strength: Some(-5),
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::ColorSpaceGlitch {
                magnitude: 0.5,
                hue_shift: Some(f64::NAN),
                saturation_mult: None,
                lightness_mult: None,
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::ColorSpaceGlitch {
                magnitude: 0.5,
                hue_shift: None,
                saturation_mult: Some(-0.5),
                lightness_mult: None,
            })
            .is_err()
        );
        assert!(
            ctx.add_from_config(FilterConfig::RandomCopy {
                times: MAX_RANDOM_COPY_TIMES + 1,
            })
            .is_err()
        );
        Ok(())
    }
}
