pub use crate::operation::Transpose;
use crate::operation::{Encode, Scan};
use crate::png::Png;
pub use crate::png::{FilterType, ScanLine, Pixel};
use std::fs::File;
use std::io::Read;
use std::path::Path;

mod operation;
mod png;

/// PngGlitch is a crate to create a glitched PNG image.
/// Please refer to ["The Art of PNG glitch"](https://ucnv.github.io/pnglitch/) for the description about what glitched PNG is.
///
/// # Examples
///
/// The following snippet shows how you can glitch "./etc/sample00.png" and save the generated image as "./glitched.png".
///
/// ```
/// # use std::env;
/// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
///
/// use png_glitch::{FilterType, PngGlitch};
///
/// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
/// png_glitch.foreach_scanline(|scan_line|{
///   scan_line.set_filter_type(FilterType::None);
///   let pixel = scan_line.index(4).unwrap_or(0);
///   scan_line.update(4, pixel / 2);
/// });
/// png_glitch.save("./glitched.png").expect("The glitched file should be saved as a PNG file");
/// ```
///
pub struct PngGlitch {
    png: Png,
}

impl PngGlitch {
    /// The method creates a PngGlitch object to glitch the PNG image loaded from the given file path.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<PngGlitch> {
        let mut file = File::open(path)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        PngGlitch::new(buf)
    }

    /// The method creates a PngGlitch object to glitch the PNG image stored in a given `Vec<u8>`.
    pub fn new(buffer: Vec<u8>) -> anyhow::Result<PngGlitch> {
        let png = Png::try_from(&buffer as &[u8])?;
        Ok(PngGlitch { png })
    }

    /// The method returns a list of scan lines in the given PNG file.
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        self.png.scan_lines()
    }

    /// The method takes the specified number of ScanLine objects at most.
    pub fn scan_lines_from(&mut self, from: u32, lines: u32) -> Vec<ScanLine<'_>> {
        self.png.scan_lines_from(from as usize, lines as usize)
    }

    /// The method allows you to manipulate for each scan line.
    pub fn foreach_scanline<F>(&mut self, modifier: F)
    where
        F: FnMut(&mut ScanLine),
    {
        self.png.foreach_scanline(modifier)
    }

    /// The method allows you to manipulate for each scan line in parallel.
    pub fn par_foreach_scanline<F>(&mut self, modifier: F)
    where
        F: Fn(&mut ScanLine) + Sync + Send,
    {
        self.png.par_foreach_scanline(modifier)
    }

    /// The method saves the glitched image as a PNG file to the given path.
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.png.save(path)
    }

    /// The method encodes the glitched image as a PNG data and write the encoded data to the given buffer.
    pub fn encode(&self, buffer: &mut Vec<u8>) -> anyhow::Result<()> {
        self.png.encode(buffer)?;
        Ok(())
    }

    /// The method returns the width of the loaded PNG file
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// The method returns the height of the loaded PNG file
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// The method copies the lines starting from src to dest
    pub fn transpose(&mut self, src: u32, dst: u32, lines: u32) {
        self.png.transpose(src as usize, dst as usize, lines)
    }

    /// The method removes filter from all scan lines.
    pub fn remove_filter(&mut self) {
        self.png.remove_filter();
    }

    /// The method removes filter from the scan lines in specified region
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) {
        self.png.remove_filter_from(from, lines);
    }

    /// The method applies filter to all scan lines.
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.png.apply_filter(filter);
    }

    /// The method applies filter to scan lines in specified region
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        self.png.apply_filter_from(filter_type, from, lines);
    }

    /// The method changes the filter type of all scan lines.
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.png.change_filter_type(filter_type);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_change_filter_type() -> anyhow::Result<()> {
        let bytes = include_bytes!("../etc/sample00.png");

        // 1. Baseline: Raw (removed filter)
        let mut png_raw = PngGlitch::new(bytes.to_vec())?;
        png_raw.remove_filter();

        // 2. Test Subject
        let mut png_glitch = PngGlitch::new(bytes.to_vec())?;

        // Change to Sub
        png_glitch.change_filter_type(FilterType::Sub);

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
}
