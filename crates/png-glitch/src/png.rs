use crate::operation::{Encode, Scan, Transpose};
pub use crate::png::live::{Encoding, FilterStrategy, PixelSource};
pub use crate::png::live_pipeline::LivePipeline;
use crate::png::parser::Header;
use crate::png::parser::Parser;
use crate::png::parser::Terminator;
use crate::png::parser::{Chunk, ChunkType};
pub use crate::png::parser::{Limits, ParseOptions};
pub use crate::png::png_error::{PixelError, PngError, PngWarning};
pub use crate::png::render::Renderer;
pub use crate::png::scan_line::Pixel;
pub use crate::png::scan_line::ScanLine;
pub use crate::png::scan_lines::{BlockMut, ScanLines};
pub use parser::ColorType;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
pub use scan_line::FilterType;
use std::fs::File;
use std::ops::Range;
use std::path::Path;

mod live;
mod live_pipeline;
mod parser;
mod png_error;
mod render;
mod scan_line;
mod scan_lines;

/// A type alias for a vector of bytes representing decoded PNG data.
pub type DecodedData = Vec<u8>;

/// A struct representing a PNG image.
#[derive(Debug)]
pub struct Png {
    pub(crate) header: Header,
    pub(crate) terminator: Terminator,
    pub(crate) pre_idat_chunks: Vec<Chunk>,
    pub(crate) post_idat_chunks: Vec<Chunk>,
    pub(crate) data: DecodedData,
    pub(crate) warnings: Vec<PngWarning>,
}

