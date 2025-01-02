use std::cell::RefCell;
use std::fs::File;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use anyhow::Context;
use crate::operation::{Scan, Transpose, Encode};
use crate::png::parser::{Chunk, ChunkType};
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use scan_line::MemoryRange;
pub use crate::png::scan_line::ScanLine;
pub use scan_line::FilterType;

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


impl Encode for Png {
    fn encode(&self, mut writer: impl std::io::Write) -> anyhow::Result<()> {
        writer.write_all(SIGNATURE)?;
        self.header
            .encode(&mut writer)
            .context("Failed to encode IHDR")?;
        for chunk in self.misc_chunks.iter() {
            chunk.encode(&mut writer)?;
        }
        let idat_chunk_list =
            create_idat_chunk(self).context("Failed to create IDAT chunk list")?;
        for chunk in idat_chunk_list.iter() {
            chunk.encode(&mut writer).context("Failed to encode IDAT")?;
        }
        self.terminator.encode(&mut writer)?;
        writer.flush()?;
        Ok(())
    }
}

impl Scan for Png {
    fn scan_lines(&mut self) -> Vec<ScanLine> {
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

    fn foreach_scanline<F>(&mut self, mut modifier: F)
    where
        F: FnMut(&mut ScanLine),
    {
        for mut scan_line in self.scan_lines() {
            modifier(&mut scan_line);
        }
    }
}

fn create_idat_chunk(png: &Png) -> anyhow::Result<Vec<Chunk>> {
    let mut list = vec![];

    let mut encoder = fdeflate::Compressor::new(vec![])?;
    encoder.write_data(&png.data.borrow())?;
    let buffer = encoder.finish()?;

    let mut crc = crc32fast::Hasher::new();
    crc.update(ChunkType::IDAT);
    crc.update(&buffer);
    let crc = crc.finalize().to_be_bytes();

    let chunk = Chunk::new(ChunkType::Data, buffer, crc);

    list.push(chunk);
    Ok(list)
}


pub const SIGNATURE: &'static [u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_ihdr() -> anyhow::Result<()> {
        let bytes = include_bytes!("../etc/sample00.png");
        let png = Png::parse(bytes)?;
        let mut buffer = vec![];
        png.header.encode(&mut buffer)?;
        assert_eq!(
            &buffer[0..4],
            &(png.header.inner.length() as u32).to_be_bytes()
        );
        assert_eq!(&buffer[4..8], ChunkType::IHDR);
        assert_eq!(&buffer[8..21], &png.header.inner.data);
        assert_eq!(&buffer[21..25], &png.header.inner.crc);
        Ok(())
    }

    #[test]
    fn test_encode() -> anyhow::Result<()> {
        let bytes = include_bytes!("../etc/sample00.png");
        let png = Png::parse(bytes)?;
        let mut buffer = vec![];
        png.encode(&mut buffer)?;
        let another = Png::parse(&buffer)?;
        assert_eq!(png.decoded_data_size(), another.decoded_data_size());
        for i in 0..png.decoded_data_size() {
            let decoded_data = &png.data.borrow();
            let another_decoded_data = &another.data.borrow();
            assert_eq!(decoded_data[i], another_decoded_data[i]);
        }
        Ok(())
    }
}