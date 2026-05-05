use anyhow::{Context, Result};
pub use png_glitch::presets::{Brighten, Invert, ShiftChannels};
pub use png_glitch::{FilterType, PngGlitch, Pixel};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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
