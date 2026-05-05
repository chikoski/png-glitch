use anyhow::{Context, Result};
use clap::ValueEnum;
pub use png_glitch::presets::{Brighten, Invert, ShiftChannels};
pub use png_glitch::{FilterType, PngGlitch, Pixel};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::path::Path;

/// A trait for glitch filters.
pub trait GlitchFilter {
    /// Applies the filter to the PngGlitch context.
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng);
}

/// A struct that manages the glitch context.
pub struct GlitchContext {
    png: PngGlitch,
    filters: Vec<Box<dyn GlitchFilter>>,
    rng: ChaCha8Rng,
}

impl GlitchContext {
    /// Creates a new GlitchContext from a file path.
    pub fn open(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self> {
        let png = PngGlitch::open(path).context("Failed to open PNG file")?;
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

    /// Creates a new GlitchContext from a byte slice.
    pub fn new(data: &[u8], seed: Option<u64>) -> Result<Self> {
        let png = PngGlitch::new(data.to_vec()).context("Failed to parse PNG data")?;
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

    /// Adds a filter to the context.
    pub fn add_filter(&mut self, filter: impl GlitchFilter + 'static) {
        self.filters.push(Box::new(filter));
    }

    /// Executes all registered filters on the image.
    pub fn execute(&mut self) {
        for filter in &self.filters {
            filter.apply(&mut self.png, &mut self.rng);
        }
    }

    /// Returns the encoded glitched image as a byte vector.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.png
            .encode(&mut buffer)
            .context("Failed to encode PNG")?;
        Ok(buffer)
    }

    /// Saves the glitched image to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.png.save(path).context("Failed to save PNG")?;
        Ok(())
    }

    /// Returns the width of the image.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// Returns the height of the image.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// The method changes the filter type of all scan lines.
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.png.change_filter_type(filter_type);
    }

    /// Applies a pre-process filter.
    pub fn pre_process(&mut self, pre_process: PreProcess) {
        match pre_process {
            PreProcess::RemoveFilter => self.add_filter(RemoveFilter),
            PreProcess::SubFilter => self.add_filter(SubFilter),
            PreProcess::UpFilter => self.add_filter(UpFilter),
            PreProcess::AverageFilter => self.add_filter(AverageFilter),
            PreProcess::PaethFilter => self.add_filter(PaethFilter),
        }
    }

