extern crate core;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::png::{GlitchContext, Png, ScanLine};
use crate::png::Encoder;

mod png;

pub struct PngGlitch {
    png: Png,
}

impl PngGlitch {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<PngGlitch> {
        let mut file = File::open(path)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        PngGlitch::new(buf)
    }

    pub fn new(buffer: Vec<u8>) -> anyhow::Result<PngGlitch> {
        let png = Png::try_from(&buffer)?;
        Ok(PngGlitch { png })
    }

    pub fn glitch<F>(&mut self, modifier: F) where F: FnMut(&mut GlitchContext) {
        self.png.glitch(modifier)
    }

    pub fn foreach_scanline<F>(&mut self, modifier: F) where F: FnMut(&mut ScanLine) {
        self.png.foreach_scanline(modifier)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.png.save(path)
    }

    pub fn encode(&self, buffer: &mut [u8]) -> anyhow::Result<()> {
        let _ = self.png.encode(buffer)?;
        Ok(())
    }
}
