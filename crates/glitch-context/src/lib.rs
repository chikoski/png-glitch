use anyhow::{Context, Result};
pub use png_glitch::presets::{Brighten, Invert, ShiftChannels};
pub use png_glitch::{FilterType, PngGlitch};
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

        Ok(())
    }
}
