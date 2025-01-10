use std::fs::File;
use std::io::Read;
use std::path::Path;
pub use crate::operation::Transpose;
use crate::operation::{Encode, Scan};
use crate::png::Png;
pub use crate::png::{FilterType, ScanLine};

mod png;
mod operation;

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
        let png = Png::try_from(&buffer)?;
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
    pub fn scan_lines_from(&self, from: usize, lines: usize) -> Vec<ScanLine> {
        self.png.scan_lines_from(from, lines)
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
        let _ = self.png.encode(buffer)?;
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
    pub fn transpose(&mut self, src: usize, dst: usize, lines: u32) {
        self.png.transpose(src, dst, lines)
    }

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.remove_filter();
    /// png_glitch.save("./etc/removed.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn remove_filter(&mut self) {
        self.png.remove_filter()
    }

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/none.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.apply_filter(FilterType::Sub);
    /// png_glitch.save("./etc/filter-sub.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.png.apply_filter(filter)
    }
}