impl Png {
    /// The method saves the PNG image to a file.
    /// The `path` parameter is the path to the file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PngError> {
        let mut file = File::create(path)?;
        self.encode(&mut file)?;
        Ok(())
    }

    fn new(
        header: Header,
        terminator: Terminator,
        pre_idat_chunks: Vec<Chunk>,
        post_idat_chunks: Vec<Chunk>,
        data: Vec<u8>,
        warnings: Vec<PngWarning>,
    ) -> Png {
        Png {
            header,
            terminator,
            pre_idat_chunks,
            post_idat_chunks,
            data,
            warnings,
        }
    }

    pub(crate) fn parse(buffer: &[u8]) -> Result<Png, PngError> {
        Parser::parse_with_options(buffer, ParseOptions::default())
    }

    pub(crate) fn parse_with_options(
        buffer: &[u8],
        options: ParseOptions,
    ) -> Result<Png, PngError> {
        Parser::parse_with_options(buffer, options)
    }

    /// Returns any non-fatal integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        &self.warnings
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

    fn scan_line_range(&self, scan_line_index: usize, lines: usize) -> Range<usize> {
        let start = self.index_of(scan_line_index);
        let end = start + self.scan_line_width() * lines;
        start..end
    }

    /// The stored pixel at column `x` of row `y`, or `None` outside the image.
    pub(crate) fn pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        let stride = self.scan_line_width();
        let start = (y as usize).checked_mul(stride)?;
        let row = self.data.get(start..start.checked_add(stride)?)?;
        scan_line::pixel_at(
            row,
            self.header.color_type(),
            self.header.bit_depth(),
            self.header.width(),
            x as usize,
        )
    }

    /// The method removes filter from all scan lines.
    pub fn remove_filter(&mut self) {
        self.remove_filter_from(0, self.height());
    }

    /// Reconstructs raw pixel bytes by removing PNG filters in `[from, from + lines)`,
    /// clamping out-of-range regions to the image height.
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) {
        let height = self.height() as usize;
        let start_index = (from as usize).min(height);
        let end_index = start_index.saturating_add(lines as usize).min(height);
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();

        let bpp = scan_line::bytes_per_pixel(color_type, bit_depth);

        // Reconstructing row `i` requires row `i - 1` to already be unfiltered,
        // so rows are processed top-down (`start_index..end_index`). Where a multi-row kernel
        // exists (`filter::remove_rows`, #152), several rows are reconstructed together; it keeps
        // the per-pixel dependency order, so the bytes are the same.
        let mut i = start_index;
        while i < end_index {
            let current_start = i * scan_line_width;
            let (prev_part, current_part) = self.data.split_at_mut(current_start);
            // `data` is `height * scan_line_width` bytes, so these ranges exist; if it were ever
            // shorter, the rows past its end are left as they are.
            let prev_line_data = match i.checked_sub(1) {
                Some(prev) => {
                    let prev_start = prev * scan_line_width;
                    match prev_part.get_mut(prev_start..prev_start + scan_line_width) {
                        Some(row) => Some(row),
                        None => return,
                    }
                }
                None => None,
            };
            let Some(block) = current_part.get_mut(..(end_index - i) * scan_line_width) else {
                return;
            };
            let rows = scan_line::filter::remove_rows(
                block,
                scan_line_width,
                bpp,
                prev_line_data.as_deref(),
            );
            if rows > 0 {
                i += rows;
                continue;
            }

            let Some(row) = block.get_mut(..scan_line_width) else {
                return;
            };
            let mut current_line = ScanLine::new(row, color_type, bit_depth, image_width);
            match prev_line_data {
                Some(prev_line_data) => {
                    let prev_line =
                        ScanLine::new(prev_line_data, color_type, bit_depth, image_width);
                    current_line.remove_filter(Some(&prev_line));
                }
                None => current_line.remove_filter(None),
            }
            i += 1;
        }
    }

    /// The method applies a filter to all scan lines.
    pub fn apply_filter(&mut self, filter: FilterType) {
        self.apply_filter_from(filter, 0, self.height());
    }

    /// Applies `filter_type` to scan lines in `[from, from + lines)`,
    /// clamping out-of-range regions to the image height.
    pub fn apply_filter_from(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        let height = self.height() as usize;
        let start_index = (from as usize).min(height);
        let end_index = start_index.saturating_add(lines as usize).min(height);
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();

        // Filtering row `i` in place requires row `i - 1` to still hold its
        // unfiltered bytes, so rows must be processed bottom-up (`rev()`).
        for i in (start_index..end_index).rev() {
            let current_start = i * scan_line_width;
            let (prev_part, current_part) = self.data.split_at_mut(current_start);
            // `data` is `height * scan_line_width` bytes; rows past a shorter buffer are skipped.
            let Some(current_line_data) = current_part.get_mut(..scan_line_width) else {
                continue;
            };

            let mut current_line =
                ScanLine::new(current_line_data, color_type, bit_depth, image_width);

            match i.checked_sub(1) {
                Some(prev) => {
                    let prev_start = prev * scan_line_width;
                    let Some(prev_line_data) =
                        prev_part.get_mut(prev_start..prev_start + scan_line_width)
                    else {
                        continue;
                    };
                    let prev_line =
                        ScanLine::new(prev_line_data, color_type, bit_depth, image_width);
                    current_line.apply_filter(filter_type, Some(&prev_line));
                }
                None => current_line.apply_filter(filter_type, None),
            }
        }
    }

    /// Re-encodes the scan lines in `[from, from + lines)` with `filter_type`, clamping to the
    /// image height. Only the stored bytes of those rows change: rows outside the range keep
    /// their tag and filtered bytes.
    ///
    /// The rows are decoded from the stored bytes of the whole image above them, so glitches
    /// above the range are baked in as they would be by a whole-image re-encode, and the row
    /// above the range is the previous line of the first re-encoded row.
    pub fn change_filter_type_in_rows(&mut self, filter_type: FilterType, from: u32, lines: u32) {
        let height = self.height() as usize;
        let start = (from as usize).min(height);
        let end = start.saturating_add(lines as usize).min(height);
        if start == end {
            return;
        }
        let width = self.scan_line_width();
        // Rows above the range are decoded in place to serve as context, then put back.
        let Some(above) = self.data.get(..start * width).map(<[u8]>::to_vec) else {
            return;
        };
        self.remove_filter_from(0, end as u32);
        self.apply_filter_from(filter_type, start as u32, (end - start) as u32);
        if let Some(rows) = self.data.get_mut(..above.len()) {
            rows.copy_from_slice(&above);
        }
    }

    /// The method changes the filter type of all scan lines.
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.remove_filter();
        self.apply_filter(filter_type);
    }
}

