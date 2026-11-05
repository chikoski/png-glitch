pub use crate::operation::Transpose;
use crate::operation::{Encode, Scan};
use crate::png::Png;
pub use crate::png::{FilterType, Pixel, PngError, ScanLine};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

mod operation;
mod png;
pub mod presets;

/// PngGlitch is a crate to create a glitched PNG image.
/// Please refer to ["The Art of PNG glitch"](https://ucnv.github.io/pnglitch/) for the description about what glitched PNG is.
///
/// # Available Presets (Recipes)
///
/// The library provides several built-in glitch recipes in the [`presets`] module:
/// - [`presets::Invert`]: Inverts all color channels of each pixel.
/// - [`presets::ShiftChannels`]: Shifts color channels (R, G, B) by specific amounts.
/// - [`presets::Brighten`]: Increases image brightness (saturating at the bit-depth maximum).
/// - [`presets::Darken`]: Decreases image brightness (saturating at 0).
///
/// # Examples
///
/// The following snippet shows how you can glitch "./etc/sample00.png" using a mix of presets and low-level manipulation.
///
/// ```
/// # use std::env;
/// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
///
/// use png_glitch::{FilterType, PngGlitch, Pixel};
/// use png_glitch::presets::{Invert, Brighten};
///
/// let out_path = std::env::temp_dir().join("png_glitch_doctest_glitched.png");
/// PngGlitch::open("./etc/sample00.png")
///   .expect("The PNG file should be successfully parsed")
///   .remove_filter()
///   .apply(Invert)
///   .apply(Brighten { strength: 30 })
///   .par_foreach_scanline(|scan_line| {
///     // Custom manipulation using the Pixel API
///     if let Some(Pixel::RGB(r, g, b)) = scan_line.get_pixel(10) {
///         scan_line.set_pixel(10, Pixel::RGB(r, 0, b));
///     }
///   })
///   .save(&out_path)
///   .expect("The glitched file should be saved as a PNG file");
/// # let _ = std::fs::remove_file(out_path);
/// ```
///
pub struct PngGlitch {
    png: Png,
}

impl PngGlitch {
    /// Creates a `PngGlitch` object to glitch the PNG image loaded from the given file path.
    pub fn open(path: impl AsRef<Path>) -> Result<PngGlitch, PngError> {
        let mut file = File::open(path)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        PngGlitch::from_bytes(&buf)
    }

