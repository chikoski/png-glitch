use std::cell::RefCell;
use std::fs::File;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use crate::operation::Transpose;
pub use crate::png::encoder::Encoder;
use crate::png::parser::Chunk;
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use scan_line::MemoryRange;
pub use crate::png::scan_line::ScanLine;
pub use scan_line::FilterType;

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

        (0..decoded_data_size / size)
            .map(|index| {
                let range = self.scan_line_range(index, 1);
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

    fn index_of(&self, scan_line_index: usize) -> usize {
        let size = self.scan_line_width();
        scan_line_index * size
    }

    fn scan_line_range(&self, scan_line_index: usize, lines: u32) -> Range<usize> {
        let start = self.index_of(scan_line_index);
        let end = start + self.scan_line_width() * lines as usize;
        start..end
    }
}

impl TryFrom<&Vec<u8>> for Png {
    type Error = anyhow::Error;

    fn try_from(buffer: &Vec<u8>) -> Result<Self, Self::Error> {
        Png::parse(buffer)
    }
}

impl Transpose for Png {
    fn transpose(&mut self, src: usize, dest: usize, lines: u32) {
        let src = self.scan_line_range(src, lines);
        let dest = self.scan_line_range(dest, lines);
        let mut buf = vec![0; src.len()];
        buf.copy_from_slice(&self.data.borrow()[src]);

        let mut data = self.data.borrow_mut();
        data.splice(dest, buf);
    }
}

pub const SIGNATURE: &'static [u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
