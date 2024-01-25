cargo_component_bindings::generate!();

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::png::Encoder;
use crate::png::Png;
pub use crate::png::{FilterType, GlitchContext, ScanLine};

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
/// use png_glitch::{FilterType, PngGlitch};///
///
/// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
/// png_glitch.foreach_scanline(|scan_line|{
///   scan_line.set_filter_type(FilterType::None);
///   scan_line[4] = 1;
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

    /// The method manipulates the decoded bitmap data with the specified modifier function.
    /// The specified modifier is called with a `GlitchContext` object.
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
    /// png_glitch.glitch(|context|{
    ///   let mut index = 1;
    ///   while index  < (context.width() as usize) {
    ///      context.data()[index] = 0;
    ///      index += context.scan_line_width();
    ///   }
    /// });
    /// ```
    pub fn glitch<F>(&mut self, modifier: F)
    where
        F: FnMut(&mut GlitchContext),
    {
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
    pub fn foreach_scanline<F>(&mut self, modifier: F)
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
}
