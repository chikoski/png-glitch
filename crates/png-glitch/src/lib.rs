pub use crate::operation::Transpose;
use crate::operation::{Encode, Scan};
use crate::png::Png;
pub use crate::png::{FilterType, ScanLine};
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
    ///
    /// # Example
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    ///
    /// use png_glitch::PngGlitch;
    ///
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// ```
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<PngGlitch> {
        let mut file = File::open(path)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        PngGlitch::new(buf)
    }

    /// The method creates a PngGlitch object to glitch the PNG image stored in a given `Vec<u8>`.
    ///
    /// # Example
    ///
    /// A PngGlitch object is created from a `Vec<u8>` object containing PNG image data in the following snippet.
    ///
    /// ```
    /// use std::fs::File;
    /// use std::io::Read;
    /// use png_glitch::PngGlitch;
    ///
    /// let mut buffer = vec![];
    /// let mut file = File::open("./etc/sample00.png").expect("The file should be opened");
    /// file.read_to_end(&mut buffer).expect("The bytes in the file should be written into the buffer");
    /// let mut png_glitch = PngGlitch::new(buffer).expect("The data in the buffer should be successfully parsed as PNG");
    /// ```
    pub fn new(buffer: Vec<u8>) -> anyhow::Result<PngGlitch> {
        let png = Png::try_from(&buffer as &[u8])?;
        Ok(PngGlitch { png })
    }

    /// The method returns a list of [scan line](https://www.w3.org/TR/2003/REC-PNG-20031110/#4Concepts.EncodingScanlineAbs%22). in the given PNG file.
    ///
    /// # Example
    ///
    /// The following example changes the filter type of each scan line according its position
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    /// use png_glitch::{FilterType, PngGlitch};
    ///
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// for (index, scan_line) in png_glitch.scan_lines().iter_mut().enumerate() {
    ///    let filter_type = if index % 2 == 0 {
    ///        FilterType::None
    ///    } else {
    ///        FilterType::Average
    ///    };
    ///    scan_line.set_filter_type(filter_type);
    /// }
    /// ```
    pub fn scan_lines(&self) -> Vec<ScanLine> {
        self.png.scan_lines()
    }

    /// The method takes the specified number of ScanLine objects at most.
    /// The maximum number of ScanLines is specified as `lines` parameter.
    /// The `from` parameter specifies the index of first ScanLine.
    ///
    /// # Example
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    ///
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// let scan_liens = png_glitch.scan_lines_from(5, 10);
    /// ```
    pub fn scan_lines_from(&self, from: u32, lines: u32) -> Vec<ScanLine> {
        self.png.scan_lines_from(from as usize, lines as usize)
    }

    /// The method allows you to manipulate for each [scan line](https://www.w3.org/TR/2003/REC-PNG-20031110/#4Concepts.EncodingScanlineAbs%22).
    /// The modifier function is called with a `ScanLine` object which represents a scan line.
    ///
    /// # Example
    ///
    /// The following example changes the filter method of all scan line to None.
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    ///
    /// use png_glitch::{FilterType, PngGlitch};
    ///
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.foreach_scanline(|scan_line|{
    ///    scan_line.set_filter_type(FilterType::None);
    /// });
    /// ```
    pub fn foreach_scanline<F>(&self, modifier: F)
    where
        F: FnMut(&mut ScanLine),
    {
        self.png.foreach_scanline(modifier)
    }

    /// The method saves the glitched image as a PNG file to the given path.
    ///
    /// # Example
    ///
    /// The following example copies `./etc/sample00.png` as `./glitched.png`.
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    ///
    /// let png_glitch = PngGlitch::open("etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.save("./glitched.png").expect("The glitched PNG data should be saved to the given path");
    /// ```
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.png.save(path)
    }

    /// The method encodes the glitched image as a PNG data and write the encoded data to the given buffer.
    ///
    /// # Example
    ///
    /// The following example writes a PNG format data into the `encoded_data`.
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    /// use png_glitch::PngGlitch;
    ///
    /// let png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// let mut encoded_data:Vec<u8> = vec![];
    /// png_glitch.encode(&mut encoded_data).expect("The glitched PNG data should be written into the encoded_data in PNG format");
    /// ```
    pub fn encode(&self, buffer: &mut Vec<u8>) -> anyhow::Result<()> {
        self.png.encode(buffer)?;
        Ok(())
    }

    /// The method returns the width of the loaded PNG file
    ///
    /// # Example
    ///
    /// The following example retrieves width of ./etc/sample00.png
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    /// use png_glitch::PngGlitch;
    ///
    /// let png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// let width = png_glitch.width();
    /// ```
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// The method returns the height of the loaded PNG file
    ///
    /// # Example
    ///
    /// The following example retrieves height of ./etc/sample00.png
    ///
    /// ```
    /// # use std::env;
    /// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
    /// use png_glitch::PngGlitch;
    ///
    /// let png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// let width = png_glitch.width();
    /// ```
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// The method copies the lines starting from src to dest
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// let width = png_glitch.transpose(2, 5, 10);
    /// ```
    pub fn transpose(&mut self, src: u32, dst: u32, lines: u32) {
        self.png.transpose(src as usize, dst as usize, lines)
    }

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.remove_filter();
    /// png_glitch.save("./etc/removed-all.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn remove_filter(&mut self) {
        self.png.remove_filter();
    }

    /// The method removes filter from the scan lines in specified region
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.remove_filter_from(5, 10); // Remove filter from the scan line #5 - # 14
    /// png_glitch.save("./etc/removed-partial.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) {
        self.png.remove_filter_from(from, lines);
    }

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/none.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.apply_filter(FilterType::Sub);
    /// png_glitch.save("./etc/filter-all.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.png.apply_filter(filter);
    }

    /// The method removes filter from scan lines in specified region
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/none.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.apply_filter_from(FilterType::Sub, 5, 3); // Apply sub filter to the scan line #5, #6, and #7.
    /// png_glitch.save("./etc/filter-partial.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        self.png.apply_filter_from(filter_type, from, lines);
    }

    /// The method changes the filter type of all scan lines.
    /// It first calculates the original pixel value (decodes it),
    /// and then calculates the scan line data based on the new filter type.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.change_filter_type(FilterType::Sub);
    /// png_glitch.save("./etc/changed-all.png").expect("The PNG file should be successfully saved")
    /// ```
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