    /// Adds a filter from a configuration object.
    pub fn add_from_config(&mut self, config: FilterConfig) {
        match config {
            FilterConfig::ChangeFilterType { magnitude } => {
                self.add_filter(ChangeFilterType { magnitude });
            }
            FilterConfig::Replace { magnitude } => {
                self.add_filter(Replace { magnitude });
            }
            FilterConfig::Transpose { magnitude } => {
                self.add_filter(Transpose { magnitude });
            }
            FilterConfig::SetZero { magnitude } => {
                self.add_filter(SetZero { magnitude });
            }
            FilterConfig::RandomCopy { times } => {
                self.add_filter(RandomCopy { times });
            }
            FilterConfig::Substitute { index, value } => {
                self.add_filter(Substitute { index, value });
            }
            FilterConfig::PixelSort {
                magnitude,
                criterion,
            } => {
                self.add_filter(PixelSort {
                    magnitude,
                    criterion: criterion.unwrap_or(SortCriterion::Brightness),
                });
            }
            FilterConfig::Bitwise {
                magnitude,
                op,
                value,
            } => {
                self.add_filter(Bitwise {
                    magnitude,
                    op: op.unwrap_or(BitOp::Xor),
                    value: value.unwrap_or(0),
                });
            }
            FilterConfig::ChannelSwap { magnitude, target } => {
                self.add_filter(ChannelSwap {
                    magnitude,
                    target: target.unwrap_or(SwapTarget::Rg),
                });
            }
            FilterConfig::HorizontalShift { magnitude } => {
                self.add_filter(HorizontalShift { magnitude });
            }
            FilterConfig::BlockScramble {
                magnitude,
                block_size,
            } => {
                self.add_filter(BlockScramble {
                    magnitude,
                    block_size: block_size.unwrap_or(16),
                });
            }
            FilterConfig::ColorDistortion {
                magnitude,
                strength,
            } => {
                self.add_filter(ColorDistortion {
                    magnitude,
                    strength: strength.unwrap_or(20),
                });
            }
            FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift,
                saturation_mult,
                lightness_mult,
            } => {
                self.add_filter(ColorSpaceGlitch {
                    magnitude,
                    hue_shift: hue_shift.unwrap_or(0.0),
                    saturation_mult: saturation_mult.unwrap_or(1.0),
                    lightness_mult: lightness_mult.unwrap_or(1.0),
                });
            }
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            } => {
                self.add_filter(ChromaticAberration {
                    magnitude,
                    r_offset: r_offset.unwrap_or(2),
                    g_offset: g_offset.unwrap_or(0),
                    b_offset: b_offset.unwrap_or(-2),
                });
            }
            FilterConfig::Invert => {
                self.add_filter(Invert);
            }
            FilterConfig::Brighten { strength } => {
                self.add_filter(Brighten { strength });
            }
            FilterConfig::ShiftChannels { r, g, b } => {
                self.add_filter(ShiftChannels { r, g, b });
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
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreProcess {
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum FilterConfig {
    ChangeFilterType {
        magnitude: f64,
    },
    Replace {
        magnitude: f64,
    },
    Transpose {
        magnitude: f64,
    },
    SetZero {
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
        magnitude: f64,
        criterion: Option<SortCriterion>,
    },
    Bitwise {
        magnitude: f64,
        op: Option<BitOp>,
        value: Option<u8>,
    },
    ChannelSwap {
        magnitude: f64,
        target: Option<SwapTarget>,
    },
    HorizontalShift {
        magnitude: f64,
    },
    BlockScramble {
        magnitude: f64,
        block_size: Option<u32>,
    },
    ColorDistortion {
        magnitude: f64,
        strength: Option<i16>,
    },
    ColorSpaceGlitch {
        magnitude: f64,
        hue_shift: Option<f64>,
        saturation_mult: Option<f64>,
        lightness_mult: Option<f64>,
    },
    ChromaticAberration {
        magnitude: f64,
        r_offset: Option<i32>,
        g_offset: Option<i32>,
        b_offset: Option<i32>,
    },
    Invert,
    Brighten {
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
}

/// Filter that changes the filter type of scanlines.
pub struct ChangeFilterType {
    pub magnitude: f64,
}

impl GlitchFilter for ChangeFilterType {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
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

/// Filter that removes filter from all scan lines.
pub struct RemoveFilter;

impl GlitchFilter for RemoveFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.change_filter_type(FilterType::None);
    }
}

/// Filter that changes the filter type of all scan lines to Sub.
pub struct SubFilter;

impl GlitchFilter for SubFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.change_filter_type(FilterType::Sub);
    }
}

/// Filter that changes the filter type of all scan lines to Up.
pub struct UpFilter;

impl GlitchFilter for UpFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.change_filter_type(FilterType::Up);
    }
}

/// Filter that changes the filter type of all scan lines to Average.
pub struct AverageFilter;

impl GlitchFilter for AverageFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.change_filter_type(FilterType::Average);
    }
}

/// Filter that changes the filter type of all scan lines to Paeth.
pub struct PaethFilter;

impl GlitchFilter for PaethFilter {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.change_filter_type(FilterType::Paeth);
    }
}

/// Filter that replaces pixel data with random noise.
pub struct Replace {
    pub magnitude: f64,
}

impl GlitchFilter for Replace {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
                let val: u8 = rng.random();
                let index: u64 = rng.random();
                let index = (index as usize) % scan_line.size();
                scan_line.update(index, val);
            }
        });
    }
}

/// Filter that swaps scanlines.
pub struct Transpose {
    pub magnitude: f64,
}

impl GlitchFilter for Transpose {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        let height = png.height();
        for i in 0..height {
            if rng.random_bool(self.magnitude) {
                let target = rng.random_range(0..height);
                if i != target {
                    png.transpose(i, target, 1);
                }
            }
        }
    }
}

/// Filter that copies scanlines.
pub struct RandomCopy {
    pub times: u32,
}

impl GlitchFilter for RandomCopy {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
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

/// Filter that sets a byte at a specific index to a specific value for all scanlines.
pub struct Substitute {
    pub index: usize,
    pub value: u8,
}

impl GlitchFilter for Substitute {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scanline| {
            if self.index < scanline.size() {
                scanline.update(self.index, self.value);
            }
        });
    }
}

/// Filter that sets pixels to zero.
pub struct SetZero {
    pub magnitude: f64,
}

