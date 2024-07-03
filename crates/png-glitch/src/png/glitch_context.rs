use crate::png::Png;
use crate::ScanLine;

/// GlitchContext provides information necessary to glitch PNG images.
pub struct GlitchContext<'a> {
    png: &'a mut Png,
}

impl<'a> GlitchContext<'a> {
    pub fn new(png: &mut Png) -> GlitchContext {
        GlitchContext { png }
    }

    /// This method returns the width of the PNG file you are dealing with.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// This method returns the height of the PNG file you are dealing with.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// This method returns the byte size of a scan line.
    pub fn scan_line_width(&self) -> usize {
        self.png.scan_line_width()
    }

    pub fn scan_lines(&mut self) -> Vec<ScanLine> {
        self.png.scan_lines()
    }
}
