use crate::png::ColorType;
pub use filter_type::FilterType;
pub use pixel::Pixel;
use std::io::{Read, Write};
use std::ops::{Index, IndexMut, Range};

mod filter;
mod filter_type;
mod pixel;

/// A type alias for a range of `usize`.
pub type UsizeRange = Range<usize>;

/// A struct representing a scan line in a PNG image.
pub struct ScanLine<'a> {
    pub(crate) data: &'a mut [u8],
    pub(crate) color_type: ColorType,
    pub(crate) bit_depth: u8,
    pub(crate) width: u32,
}

impl<'a> ScanLine<'a> {
    pub(crate) fn new(
        data: &'a mut [u8],
        color_type: ColorType,
        bit_depth: u8,
        width: u32,
    ) -> ScanLine<'a> {
        ScanLine {
            data,
            color_type,
            bit_depth,
            width,
        }
    }

    /// The method returns the number of pixels in the scan line.
    pub fn pixels_count(&self) -> u32 {
        self.width
    }

    pub(crate) fn pixel_data_offset(&self) -> usize {
        1
    }

    pub(crate) fn pixel_data_range(&self) -> UsizeRange {
        self.pixel_data_offset()..self.data.len()
    }

    /// The method returns the number of bytes per pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        let bits = self.bit_depth;

        match self.color_type {
            ColorType::GrayScale => std::cmp::max(bits / 8, 1) as usize,
            ColorType::GrayScaleAlpha => std::cmp::max(bits * 2 / 8, 1) as usize,
            ColorType::TrueColor => std::cmp::max(bits * 3 / 8, 1) as usize,
            ColorType::TrueColorAlpha => std::cmp::max(bits * 4 / 8, 1) as usize,
            ColorType::IndexColor => std::cmp::max(bits / 8, 1) as usize,
        }
    }

    /// The method returns a pixel at the specified index.
    pub fn get_pixel(&self, x: usize) -> Option<Pixel> {
        if self.bit_depth < 8 {
            let pixels_per_byte = 8 / self.bit_depth as usize;
            let byte_offset = self.pixel_data_offset() + x / pixels_per_byte;
            if byte_offset >= self.data.len() {
                return None;
            }
            let byte = self.data[byte_offset];
            let shift = (pixels_per_byte - 1 - (x % pixels_per_byte)) * self.bit_depth as usize;
            let mask = (1 << self.bit_depth) - 1;
            let value = (byte >> shift) & mask;

            return match self.color_type {
                ColorType::GrayScale => Some(Pixel::Gray(value as u16)),
                ColorType::IndexColor => Some(Pixel::Indexed(value as u8)),
                _ => None,
            };
        }

        let bpp = self.bytes_per_pixel();
        let offset = self.pixel_data_offset() + x * bpp;
        if offset + bpp > self.data.len() {
            return None;
        }

        let slice = &self.data[offset..offset + bpp];
        match (self.color_type, self.bit_depth) {
            (ColorType::GrayScale, 8) => Some(Pixel::Gray(slice[0] as u16)),
            (ColorType::GrayScale, 16) => Some(Pixel::Gray(u16::from_be_bytes([slice[0], slice[1]]))),
            (ColorType::TrueColor, 8) => Some(Pixel::RGB(slice[0] as u16, slice[1] as u16, slice[2] as u16)),
            (ColorType::TrueColor, 16) => Some(Pixel::RGB(
                u16::from_be_bytes([slice[0], slice[1]]),
                u16::from_be_bytes([slice[2], slice[3]]),
                u16::from_be_bytes([slice[4], slice[5]]),
            )),
            (ColorType::IndexColor, 8) => Some(Pixel::Indexed(slice[0])),
            (ColorType::GrayScaleAlpha, 8) => Some(Pixel::GrayAlpha(slice[0] as u16, slice[1] as u16)),
            (ColorType::GrayScaleAlpha, 16) => Some(Pixel::GrayAlpha(
                u16::from_be_bytes([slice[0], slice[1]]),
                u16::from_be_bytes([slice[2], slice[3]]),
            )),
            (ColorType::TrueColorAlpha, 8) => Some(Pixel::RGBA(slice[0] as u16, slice[1] as u16, slice[2] as u16, slice[3] as u16)),
            (ColorType::TrueColorAlpha, 16) => Some(Pixel::RGBA(
                u16::from_be_bytes([slice[0], slice[1]]),
                u16::from_be_bytes([slice[2], slice[3]]),
                u16::from_be_bytes([slice[4], slice[5]]),
                u16::from_be_bytes([slice[6], slice[7]]),
            )),
            _ => None,
        }
    }

    /// The method sets a pixel at the specified index.
    pub fn set_pixel(&mut self, x: usize, pixel: Pixel) {
        if self.bit_depth < 8 {
            let pixels_per_byte = 8 / self.bit_depth as usize;
            let byte_offset = self.pixel_data_offset() + x / pixels_per_byte;
            if byte_offset >= self.data.len() {
                return;
            }

            let value = match (pixel, self.color_type) {
                (Pixel::Gray(v), ColorType::GrayScale) => v as u8,
                (Pixel::Indexed(v), ColorType::IndexColor) => v,
                _ => return,
            };

            let shift = (pixels_per_byte - 1 - (x % pixels_per_byte)) * self.bit_depth as usize;
            let mask = (1 << self.bit_depth) - 1;
            let value = (value & mask) << shift;

            let current_byte = self.data[byte_offset];
            let clean_mask = !(mask << shift);
            self.data[byte_offset] = (current_byte & clean_mask) | value;
            return;
        }

        let bpp = self.bytes_per_pixel();
        let offset = self.pixel_data_offset() + x * bpp;
        if offset + bpp > self.data.len() {
            return;
        }

        let slice = &mut self.data[offset..offset + bpp];
        match (pixel, self.color_type, self.bit_depth) {
            (Pixel::Gray(v), ColorType::GrayScale, 8) => slice[0] = v as u8,
            (Pixel::Gray(v), ColorType::GrayScale, 16) => slice.copy_from_slice(&v.to_be_bytes()),

            (Pixel::RGB(r, g, b), ColorType::TrueColor, 8) => {
                slice[0] = r as u8;
                slice[1] = g as u8;
                slice[2] = b as u8;
            }
            (Pixel::RGB(r, g, b), ColorType::TrueColor, 16) => {
                slice[0..2].copy_from_slice(&r.to_be_bytes());
                slice[2..4].copy_from_slice(&g.to_be_bytes());
                slice[4..6].copy_from_slice(&b.to_be_bytes());
            }

            (Pixel::Indexed(v), ColorType::IndexColor, 8) => slice[0] = v,

            (Pixel::GrayAlpha(v, a), ColorType::GrayScaleAlpha, 8) => {
                slice[0] = v as u8;
                slice[1] = a as u8;
            }
            (Pixel::GrayAlpha(v, a), ColorType::GrayScaleAlpha, 16) => {
                slice[0..2].copy_from_slice(&v.to_be_bytes());
                slice[2..4].copy_from_slice(&a.to_be_bytes());
            }

            (Pixel::RGBA(r, g, b, a), ColorType::TrueColorAlpha, 8) => {
                slice[0] = r as u8;
                slice[1] = g as u8;
                slice[2] = b as u8;
                slice[3] = a as u8;
            }
            (Pixel::RGBA(r, g, b, a), ColorType::TrueColorAlpha, 16) => {
                slice[0..2].copy_from_slice(&r.to_be_bytes());
                slice[2..4].copy_from_slice(&g.to_be_bytes());
                slice[4..6].copy_from_slice(&b.to_be_bytes());
                slice[6..8].copy_from_slice(&a.to_be_bytes());
            }
            _ => {}
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
                1,
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
                    5,
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
                    5,
                );
                scan_line.change_filter_type(FilterType::None, None);
                assert_eq!(FilterType::None, scan_line.filter_type());
            }
            let raw_data = vec![1, 2, 3, 4, 5];
            assert_eq!(raw_data, &buffer[1..]);
        }
    }

    mod pixel {
        use super::*;

        #[test]
        fn test_get_set_pixel_rgba8() {
            let mut buffer = vec![0, 10, 20, 30, 40, 50, 60, 70, 80];
            {
                let mut scan_line = ScanLine::new(
                    &mut buffer,
                    ColorType::TrueColorAlpha,
                    8,
                    2,
                );

                let p = scan_line.get_pixel(0).unwrap();
                assert_eq!(p, Pixel::RGBA(10, 20, 30, 40));

                scan_line.set_pixel(0, Pixel::RGBA(100, 110, 120, 130));
            }
            assert_eq!(buffer[1], 100);
            assert_eq!(buffer[4], 130);

            {
                let scan_line = ScanLine::new(
                    &mut buffer,
                    ColorType::TrueColorAlpha,
                    8,
                    2,
                );
                let p2 = scan_line.get_pixel(1).unwrap();
                assert_eq!(p2, Pixel::RGBA(50, 60, 70, 80));
            }
        }

        #[test]
        fn test_get_set_pixel_gray8() {
            let mut buffer = vec![0, 10, 20, 30];
            {
                let mut scan_line = ScanLine::new(
                    &mut buffer,
                    ColorType::GrayScale,
                    8,
                    3,
                );

                let p = scan_line.get_pixel(1).unwrap();
                assert_eq!(p, Pixel::Gray(20));

                scan_line.set_pixel(2, Pixel::Gray(200));
            }
            assert_eq!(buffer[3], 200);
        }

        #[test]
        fn test_get_set_pixel_sub_8bit() {
            // 1-bit GrayScale
            let mut buffer = vec![0, 0b10110000]; // filter type + 1 byte (8 pixels)
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 1, 8);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Gray(0));
                assert_eq!(scan_line.get_pixel(2).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(4).unwrap(), Pixel::Gray(0));

                scan_line.set_pixel(4, Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(4).unwrap(), Pixel::Gray(1));
            }
            assert_eq!(buffer[1], 0b10111000);

            // 2-bit GrayScale
            let mut buffer = vec![0, 0b11011000]; // 11, 01, 10, 00
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 2, 4);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Gray(3));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(2).unwrap(), Pixel::Gray(2));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(0));

                scan_line.set_pixel(3, Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(1));
            }
            assert_eq!(buffer[1], 0b11011001);

            // 4-bit IndexColor
            let mut buffer = vec![0, 0b10100101]; // 1010, 0101 (10, 5)
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::IndexColor, 4, 2);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Indexed(10));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Indexed(5));

                scan_line.set_pixel(1, Pixel::Indexed(15));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Indexed(15));
            }
            assert_eq!(buffer[1], 0b10101111);
        }
    }
}