impl GlitchFilter for SetZero {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
                let index: u64 = rng.random();
                let index = (index as usize) % scan_line.size();
                scan_line.update(index, 0);
            }
        });
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortCriterion {
    Brightness,
    Hue,
}

/// Filter that sorts pixels in each scanline.
pub struct PixelSort {
    pub criterion: SortCriterion,
    pub magnitude: f64,
}

impl GlitchFilter for PixelSort {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
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
                // simple brightness
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

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitOp {
    And,
    Or,
    Xor,
}

/// Filter that performs bitwise operations on scanline data.
pub struct Bitwise {
    pub op: BitOp,
    pub value: u8,
    pub magnitude: f64,
}

impl GlitchFilter for Bitwise {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
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

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwapTarget {
    Rg,
    Gb,
    Br,
}

/// Filter that swaps color channels.
pub struct ChannelSwap {
    pub target: SwapTarget,
    pub magnitude: f64,
}

impl GlitchFilter for ChannelSwap {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
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

/// Filter that shifts scanlines horizontally.
pub struct HorizontalShift {
    pub magnitude: f64,
}

impl GlitchFilter for HorizontalShift {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
                let shift = rng.random_range(0..scan_line.size());
                let mut buffer = vec![0; scan_line.size()];
                for i in 0..scan_line.size() {
                    buffer[i] = scan_line.index(i).unwrap();
                }
                
                for i in 0..scan_line.size() {
                    let new_idx = (i + shift) % scan_line.size();
                    scan_line.update(new_idx, buffer[i]);
                }
            }
        });
    }
}

/// Filter that scrambles blocks of pixels.
pub struct BlockScramble {
    pub magnitude: f64,
    pub block_size: u32,
}

impl GlitchFilter for BlockScramble {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
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
        let num_scrambles = (total_blocks as f64 * self.magnitude) as usize;

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

/// Filter that adds random color distortion.
pub struct ColorDistortion {
    pub magnitude: f64,
    pub strength: i16,
}

impl GlitchFilter for ColorDistortion {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        let strength = self.strength;
        let magnitude = self.magnitude;
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
pub struct ColorSpaceGlitch {
    pub magnitude: f64,
    pub hue_shift: f64,        // 0.0 - 360.0
    pub saturation_mult: f64,  // multiplier
    pub lightness_mult: f64,   // multiplier
}

impl GlitchFilter for ColorSpaceGlitch {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
                scan_line.process_pixels(|_, p| {
                    match p {
                        Pixel::RGB(r, g, b) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + self.hue_shift) % 360.0;
                            let s = (s * self.saturation_mult).clamp(0.0, 1.0);
                            let l = (l * self.lightness_mult).clamp(0.0, 1.0);
                            let (r, g, b) = hsl_to_rgb(h, s, l);
                            Pixel::RGB(r, g, b)
                        }
                        Pixel::RGBA(r, g, b, a) => {
                            let (h, s, l) = rgb_to_hsl(r, g, b);
                            let h = (h + self.hue_shift) % 360.0;
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

/// Filter that creates chromatic aberration by shifting channels.
pub struct ChromaticAberration {
    pub magnitude: f64,
    pub r_offset: i32,
    pub g_offset: i32,
    pub b_offset: i32,
}

impl GlitchFilter for ChromaticAberration {
    fn apply(&self, png: &mut PngGlitch, rng: &mut ChaCha8Rng) {
        let width = png.width() as usize;
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
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
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.apply(*self);
    }
}

impl GlitchFilter for Brighten {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.apply(*self);
    }
}

impl GlitchFilter for ShiftChannels {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut ChaCha8Rng) {
        png.apply(*self);
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
        // First set to Sub
        ctx.add_filter(SubFilter);
        // Then Remove
        ctx.add_filter(RemoveFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::None, line.filter_type());
        }

        // Test Substitute
        let mut ctx = GlitchContext::new(bytes, None)?;
        ctx.add_filter(Substitute { index: 0, value: 255 });
        ctx.execute();
        let buffer = ctx.buffer()?;
        let mut png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(line.index(0), Some(255));
        }

        // Test RandomCopy
        let mut ctx = GlitchContext::new(bytes, Some(12345))?;
        ctx.add_filter(RandomCopy { times: 10 });
        ctx.execute();
        let buffer = ctx.buffer()?;
        assert!(buffer.len() > 0);

        Ok(())
    }
}
