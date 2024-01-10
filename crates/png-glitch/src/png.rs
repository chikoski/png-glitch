use std::fs::File;
use std::path::Path;

use crate::png::parser::Chunk;
use crate::png::parser::Header;
use crate::png::parser::Terminator;
use crate::png::parser::Parser;

pub use crate::png::encoder::Encoder;
pub use crate::png::glitch_context::GlitchContext;
pub use crate::png::scan_line::ScanLine;

mod parser;
mod encoder;
mod scan_line;
mod png_error;
mod glitch_context;

pub struct Png {
    header: Header,
    terminator: Terminator,
    misc_chunks: Vec<Chunk>,
    data: Vec<u8>,
}

impl Png {

    pub fn glitch<F>(&mut self, mut modifier: F) where F: FnMut(&mut GlitchContext) {
        let mut context = self.glitch_context();
        modifier(&mut context);
    }

    pub fn foreach_scanline<F>(&mut self, mut modifier: F) where F: FnMut(&mut ScanLine) {
        let mut start = 0;
        while start + self.scan_line_width() < self.data.len() {
            let end = start + self.scan_line_width();
            let buffer = &mut self.data[start..end];
            if let Ok(mut scan_line) = ScanLine::try_from(buffer){
                modifier(&mut scan_line);
            }
            start = end;
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        let _ = self.encode(&mut file)?;
        Ok(())
    }

    fn new(header: Header,
           terminator: Terminator,
           misc_chunks: Vec<Chunk>,
           data: Vec<u8>) -> Png {
        Png { header, terminator, misc_chunks, data }
    }

    fn parse(buffer: &[u8]) -> anyhow::Result<Png> {
        let png = Parser::parse(buffer)?;
        Ok(png)
    }

    fn width(&self) -> u32 {
        self.header.width()
    }

    fn height(&self) -> u32 {
        self.header.height()
    }

    fn scan_line_width(&self) -> usize {
        self.header.scan_line_width()
    }

    fn glitch_context(&mut self) -> GlitchContext {
        GlitchContext::new(self)
    }

}

impl TryFrom<&Vec<u8>> for Png {
    type Error = anyhow::Error;

    fn try_from(buffer: &Vec<u8>) -> Result<Self, Self::Error> {
        Png::parse(buffer)
    }
}

pub const SIGNATURE: &'static [u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
