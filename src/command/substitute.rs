use crate::command::Command;
use png_glitch::PngGlitch;
#[derive(Debug, Default)]
pub struct Substitute {
    index: usize,
    value: u8,
}

impl Substitute {
    pub fn new(index: usize, value: u8) -> Substitute {
        Substitute { index, value }
    }
}

impl Command for Substitute {
    fn run(&self, png: &mut PngGlitch) {
        png.foreach_scanline(|scanline| {
            if self.index < scanline.size() {
                scanline[self.index] = self.value;
            }
        });
    }
}