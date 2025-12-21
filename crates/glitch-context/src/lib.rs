use anyhow::{Context, Result};
pub use png_glitch::{FilterType, PngGlitch};
use rand::Rng;
use std::path::Path;

/// A trait for glitch filters.
pub trait GlitchFilter {
    /// Applies the filter to the PngGlitch context.
    fn apply(&self, png: &mut PngGlitch);
}

/// A struct that manages the glitch context.
pub struct GlitchContext {
    png: PngGlitch,
    filters: Vec<Box<dyn GlitchFilter>>,
}

impl GlitchContext {
    /// Creates a new GlitchContext from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let png = PngGlitch::open(path).context("Failed to open PNG file")?;
        Ok(Self {
            png,
            filters: Vec::new(),
        })
    }

    /// Creates a new GlitchContext from a byte slice.
    pub fn new(data: &[u8]) -> Result<Self> {
        let png = PngGlitch::new(data.to_vec()).context("Failed to parse PNG data")?;
        Ok(Self {
            png,
            filters: Vec::new(),
        })
    }

    /// Adds a filter to the context.
    pub fn add_filter(&mut self, filter: impl GlitchFilter + 'static) {
        self.filters.push(Box::new(filter));
    }

    /// Executes all registered filters on the image.
    pub fn execute(&mut self) {
        for filter in &self.filters {
            filter.apply(&mut self.png);
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
    ///
    /// # Example
    ///
    /// ```
    /// # use std::env;
    /// # use std::path::Path;
    /// # let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string());
    /// # let root = Path::new(&manifest_dir).parent().unwrap();
    /// # let sample_path = root.join("png-glitch/etc/sample00.png");
    /// # let output_path = root.join("png-glitch/etc/changed_context.png");
    ///
    /// use glitch_context::{FilterType, GlitchContext};
    /// let mut context = GlitchContext::open(sample_path).expect("The PNG file should be successfully parsed");
    /// context.change_filter_type(FilterType::Sub);
    /// context.save(output_path).expect("The PNG file should be successfully saved")
    /// ```
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.png.change_filter_type(filter_type);
    }
}

/// Filter that changes the filter type of scanlines.
pub struct ChangeFilterType {
    pub magnitude: f64,
}

impl GlitchFilter for ChangeFilterType {
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::rng();
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
    fn apply(&self, png: &mut PngGlitch) {
        png.change_filter_type(FilterType::None);
    }
}

/// Filter that changes the filter type of all scan lines to Sub.
pub struct SubFilter;

impl GlitchFilter for SubFilter {
    fn apply(&self, png: &mut PngGlitch) {
        png.change_filter_type(FilterType::Sub);
    }
}

/// Filter that changes the filter type of all scan lines to Up.
pub struct UpFilter;

impl GlitchFilter for UpFilter {
    fn apply(&self, png: &mut PngGlitch) {
        png.change_filter_type(FilterType::Up);
    }
}

/// Filter that changes the filter type of all scan lines to Average.
pub struct AverageFilter;

impl GlitchFilter for AverageFilter {
    fn apply(&self, png: &mut PngGlitch) {
        png.change_filter_type(FilterType::Average);
    }
}

/// Filter that changes the filter type of all scan lines to Paeth.
pub struct PaethFilter;

impl GlitchFilter for PaethFilter {
    fn apply(&self, png: &mut PngGlitch) {
        png.change_filter_type(FilterType::Paeth);
    }
}

/// Filter that replaces pixel data with random noise.
pub struct Replace {
    pub magnitude: f64,
}

impl GlitchFilter for Replace {
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::rng();
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
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::rng();
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
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::rng();
        png.foreach_scanline(|scan_line| {
            if rng.random_bool(self.magnitude) {
                let index: u64 = rng.random();
                let index = (index as usize) % scan_line.size();
                scan_line.update(index, 0);
            }
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filters() -> Result<()> {
        let bytes = include_bytes!("../../png-glitch/etc/sample00.png");

        // Test SubFilter
        let mut ctx = GlitchContext::new(bytes)?;
        ctx.add_filter(SubFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Sub, line.filter_type());
        }

        // Test UpFilter
        let mut ctx = GlitchContext::new(bytes)?;
        ctx.add_filter(UpFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Up, line.filter_type());
        }

        // Test AverageFilter
        let mut ctx = GlitchContext::new(bytes)?;
        ctx.add_filter(AverageFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Average, line.filter_type());
        }

        // Test PaethFilter
        let mut ctx = GlitchContext::new(bytes)?;
        ctx.add_filter(PaethFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::Paeth, line.filter_type());
        }

        // Test RemoveFilter (None)
        let mut ctx = GlitchContext::new(bytes)?;
        // First set to Sub
        ctx.add_filter(SubFilter);
        // Then Remove
        ctx.add_filter(RemoveFilter);
        ctx.execute();
        let buffer = ctx.buffer()?;
        let png = PngGlitch::new(buffer)?;
        for line in png.scan_lines() {
            assert_eq!(FilterType::None, line.filter_type());
        }

        Ok(())
    }
}
