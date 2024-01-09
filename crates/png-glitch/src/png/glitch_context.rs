use crate::png::Png;

pub struct GlitchContext<'a> {
    png: &'a mut Png,
}

impl<'a> GlitchContext<'a> {
    pub fn new(png: &mut Png) -> GlitchContext {
        GlitchContext { png }
    }

    pub fn width(&self) -> u32 {
        self.png.width()
    }

    pub fn height(&self) -> u32 {
        self.png.height()
    }

    pub fn scan_line_width(&self) -> usize {
        self.png.scan_line_width()
    }

    pub fn data(&mut self) -> &mut [u8] {
        &mut self.png.data
    }
}