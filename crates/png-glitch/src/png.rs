use crate::operation::{Encode, Scan, Transpose};
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use crate::png::parser::{Chunk, ChunkType};
pub use crate::png::scan_line::ScanLine;
pub use crate::png::scan_line::Pixel;
use anyhow::Context;
pub use parser::ColorType;
use rayon::prelude::*;
pub use scan_line::FilterType;
use std::fs::File;
use std::ops::Range;
use std::path::Path;

mod parser;
mod png_error;
mod scan_line;

/// A type alias for a vector of bytes representing decoded PNG data.
pub type DecodedData = Vec<u8>;

/// A struct representing a PNG image.
#[derive(Debug)]
pub struct Png {
    pub(crate) header: Header,
    pub(crate) terminator: Terminator,
    pub(crate) misc_chunks: Vec<Chunk>,
    pub(crate) data: DecodedData,
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
    pub fn remove_filter(&mut self) {
        self.remove_filter_from(0, self.height());
    }

    /// The method removes filter from the scan lines in specified region
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) {
        let end_index = from + lines;
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();
        
        // This operation is inherently sequential because each line depends on the previous one.
        for i in (from..end_index).rev() {
            let current_start = i as usize * scan_line_width;
            let (prev_part, current_part) = self.data.split_at_mut(current_start);
            let current_line_data = &mut current_part[..scan_line_width];
            
            let mut current_line = ScanLine::new(current_line_data, color_type, bit_depth, image_width);
            
            if i > 0 {
                let prev_start = (i as usize - 1) * scan_line_width;
                let prev_line_data = &mut prev_part[prev_start..prev_start + scan_line_width];
                let prev_line = ScanLine::new(prev_line_data, color_type, bit_depth, image_width);
                current_line.remove_filter(Some(&prev_line));
            } else {
                current_line.remove_filter(None);
            }
        }
    }

    /// The method applies a filter to all scan lines.
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.apply_filter_from(filter, 0, self.height());
    }

    /// The method applies a filter to scan lines in specified region
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        let end_index = from + lines;
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();

        for i in from..end_index {
            let current_start = i as usize * scan_line_width;
            let (prev_part, current_part) = self.data.split_at_mut(current_start);
            let current_line_data = &mut current_part[..scan_line_width];
            
            let mut current_line = ScanLine::new(current_line_data, color_type, bit_depth, image_width);
            
            if i > 0 {
                let prev_start = (i as usize - 1) * scan_line_width;
                let prev_line_data = &mut prev_part[prev_start..prev_start + scan_line_width];
                let prev_line = ScanLine::new(prev_line_data, color_type, bit_depth, image_width);
                current_line.apply_filter(filter_type, Some(&prev_line));
            } else {
                current_line.apply_filter(filter_type, None);
            }
        }
    }

    /// The method changes the filter type of all scan lines.
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.remove_filter();
        self.apply_filter(filter_type);
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

        let mut tmp = vec![0; src_range.len()];
        tmp.copy_from_slice(&self.data[src_range.clone()]);
        self.data.copy_within(dest_range.clone(), src_range.start);
        self.data[dest_range].copy_from_slice(&tmp);
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
    fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        let height = self.height() as usize;
        self.scan_lines_from(0, height)
    }

    fn foreach_scanline<F>(&mut self, mut modifier: F)
    where
        F: FnMut(&mut ScanLine),
    {
        for mut scan_line in self.scan_lines() {
            modifier(&mut scan_line);
        }
    }

    fn par_foreach_scanline<F>(&mut self, modifier: F)
    where
        F: Fn(&mut ScanLine) + Sync + Send,
    {
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();
        self.data.par_chunks_exact_mut(scan_line_width).for_each(|chunk| {
            let mut scan_line = ScanLine::new(chunk, color_type, bit_depth, image_width);
            modifier(&mut scan_line);
        });
    }

    fn scan_lines_from(&mut self, from: usize, lines: usize) -> Vec<ScanLine<'_>> {
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();
        
        let start = from * scan_line_width;
        let end = (from + lines) * scan_line_width;
        
        self.data[start..end]
            .chunks_exact_mut(scan_line_width)
            .map(|chunk| ScanLine::new(chunk, color_type, bit_depth, image_width))
            .collect()
    }
}

fn create_idat_chunk(png: &Png) -> anyhow::Result<Vec<Chunk>> {
    let mut list = vec![];

    let mut encoder = fdeflate::Compressor::new(vec![])?;
    encoder.write_data(&png.data)?;
    let buffer = encoder.finish()?;

    list.push(Chunk::with_recomputed_crc(ChunkType::Data, buffer));
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

        let decoded_data_size = png.data.len();
        for i in 0..decoded_data_size {
            assert_eq!(png.data[i], another.data[i]);
        }
        Ok(())
    }

    #[test]
    fn test_transpose() -> anyhow::Result<()> {
        let bytes = include_bytes!("../etc/sample00.png");
        let mut png = Png::parse(bytes)?;
        let original_data = png.data.clone();
        let height = png.height();
        
        if height > 2 {
            png.transpose(0, 1, 1);
            // Verify that row 0 and row 1 are swapped
            let width = png.scan_line_width();
            assert_eq!(png.data[0..width], original_data[width..2*width]);
            assert_eq!(png.data[width..2*width], original_data[0..width]);
            // Verify the rest of the data is unchanged
            assert_eq!(png.data[2*width..], original_data[2*width..]);
        }
        
        Ok(())
    }
}
