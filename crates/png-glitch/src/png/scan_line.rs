use std::fmt::Debug;
use std::io::{Read, Write};
use std::ops::{Index, IndexMut, Range};
use thiserror::Error;
use crate::png::{ColorType, SharedDecodedData};
pub use filter_type::FilterType;
pub use memory_range::MemoryRange;

mod filter_type;
mod memory_range;
mod filter;

pub type UsizeRange = Range<usize>;

/// ScanLine represents each scan line in a PNG image.
pub struct ScanLine {
    filter_type: FilterType,
    range: UsizeRange,
    decoded_data: SharedDecodedData,
    color_type: ColorType,
    bit_depth: u8,
}

impl ScanLine {
    fn new(filter_type: FilterType, decoded_data: SharedDecodedData, range: UsizeRange, color_type: ColorType, bit_depth: u8) -> ScanLine {
        ScanLine {
            filter_type,
            decoded_data,
            range,
            color_type,
            bit_depth,
        }
    }

    fn pixel_data_offset(&self) -> usize {
        self.range.start + 1
    }

    fn pixel_data_range(&self) -> UsizeRange {
        self.pixel_data_offset()..self.range.end
    }

    fn bytes_per_pixel(&self) -> usize {
        let bits = self.bit_depth;
        match self.color_type {
            ColorType::GrayScale => std::cmp::max(bits / 8, 1) as usize,
            ColorType::GrayScaleAlpha => std::cmp::max(bits * 2 / 8, 1) as usize,
            ColorType::TrueColor => std::cmp::max(bits * 3 / 8, 1) as usize,
            ColorType::TrueColorAlpha => std::cmp::max(bits * 4 / 8, 1) as usize,
            ColorType::IndexColor => (bits / 8) as usize,
        }
    }

    /// Apply filter to the scanline
    pub fn apply_filter(&mut self, filter_type: FilterType, previous: Option<&ScanLine>) {
        filter::apply(filter_type, self, previous);
        self.set_filter_type(filter_type);
    }

    /// Remove filter applied to the scanline
    pub fn remove_filter(&mut self, other: Option<&ScanLine>) {
        filter::remove(self, other);
        self.set_filter_type(FilterType::None);
    }

    /// This method returns the filter method applied to the scan line.
    pub fn filter_type(&self) -> FilterType {
        self.filter_type
    }

    /// This method updates the filter method of the scan line with the specified one.
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.decoded_data.borrow_mut()[self.range.start] = filter_type.into();
    }

    /// This method returns the byte size of the scan line.
    pub fn size(&self) -> usize {
        self.range.len() - 1
    }

    /// This method returns the color type of the scan line.
    pub fn color_type(&self) -> ColorType {
        self.color_type
    }

    /// This method returns the bit_depth of each pixel.
    pub fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    /// index method returns a byte in a pixel_data specified with the index parameter
    pub fn index(&self, index: usize) -> Option<u8> {
        let pixel_data_range = self.pixel_data_range();
        let index = pixel_data_range.start + index;
        if index < pixel_data_range.end {
            Some(self.decoded_data.borrow()[index])
        } else {
            None
        }
    }

    /// update method updates a value of the pixel specified by the index with the given value
    pub fn update(&self, index: usize, value: u8) {
        let pixel_data_range = self.pixel_data_range();
        let index = pixel_data_range.start + index;
        if index < pixel_data_range.end {
            self.decoded_data.borrow_mut()[index] = value
        }
    }
}

impl Index<usize> for ScanLine {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let index = index + self.pixel_data_offset();
            &(&(*self.decoded_data.as_ptr()))[index]
        }
    }
}

impl IndexMut<usize> for ScanLine {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe {
            let index = index + self.pixel_data_offset();
            &mut (&mut *self.decoded_data.as_ptr())[index]
        }
    }
}

impl Read for ScanLine {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut buffer = &self.decoded_data.borrow()[self.pixel_data_range()];
        buffer.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut buffer = &self.decoded_data.borrow()[self.pixel_data_range()];
        buffer.read_to_end(buf)
    }
}

impl Write for ScanLine {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let pixel_data_range = self.pixel_data_range();
        let mut buffer = &mut self.decoded_data.borrow_mut()[pixel_data_range];
        buffer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.decoded_data.borrow_mut().flush()
    }
}

impl TryFrom<MemoryRange> for ScanLine {
    type Error = anyhow::Error;

    fn try_from(value: MemoryRange) -> Result<Self, Self::Error> {
        let byte = value
            .first_byte()
            .ok_or(ScanLineError::InvalidMemoryRange)?;

        let filter_type = FilterType::try_from(byte)?;
         Ok(ScanLine::new(filter_type, value.decoded_data, value.range, value.color_type, value.bit_depth))
    }
}

#[derive(Error, Debug)]
enum ScanLineError {
    #[error("Invalid memory range is specified")]
    InvalidMemoryRange,
}

#[cfg(test)]
mod test {
    use crate::png::share_decoded_data;

    use super::*;

    struct TestTarget {
        buffer: SharedDecodedData,
    }

    impl<'a> TestTarget {
        fn new() -> Self {
            let buffer = vec![0, 1, 2, 3, 4, 5];
            let buffer = share_decoded_data(buffer);
            TestTarget { buffer }
        }

        fn usize_range(&self) -> UsizeRange {
            0..self.buffer.borrow().len()
        }

        fn scan_line(&self) -> ScanLine {
            ScanLine::new(FilterType::None, self.buffer.clone(), self.usize_range(), ColorType::TrueColorAlpha, 8)
        }
    }

    mod index {
        use crate::png::scan_line::test::TestTarget;

        #[test]
        fn test_index() {
            let target = TestTarget::new();
            let scan_line = target.scan_line();

            assert_eq!(scan_line[0], target.buffer.borrow()[1]);
        }

        #[test]
        fn test_index_mut() {
            let target = TestTarget::new();
            let mut scan_line = target.scan_line();

            scan_line[0] = 10;

            assert_eq!(scan_line[0], target.buffer.borrow()[1]);
        }
    }

    mod read {
        use std::io::Read;

        use super::*;

        #[test]
        fn test_read() {
            let target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![0; scan_line.size()];

            let result = scan_line.read(&mut buffer);
            assert_eq!(true, result.is_ok());
            assert_eq!(scan_line.size(), buffer.len());
            assert_eq!(&scan_line.decoded_data.borrow()[1..], &buffer);
        }

        #[test]
        fn test_read_to_end() {
            let target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![];

            let size = scan_line.size();
            let result = scan_line.read_to_end(&mut buffer);
            assert_eq!(true, result.is_ok());
            assert_eq!(&scan_line.decoded_data.borrow()[1..], &buffer[0..size]);
        }
    }

    mod write {
        use super::*;

        #[test]
        fn test_write() {
            let target = TestTarget::new();
            let mut scan_line = target.scan_line();
            let size = scan_line.size();

            let buffer = vec![10; size];
            let result = scan_line.write(&buffer);
            assert_eq!(true, result.is_ok());
            assert_eq!(buffer.len(), result.unwrap());
            assert_eq!(&buffer, &scan_line.decoded_data.borrow()[1..]);
        }
    }
}
