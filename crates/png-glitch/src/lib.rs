extern crate core;

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub use crate::png::{FilterType, GlitchContext, ScanLine};
use crate::png::Encoder;
use crate::png::Png;

mod png;

/// PngGlitch is a crate to create a glitched PNG image.
/// Please refer to ["The Art of PNG glitch"](https://ucnv.github.io/pnglitch/) for the description about what glitched PNG is.
///
/// # Examples
///
/// The following snippet shows how you can glitch "./a_png_file.png" and save the generated image as "./glitched.png".
///
/// ```
/// use png_glitch::{FilterType, PngGlitch};
/// let mut png_glitch = PngGlitch::open("./a_png_file.png")?;
/// png_glitch.foreach_scanline(|scan_line|{
///   scan_line.set_filter_type(FilterType::None);
///   scan_line[4] = 1;
/// });
/// png_glitch.save("./glitched.png")?;
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
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./a_png_file.png")?;
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
    /// let mut buffer = vec![];
    /// let mut file = File::open("./a_png_file/png")?;
    /// file.read_to_end(&mut buffer)?;
    /// let mut png_glitch = PngGlitch::new(buffer)?;
    /// ```
    pub fn new(buffer: Vec<u8>) -> anyhow::Result<PngGlitch> {
        let png = Png::try_from(&buffer)?;
        Ok(PngGlitch { png })
    }

    /// The method manipulates the decoded bitmap data with the specified modifier function.
    /// The specified modifier is called with a `GlitchContext` object.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./a_png_file.png")?;
    /// png_glitch.glitch(|context|{
    ///   let mut index = 1;
    ///   while index < context.width() {
    ///      context.data()[index] = 0;
    ///      index += context.scan_line_width();
    ///   }
    /// });
    /// ```
    pub fn glitch<F>(&mut self, modifier: F) where F: FnMut(&mut GlitchContext) {
        self.png.glitch(modifier)
    }

    /// The method allows you to manipulate for each [scan line](https://www.w3.org/TR/2003/REC-PNG-20031110/#4Concepts.EncodingScanlineAbs%22).
    /// The modifier function is called with a `ScanLine` object which represents a scan line.
    ///
    /// # Example
    ///
    /// The following example changes the filter method of all scan line to None.
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./a_png_file.png")?;
    /// png_glitch.foreach_scanline(|scan_line|{
    ///    scan_line.set_filter_type(FilterType::None);
    /// });
    /// ```
    pub fn foreach_scanline<F>(&mut self, modifier: F) where F: FnMut(&mut ScanLine) {
        self.png.foreach_scanline(modifier)
    }

    /// The method saves the glitched image as a PNG file to the given path.
    ///
    /// # Example
    ///
    /// The following example copies `./a_png_file.png` as `./another_png_file.png`.
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let png_glitch = PngGlitch::open("./a_png_file.png")?;
    /// png_glitch.save("./another_png_file.png")?;
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
    /// use png_glitch::PngGlitch;
    /// let png_glitch = PngGlitch::open("./a_png_file.png")?;
    /// let mut encoded_data = vec![];
    /// png_glitch.encode(&mut encoded_data)?;
    /// ```
    pub fn encode(&self, buffer: &mut [u8]) -> anyhow::Result<()> {
        let _ = self.png.encode(buffer)?;
        Ok(())
    }
}
