use std::fs::File;
use std::path::Path;

use crate::png::chunk::Chunk;
use crate::png::png_parser::png_header::PngHeader;
use crate::png::png_parser::png_terminator::PngTerminator;
use crate::png::png_parser::PngParser;
pub use crate::png::png_encoder::PngEncoder;

mod chunk;
mod error;
mod png_parser;
mod png_encoder;

pub struct Png {
    header: PngHeader,
    terminator: PngTerminator,
    misc_chunks: Vec<Chunk>,
    data: Vec<u8>,
}

impl Png {
    pub fn new(header: PngHeader,
               terminator: PngTerminator,
               misc_chunks: Vec<Chunk>,
               data: Vec<u8>) -> Png {
        Png { header, terminator, misc_chunks, data }
    }

    pub fn parse(buffer: &[u8]) -> anyhow::Result<Png> {
        PngParser::parse(buffer)
    }

    pub fn glitch<F>(&mut self, mut modifier: F) where F: FnMut(&mut [u8]) {
        modifier(&mut self.data);
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        let _ = self.encode(&mut file)?;
        Ok(())
    }
}

