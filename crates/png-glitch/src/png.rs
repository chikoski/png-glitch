use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;

pub use scan_line::FilterType;

pub use crate::png::encoder::Encoder;
use crate::png::parser::Chunk;
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use crate::png::scan_line::MemoryRange;
pub use crate::png::scan_line::ScanLine;

mod encoder;
mod parser;
mod png_error;
mod scan_line;

pub type DecodedData = Vec<u8>;
pub type SharedDecodedData = Rc<RefCell<DecodedData>>;

pub fn share_decoded_data(value: DecodedData) -> SharedDecodedData {
    Rc::new(RefCell::new(value))
}

pub struct Png {
    header: Header,
    terminator: Terminator,
    misc_chunks: Vec<Chunk>,
    data: SharedDecodedData,
}

impl Png {

    pub fn scan_lines(&mut self) -> Vec<ScanLine> {
        let size = self.scan_line_width();
        let decoded_data_size = self.decoded_data_size();

        vec![size; decoded_data_size / size]
            .iter().enumerate().map(|(index, size)| {
            let start = *size * index;
            let end = start + *size;
            start..end
        }).map(|range| {
            let memory_range = MemoryRange::new(self.data.clone(), range);
            ScanLine::try_from(memory_range)
        })
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .collect()
    }

    pub fn foreach_scanline<F>(&mut self, mut modifier: F)
        where
            F: FnMut(&mut ScanLine),
    {
        for mut scan_line in self.scan_lines() {
            modifier(&mut scan_line);
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        let _ = self.encode(&mut file)?;
        Ok(())
    }

    fn new(header: Header, terminator: Terminator, misc_chunks: Vec<Chunk>, data: Vec<u8>) -> Png {
        let data = share_decoded_data(data);
        Png {
            header,
            terminator,
            misc_chunks,
            data,
        }
    }

    fn parse(buffer: &[u8]) -> anyhow::Result<Png> {
        let png = Parser::parse(buffer)?;
        Ok(png)
    }

    pub fn width(&self) -> u32 {
        self.header.width()
    }

    pub fn height(&self) -> u32 {
        self.header.height()
    }

    fn scan_line_width(&self) -> usize {
        self.header.scan_line_width()
    }

    fn decoded_data_size(&self) -> usize {
        self.data.borrow().len()
    }
}

impl TryFrom<&Vec<u8>> for Png {
    type Error = anyhow::Error;

    fn try_from(buffer: &Vec<u8>) -> Result<Self, Self::Error> {
        Png::parse(buffer)
    }
}

pub const SIGNATURE: &'static [u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
