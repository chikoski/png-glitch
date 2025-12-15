use anyhow::{Context, Result};
use png_glitch::{FilterType, PngGlitch};
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
}

/// Filter that changes the filter type of scanlines.
pub struct ChangeFilterType {
    pub magnitude: f64,
}

impl GlitchFilter for ChangeFilterType {
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::thread_rng();
        png.foreach_scanline(|scan_line| {
            if rng.gen_bool(self.magnitude) {
                let filter_type = match rng.gen_range(0..5) {
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

/// Filter that replaces pixel data with random noise.
pub struct Replace {
    pub magnitude: f64,
}

impl GlitchFilter for Replace {
    fn apply(&self, png: &mut PngGlitch) {
        let mut rng = rand::thread_rng();
        png.foreach_scanline(|scan_line| {
            // Note: ScanLine doesn't expose raw byte length easily unless we know it.
            // But we can iterate indices.
            // We assume safe upper bound or try until error?
            // png-glitch doesn't easily expose scanline length in bytes directly via scan_line object,
            // but we can try updating indices.
            // Using a heuristic: scan line length approx width * bytes_per_pixel + 1.
            // But scan_line.index(i) works.
            // Let's iterate a reasonable range.
            // Actually, we should probably access the underlying data if possible.
            // scan_line.index(x) uses byte index.
            // We don't know the max index easily from scan_line.
            // But we can just try updating random indices?
            // "Replaces pixel data"
            // If we iterate 0..10000 it might be slow or wrong.
            // Let's look at PngGlitch::width() and infer?
            // FilterType is at index 0 (conceptually? No, index(0) is first data byte).
            // Let's assume we can keep accessing until it fails?
            // scan_line.index() returns Option.

            // To do this efficiently, we might need to know the length.
            // However, implementing "Replace" for *every* pixel with probability `magnitude`
            // requires iterating all pixels. Use `while let Some(_) = scan_line.index(i)` loop?

            let mut i = 0;
            while let Some(_) = scan_line.index(i) {
                if rng.gen_bool(self.magnitude) {
                    let val: u8 = rng.gen();
                    scan_line.update(i, val);
                }
                i += 1;
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
        let mut rng = rand::thread_rng();
        let height = png.height();
        // Magnitude as probability of swapping?
        // "Magnitude means how frequent transpose happens... probability of swapping."
        // If we iterate all lines and swap with prob `magnitude`:
        for i in 0..height {
            if rng.gen_bool(self.magnitude) {
                let target = rng.gen_range(0..height);
                if i != target {
                    // src, dest, lines
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
        let mut rng = rand::thread_rng();
        png.foreach_scanline(|scan_line| {
            let mut i = 0;
            while let Some(_) = scan_line.index(i) {
                if rng.gen_bool(self.magnitude) {
                    scan_line.update(i, 0);
                }
                i += 1;
            }
        });
    }
}