    /// Creates a `PngGlitch` object from any byte slice or buffer containing PNG data.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<PngGlitch, PngError> {
        let png = Png::try_from(bytes.as_ref())?;
        Ok(PngGlitch { png })
    }

    /// Creates a `PngGlitch` object from a byte buffer (`Vec<u8>` or `&[u8]`).
    pub fn new(buffer: impl AsRef<[u8]>) -> Result<PngGlitch, PngError> {
        Self::from_bytes(buffer)
    }

    /// Returns a list of scan lines in the loaded PNG image.
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        self.png.scan_lines()
    }

    /// Takes the specified number of `ScanLine` objects starting from `from`.
    pub fn scan_lines_from(&mut self, from: u32, lines: u32) -> Vec<ScanLine<'_>> {
        self.png.scan_lines_from(from as usize, lines as usize)
    }

    /// Allows manipulating each scan line sequentially.
    pub fn foreach_scanline<F>(&mut self, modifier: F) -> &mut Self
    where
        F: FnMut(&mut ScanLine),
    {
        self.png.foreach_scanline(modifier);
        self
    }

    /// Allows manipulating each scan line in parallel.
    pub fn par_foreach_scanline<F>(&mut self, modifier: F) -> &mut Self
    where
        F: Fn(&mut ScanLine) + Sync + Send,
    {
        self.png.par_foreach_scanline(modifier);
        self
    }

    /// Saves the glitched image as a PNG file to the given path.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PngError> {
        self.png.save(path)
    }

    /// Encodes the glitched image as PNG data and writes it to the given `Write` sink.
    pub fn encode(&self, writer: impl Write) -> Result<(), PngError> {
        self.png.encode(writer)
    }

    /// Encodes the glitched image and returns the resulting PNG bytes as a `Vec<u8>`.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PngError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer)?;
        Ok(buffer)
    }

    /// Returns the width of the loaded PNG image.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// Returns the height of the loaded PNG image.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// Swaps `lines` scan lines starting from `src` with those at `dst`.
    pub fn transpose(&mut self, src: u32, dst: u32, lines: u32) -> &mut Self {
        self.png.transpose(src as usize, dst as usize, lines);
        self
    }

    /// Reconstructs raw pixel bytes by removing PNG prediction filters from all scan lines.
    pub fn remove_filter(&mut self) -> &mut Self {
        self.png.remove_filter();
        self
    }

    /// Reconstructs raw pixel bytes by removing PNG prediction filters in the specified region.
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) -> &mut Self {
        self.png.remove_filter_from(from, lines);
        self
    }

    /// Applies the specified PNG prediction filter to all scan lines.
    pub fn apply_filter(&mut self, filter: FilterType) -> &mut Self {
        self.png.apply_filter(filter);
        self
    }

    /// Applies the specified PNG prediction filter to scan lines in the specified region.
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) -> &mut Self {
        self.png.apply_filter_from(filter_type, from, lines);
        self
    }

    /// Re-encodes all scan lines with the given PNG filter type (`remove_filter` followed by `apply_filter`).
    pub fn reencode_with(&mut self, filter_type: FilterType) -> &mut Self {
        self.png.change_filter_type(filter_type);
        self
    }

    /// Changes the PNG filter type of all scan lines (alias for [`Self::reencode_with`]).
    pub fn change_filter_type(&mut self, filter_type: FilterType) -> &mut Self {
        self.reencode_with(filter_type)
    }

    /// Overwrites only the PNG filter-type tag byte on every scan line without transforming pixel bytes.
    pub fn set_filter_type(&mut self, filter_type: FilterType) -> &mut Self {
        self.foreach_scanline(|line| line.set_filter_type(filter_type))
    }

    /// Applies a glitch preset (`GlitchPreset`) to all scan lines in parallel.
    pub fn apply<P: presets::GlitchPreset + Sync + Send>(&mut self, preset: P) -> &mut Self {
        self.par_foreach_scanline(|line| {
            preset.apply_to_line(line);
        });
        self
    }

    /// Returns the raw decompressed scan-line data.
    pub fn data(&self) -> &[u8] {
        &self.png.data
    }
}

impl TryFrom<&[u8]> for PngGlitch {
    type Error = PngError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_change_filter_type() -> Result<(), PngError> {
        let bytes = include_bytes!("../etc/sample00.png");

        // 1. Baseline: Raw (removed filter)
        let mut png_raw = PngGlitch::from_bytes(bytes)?;
        png_raw.remove_filter();

        // 2. Test Subject
        let mut png_glitch = PngGlitch::try_from(bytes as &[u8])?;

        // Change to Sub
        png_glitch.reencode_with(FilterType::Sub);

        for scan_line in png_glitch.scan_lines() {
            assert_eq!(FilterType::Sub, scan_line.filter_type());
        }

        // Change back to None
        png_glitch.change_filter_type(FilterType::None);
        for scan_line in png_glitch.scan_lines() {
            assert_eq!(FilterType::None, scan_line.filter_type());
        }

        // Compare with Baseline (png_raw)
        let raw_lines = png_raw.scan_lines();
        let glitched_lines = png_glitch.scan_lines();

        assert_eq!(glitched_lines.len(), raw_lines.len());

        for (g_line, r_line) in glitched_lines.iter().zip(raw_lines.iter()) {
            let g_size = g_line.size();
            let r_size = r_line.size();
            assert_eq!(g_size, r_size);

            for i in 0..g_size {
                assert_eq!(
                    g_line.index(i),
                    r_line.index(i),
                    "Pixel mismatch at index {}",
                    i
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_presets_and_to_bytes() -> Result<(), PngError> {
        let bytes = include_bytes!("../etc/sample00.png");
        let mut png = PngGlitch::from_bytes(bytes)?;
        let original_data = png.data().to_vec();

        png.apply(presets::Invert);
        assert_ne!(png.data(), original_data);

        png.apply(presets::Invert);
        assert_eq!(png.data(), original_data);

        let encoded = png.to_bytes()?;
        assert!(!encoded.is_empty());

        Ok(())
    }

    #[test]
    fn test_typed_png_error_matching() {
        let err = match PngGlitch::from_bytes(b"not a png") {
            Err(e) => e,
            Ok(_) => panic!("expected PngError::InvalidSignature"),
        };
        assert!(matches!(err, PngError::InvalidSignature));
    }
}