impl TryFrom<&[u8]> for Png {
    type Error = PngError;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        Png::parse(buffer)
    }
}
impl Transpose for Png {
    fn transpose(&mut self, src: usize, dest: usize, lines: u32) {
        let height = self.height() as usize;
        if src >= height || dest >= height || src == dest || lines == 0 {
            return;
        }
        let max_lines = (height - src).min(height - dest);
        let actual_lines = (lines as usize).min(max_lines);
        if actual_lines == 0 {
            return;
        }

        let src_range = self.scan_line_range(src, actual_lines);
        let dest_range = self.scan_line_range(dest, actual_lines);

        // Both ranges are inside `data` (`height * scan_line_width` bytes); if not, nothing moves.
        let Some(tmp) = self.data.get(src_range.clone()).map(<[u8]>::to_vec) else {
            return;
        };
        if self.data.get(dest_range.clone()).is_none() {
            return;
        }
        self.data.copy_within(dest_range.clone(), src_range.start);
        if let Some(dest) = self.data.get_mut(dest_range) {
            dest.copy_from_slice(&tmp);
        }
    }
}

impl Encode for Png {
    fn encode(&self, mut writer: impl std::io::Write) -> Result<(), PngError> {
        writer.write_all(SIGNATURE)?;
        self.header.encode(&mut writer)?;
        for chunk in self.pre_idat_chunks.iter() {
            chunk.encode(&mut writer)?;
        }
        let idat_chunk_list = create_idat_chunk(self)?;
        for chunk in idat_chunk_list.iter() {
            chunk.encode(&mut writer)?;
        }
        for chunk in self.post_idat_chunks.iter() {
            chunk.encode(&mut writer)?;
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

    // NOTE: the closure runs on several threads in no fixed order; it must not use a shared RNG
    // (see the reproducibility policy in the README).
    fn par_foreach_scanline<F>(&mut self, modifier: F)
    where
        F: Fn(&mut ScanLine) + Sync + Send,
    {
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();
        #[cfg(feature = "parallel")]
        self.data
            .par_chunks_exact_mut(scan_line_width)
            .for_each(|chunk| {
                let mut scan_line = ScanLine::new(chunk, color_type, bit_depth, image_width);
                modifier(&mut scan_line);
            });
        // Without `parallel` the same closure runs sequentially, in row order.
        #[cfg(not(feature = "parallel"))]
        self.data
            .chunks_exact_mut(scan_line_width)
            .for_each(|chunk| {
                let mut scan_line = ScanLine::new(chunk, color_type, bit_depth, image_width);
                modifier(&mut scan_line);
            });
    }

    fn scan_lines_from(&mut self, from: usize, lines: usize) -> Vec<ScanLine<'_>> {
        let height = self.height() as usize;
        let start_line = from.min(height);
        let end_line = start_line.saturating_add(lines).min(height);
        let scan_line_width = self.scan_line_width();
        let color_type = self.header.color_type();
        let bit_depth = self.header.bit_depth();
        let image_width = self.header.width();

        let start = start_line * scan_line_width;
        let end = end_line * scan_line_width;

        self.data
            .get_mut(start..end)
            .unwrap_or_default()
            .chunks_exact_mut(scan_line_width)
            .map(|chunk| ScanLine::new(chunk, color_type, bit_depth, image_width))
            .collect()
    }
}

fn create_idat_chunk(png: &Png) -> Result<Vec<Chunk>, PngError> {
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
    fn test_encode_ihdr() -> Result<(), PngError> {
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
    fn test_encode() -> Result<(), PngError> {
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
    fn test_transpose() -> Result<(), PngError> {
        let bytes = include_bytes!("../etc/sample00.png");
        let mut png = Png::parse(bytes)?;
        let original_data = png.data.clone();
        let height = png.height();

        if height > 2 {
            png.transpose(0, 1, 1);
            // Verify that row 0 and row 1 are swapped
            let width = png.scan_line_width();
            assert_eq!(png.data[0..width], original_data[width..2 * width]);
            assert_eq!(png.data[width..2 * width], original_data[0..width]);
            // Verify the rest of the data is unchanged
            assert_eq!(png.data[2 * width..], original_data[2 * width..]);
        }

        Ok(())
    }
}
