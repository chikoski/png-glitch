use crate::png::ColorType;
pub use filter_type::FilterType;
use std::io::{Read, Write};
use std::ops::{Index, IndexMut, Range};

mod filter;
mod filter_type;

/// A type alias for a range of `usize`.
pub type UsizeRange = Range<usize>;

/// A struct representing a scan line in a PNG image.
pub struct ScanLine<'a> {
    pub(crate) data: &'a mut [u8],
    pub(crate) color_type: ColorType,
    pub(crate) bit_depth: u8,
}

impl<'a> ScanLine<'a> {
    pub(crate) fn new(
        data: &'a mut [u8],
        color_type: ColorType,
        bit_depth: u8,
    ) -> ScanLine<'a> {
        ScanLine {
            data,
            color_type,
            bit_depth,
        }
    }

    pub(crate) fn pixel_data_offset(&self) -> usize {
        1
    }

    fn pixel_data_range(&self) -> UsizeRange {
        self.pixel_data_offset()..self.data.len()
    }

    pub(crate) fn bytes_per_pixel(&self) -> usize {
        let bits = self.bit_depth;
        match self.color_type {
            ColorType::GrayScale => std::cmp::max(bits / 8, 1) as usize,
            ColorType::GrayScaleAlpha => std::cmp::max(bits * 2 / 8, 1) as usize,
            ColorType::TrueColor => std::cmp::max(bits * 3 / 8, 1) as usize,
            ColorType::TrueColorAlpha => std::cmp::max(bits * 4 / 8, 1) as usize,
            ColorType::IndexColor => (bits / 8) as usize,
        }
    }

    /// The method applies a filter to the scan line.
    /// The `filter_type` parameter is the type of the filter to apply.
    /// The `previous` parameter is the previous scan line.
    pub fn apply_filter(&mut self, filter_type: FilterType, previous_line: Option<&ScanLine>) {
        filter::apply(filter_type, self, previous_line);
        self.set_filter_type(filter_type);
    }

    pub fn remove_filter(&mut self, previous_line: Option<&ScanLine>) {
        filter::remove(self, previous_line);
        self.set_filter_type(FilterType::None);
    }

    /// The method changes the filter type of the ScanLine.
    /// It first calculates the original pixel value (decodes it),
    /// and then calculates the scan line data based on the new filter type.
    pub fn change_filter_type(
        &mut self,
        filter_type: FilterType,
        previous_line: Option<&ScanLine>,
    ) {
        self.remove_filter(previous_line);
        self.apply_filter(filter_type, previous_line);
    }

    /// This method returns the filter method applied to the scan line.
    pub fn filter_type(&self) -> FilterType {
        FilterType::try_from(self.data[0]).unwrap_or(FilterType::None)
    }

    /// This method updates the filter method of the scan line with the specified one.
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.data[0] = filter_type.into();
    }

    /// This method returns the byte size of the scan line.
    pub fn size(&self) -> usize {
        self.data.len() - 1
    }

    /// This method returns the color type of the scan line.
    pub fn color_type(&self) -> ColorType {
        self.color_type
    }

    /// This method returns the bit_depth of each pixel.
    pub fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    /// The method returns a byte in a pixel_data specified with the index parameter.
    pub fn index(&self, index: usize) -> Option<u8> {
        let pixel_data_offset = self.pixel_data_offset();
        let index = pixel_data_offset + index;
        if index < self.data.len() {
            Some(self.data[index])
        } else {
            None
        }
    }

    /// The method updates a value of the pixel specified by the index with the given value.
    pub fn update(&mut self, index: usize, value: u8) {
        let pixel_data_offset = self.pixel_data_offset();
        let index = pixel_data_offset + index;
        if index < self.data.len() {
            self.data[index] = value
        }
    }
}

impl<'a> Index<usize> for ScanLine<'a> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        let index = index + self.pixel_data_offset();
        &self.data[index]
    }
}

impl<'a> IndexMut<usize> for ScanLine<'a> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let index = index + self.pixel_data_offset();
        &mut self.data[index]
    }
}

impl<'a> Read for ScanLine<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut buffer = &self.data[self.pixel_data_range()];
        buffer.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut buffer = &self.data[self.pixel_data_range()];
        buffer.read_to_end(buf)
    }
}

impl<'a> Write for ScanLine<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let pixel_data_range = self.pixel_data_range();
        let mut buffer = &mut self.data[pixel_data_range];
        buffer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.data.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TestTarget {
        buffer: Vec<u8>,
    }

    impl TestTarget {
        fn new() -> Self {
            let buffer = vec![0, 1, 2, 3, 4, 5];
            TestTarget { buffer }
        }

        fn scan_line(&mut self) -> ScanLine<'_> {
            ScanLine::new(
                &mut self.buffer,
                ColorType::TrueColorAlpha,
                8,
            )
        }
    }

    mod index {
        use super::*;

        #[test]
        fn test_index() {
            let mut target = TestTarget::new();
            let scan_line = target.scan_line();

            assert_eq!(scan_line[0], 1);
        }

        #[test]
        fn test_index_mut() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            scan_line[0] = 10;

            assert_eq!(scan_line[0], 10);
            assert_eq!(target.buffer[1], 10);
        }
    }

    mod read {
        use std::io::Read;

        use super::*;

        #[test]
        fn test_read() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![0; scan_line.size()];

            let result = scan_line.read(&mut buffer);
            assert!(result.is_ok());
            assert_eq!(scan_line.size(), buffer.len());
            assert_eq!(&target.buffer[1..], &buffer);
        }

        #[test]
        fn test_read_to_end() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![];

            let size = scan_line.size();
            let result = scan_line.read_to_end(&mut buffer);
            assert!(result.is_ok());
            assert_eq!(&target.buffer[1..], &buffer[0..size]);
        }
    }

    mod write {
        use super::*;

        #[test]
        fn test_write() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();
            let size = scan_line.size();

            let buffer = vec![10; size];
            let result = scan_line.write(&buffer);
            assert!(result.is_ok());
            assert_eq!(buffer.len(), result.unwrap());
            assert_eq!(&buffer, &target.buffer[1..]);
        }
    }

    mod change_filter_type {
        use super::*;

        #[test]
        fn test_change_filter_type() {
            let mut buffer = vec![0, 1, 2, 3, 4, 5];
            {
                let mut scan_line = ScanLine::new(
                    &mut buffer,
                    ColorType::GrayScale,
                    8,
                );

                scan_line.change_filter_type(FilterType::Sub, None);
                assert_eq!(FilterType::Sub, scan_line.filter_type());
            }
            let encoded_sub = vec![1, 1, 1, 1, 1];
            assert_eq!(encoded_sub, &buffer[1..]);

            {
                let mut scan_line = ScanLine::new(
                    &mut buffer,
                    ColorType::GrayScale,
                    8,
                );
                scan_line.change_filter_type(FilterType::None, None);
                assert_eq!(FilterType::None, scan_line.filter_type());
            }
            let raw_data = vec![1, 2, 3, 4, 5];
            assert_eq!(raw_data, &buffer[1..]);
        }
    }
}
