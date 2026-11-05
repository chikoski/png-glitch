use anyhow::{bail, Context, Result};
#[cfg(feature = "clap")]
use clap::ValueEnum;
pub use png_glitch::presets::{Brighten, Darken, Invert, ShiftChannels};
pub use png_glitch::{FilterType, Pixel, PngError, PngGlitch};
pub use rand;
use rand::{Rng, RngCore, SeedableRng};
pub use rand_chacha;
pub use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::path::Path;

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
        let path = path.as_ref();
        let png = PngGlitch::open(path)
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
        let png = PngGlitch::from_bytes(data).context("Failed to parse PNG data")?;
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
                    f.hue_shift = h;
                }
                if let Some(s) = saturation_mult {
                    f.saturation_mult = s;
                }
                if let Some(l) = lightness_mult {
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
#[serde(rename_all = "lowercase")]
pub enum PreProcess {
    #[default]
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
}

/// Strategy for corrupting the alpha channel in [`AlphaGlitch`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum AlphaGlitchStrategy {
    #[default]
    Invert,
    Randomize,
    Zero,
    One,
}

/// Declarative configuration for a single glitch filter step (used in YAML config files).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FilterConfig {
    ChangeFilterType {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    Replace {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    Transpose {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    SetZero {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    RandomCopy {
        times: u32,
    },
    Substitute {
        index: usize,
        value: u8,
    },
    PixelSort {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        criterion: Option<SortCriterion>,
    },
    Bitwise {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        op: Option<BitOp>,
        value: Option<u8>,
    },
    ChannelSwap {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        target: Option<SwapTarget>,
    },
    HorizontalShift {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    BlockScramble {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        block_size: Option<u32>,
    },
    ColorDistortion {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        strength: Option<i16>,
    },
    ColorSpaceGlitch {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        hue_shift: Option<f64>,
        saturation_mult: Option<f64>,
        lightness_mult: Option<f64>,
    },
    ChromaticAberration {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        r_offset: Option<i32>,
        g_offset: Option<i32>,
        b_offset: Option<i32>,
    },
    Invert,
    Brighten {
        strength: u16,
    },
    Darken {
        strength: u16,
    },
    ShiftChannels {
        r: i16,
        g: i16,
        b: i16,
    },
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
    MacroblockGlitch {
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    AlphaGlitch {
        #[serde(default = "default_alpha_magnitude", deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        strategy: Option<AlphaGlitchStrategy>,
    },
    LossyArtifact {
        quality: f32,
    },
}

fn default_alpha_magnitude() -> f64 {
    1.0
}

/// Filter that randomly overwrites the PNG filter type byte of scanlines without re-filtering pixel data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChangeFilterType {
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
            if rng.random_bool(magnitude) {
                let val: u8 = rng.random();
                let index: u64 = rng.random();
                let index = (index as usize) % scan_line.size();
                scan_line.update(index, val);
            }
        });
    }
}

/// Filter that swaps scanlines randomly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transpose {
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
        if height == 0 {
            return;
        }
        let mut scan_lines = png.scan_lines();
        let index_range = 0..height as usize;
        for _ in 0..self.times {
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
    pub index: usize,
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
            if rng.random_bool(magnitude) {
                let index: u64 = rng.random();
                let index = (index as usize) % scan_line.size();
                scan_line.update(index, 0);
            }
        });
    }
}

/// Criterion used by [`PixelSort`] to order pixels within a scanline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum SortCriterion {
    #[default]
    Brightness,
    Hue,
}

/// Filter that sorts pixels in each selected scanline by brightness or hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelSort {
    pub criterion: SortCriterion,
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
#[serde(rename_all = "lowercase")]
pub enum BitOp {
    And,
    Or,
    #[default]
    Xor,
}

/// Filter that performs bitwise operations on scanline data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bitwise {
    pub op: BitOp,
    pub value: u8,
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
#[serde(rename_all = "lowercase")]
pub enum SwapTarget {
    #[default]
    Rg,
    Gb,
    Br,
}

/// Filter that swaps a pair of color channels on randomly selected scanlines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSwap {
    pub target: SwapTarget,
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
            if rng.random_bool(magnitude) {
                let shift = rng.random_range(0..scan_line.size());
                let mut buffer = vec![0; scan_line.size()];
                for (i, byte) in buffer.iter_mut().enumerate().take(scan_line.size()) {
                    *byte = scan_line.index(i).unwrap();
                }

                for (i, &byte) in buffer.iter().enumerate().take(scan_line.size()) {
                    let new_idx = (i + shift) % scan_line.size();
                    scan_line.update(new_idx, byte);
                }
            }
        });
    }
}

/// Filter that scrambles square blocks of pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockScramble {
    pub magnitude: f64,
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
    pub magnitude: f64,
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
        let strength = self.strength;
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
    pub magnitude: f64,
    pub hue_shift: f64,
    pub saturation_mult: f64,
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
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(magnitude) {
                scan_line.process_pixels(|_, p| {
                    match p {
                        Pixel::RGB(r, g, b) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + self.hue_shift).rem_euclid(360.0);
                            let s = (s * self.saturation_mult).clamp(0.0, 1.0);
                            let l = (l * self.lightness_mult).clamp(0.0, 1.0);
                            let (r, g, b) = hsl_to_rgb(h, s, l);
                            Pixel::RGB(r, g, b)
                        }
                        Pixel::RGBA(r, g, b, a) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + self.hue_shift).rem_euclid(360.0);
                            let s = (s * self.saturation_mult).clamp(0.0, 1.0);
                            let l = (l * self.lightness_mult).clamp(0.0, 1.0);
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
    pub magnitude: f64,
    pub r_offset: i32,
    pub g_offset: i32,
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
    pub r: i16,
    pub g: i16,
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
            if rng.random_bool(magnitude) {
                let size = line.size();
                if size == 0 {
                    return;
                }
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
        let strength = self.strength.min(255);
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
    pub magnitude: f64,
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
    Png,
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
    Png(GlitchContext),
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
                let ctx = GlitchContext::new(&data, seed)
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
        match detect_format(data) {
            Some(ImageFormat::Png) => Ok(Self::Png(GlitchContext::new(data, seed)?)),
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
}
