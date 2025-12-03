use crate::operation::{Encode, Scan, Transpose};
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use crate::png::parser::{Chunk, ChunkType};
pub use crate::png::scan_line::ScanLine;
use anyhow::Context;
pub use parser::ColorType;
pub use scan_line::FilterType;
use scan_line::MemoryRange;
use std::cell::RefCell;
use std::fs::File;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

mod parser;
mod png_error;
mod scan_line;

/// A type alias for a vector of bytes representing decoded PNG data.
pub type DecodedData = Vec<u8>;
/// A type alias for a shared, mutable reference to decoded PNG data.
pub type SharedDecodedData = Rc<RefCell<DecodedData>>;

/// A function to create a shared, mutable reference to decoded PNG data.
pub fn share_decoded_data(value: DecodedData) -> SharedDecodedData {
    Rc::new(RefCell::new(value))
}

/// A struct representing a PNG image.
pub struct Png {
    header: Header,
    terminator: Terminator,
    misc_chunks: Vec<Chunk>,
    data: SharedDecodedData,
}

impl Png {
    /// The method saves the PNG image to a file.
    /// The `path` parameter is the path to the file.
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        self.encode(&mut file)?;
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

    /// The method returns the width of the PNG image.
    pub fn width(&self) -> u32 {
        self.header.width()
    }

    /// The method returns the height of the PNG image.
    pub fn height(&self) -> u32 {
        self.header.height()
    }

    fn scan_line_width(&self) -> usize {
        self.header.scan_line_width()
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

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.remove_filter();
    /// png_glitch.save("./etc/removed-all.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn remove_filter(&mut self) {
        self.remove_filter_from(0, self.height());
    }

    /// The method removes filter from the scan lines in specified region
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::PngGlitch;
    /// let mut png_glitch = PngGlitch::open("./etc/sample00.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.remove_filter_from(5, 10); // Remove filter from the scan line #5 - # 14
    /// png_glitch.save("./etc/removed-partial.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) {
        let index = if from > 0 { from - 1 } else { 0 };
        let mut lines = self.scan_lines_from(index as usize, lines as usize);
        lines.reverse();

        let mut previous = if from > 0 {
            lines.pop()
        } else {
            None
        };
        while !lines.is_empty() {
            let last_index = lines.len() - 1;
            lines[last_index].remove_filter(previous.as_ref());
            previous = lines.pop()
        }
    }

    /// The method removes filter from all scan lines.
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/none.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.apply_filter(FilterType::Sub);
    /// png_glitch.save("./etc/filter-all.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.apply_filter_from(filter, 0, self.height());
    }

    /// The method removes filter from scan lines in specified region
    ///
    /// # Example
    ///
    /// ```
    /// use png_glitch::{FilterType, PngGlitch};
    /// let mut png_glitch = PngGlitch::open("./etc/none.png").expect("The PNG file should be successfully parsed");
    /// png_glitch.apply_filter_from(FilterType::Sub, 5, 3); // Apply sub filter to the scan line #5, #6, and #7.
    /// png_glitch.save("./etc/filter-partial.png").expect("The PNG file should be successfully saved")
    /// ```
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        let mut lines = self.scan_lines_from(from as usize, lines as usize);
        let mut previous = lines.pop();

        while !lines.is_empty() {
            if let Some(mut line) = previous {
                previous = lines.pop();
                line.apply_filter(filter_type, previous.as_ref());
            }
        }
    }
}

impl TryFrom<&[u8]> for Png {
    type Error = anyhow::Error;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        Png::parse(buffer)
    }
}
impl Transpose for Png {
    fn transpose(&mut self, src: usize, dest: usize, lines: u32) {
        let src_range = self.scan_line_range(src, lines);
        let dest_range = self.scan_line_range(dest, lines);

        assert_eq!(
            src_range.len(),
            dest_range.len(),
            "Source and destination ranges must have the same length for transpose."
        );

        let mut data = self.data.borrow_mut();

        // .clone() を削除
        let tmp = data[src_range.clone()].to_vec();
        data.copy_within(dest_range.clone(), src_range.start);
        data[dest_range].copy_from_slice(&tmp);
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
    fn scan_lines(&self) -> Vec<ScanLine> {
        self.scan_lines_from(0, self.height() as usize)
    }

    fn foreach_scanline<F>(&self, mut modifier: F)
    where
        F: FnMut(&mut ScanLine),
    {
        for mut scan_line in self.scan_lines() {
            modifier(&mut scan_line);
        }
    }

    fn scan_lines_from(&self, from: usize, lines: usize) -> Vec<ScanLine> {
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        (0..lines)
            .map(|index| {
                let index = from + index;
                let range = self.scan_line_range(index, 1);
                let mem_range = MemoryRange::new(self.data.clone(), range, color_type, bit_depth);
                // try_from の結果をそのまま返す
                ScanLine::try_from(mem_range)
            })
            // Result::ok を filter_map に渡すことで、Ok(value) は Some(value) に、
            // Err(_) は None に変換され、無視される。より慣用的で簡潔な書き方。
            .filter_map(Result::ok)
            .collect()
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

/// The signature of a PNG file.
pub const SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

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

        let decoded_data_size = png.data.borrow().len();
        for i in 0..decoded_data_size {
            let decoded_data = &png.data.borrow();
            let another_decoded_data = &another.data.borrow();
            assert_eq!(decoded_data[i], another_decoded_data[i]);
        }
        Ok(())
    }
}
