use std::io::{Read, Write};
use std::ops::{Index, IndexMut};

pub use filter_type::FilterType;

mod filter_type;

/// ScanLine represents each scan line in a PNG image.
pub struct ScanLine<'a> {
    filter_type: FilterType,
    inner: &'a mut [u8],
}

impl<'a> ScanLine<'a> {
    fn new(filter_type: FilterType, inner: &mut [u8]) -> ScanLine {
        ScanLine { filter_type, inner }
    }

    /// This method returns the filter method applied to the scan line.
    pub fn filter_type(&self) -> FilterType {
        self.filter_type
    }

    /// This method updates the filter method of the scan line with the specified one.
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.inner[0] = filter_type.into();
    }

    /// This method returns the byte size of the scan line.
    pub fn size(&self) -> usize {
        self.inner.len() - 1
    }
}

impl<'a> Read for ScanLine<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut buffer = &self.inner[1..];
        buffer.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut buffer = &self.inner[1..];
        buffer.read_to_end(buf)
    }
}

impl<'a> Write for ScanLine<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = &mut self.inner[1..];
        buffer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Index<usize> for ScanLine<'a> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        let buffer = &self.inner[1..];
        Index::index(buffer, index)
    }
}

impl<'a> IndexMut<usize> for ScanLine<'a> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let buffer = &mut self.inner[1..];
        IndexMut::index_mut(buffer, index)
    }
}

impl<'a> TryFrom<&'a mut [u8]> for ScanLine<'a> {
    type Error = anyhow::Error;

    fn try_from(value: &'a mut [u8]) -> Result<Self, Self::Error> {
        let filter_type = FilterType::try_from(value[0])?;
        Ok(ScanLine::new(filter_type, value))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TestTarget {
        buffer: Vec<u8>,
    }

    impl<'a> TestTarget {
        fn new() -> Self {
            let buffer = vec![0, 1, 2, 3, 4, 5];
            TestTarget {
                buffer,
            }
        }

        fn scan_line(&'a mut self) -> ScanLine<'a> {
            ScanLine::new(FilterType::None, &mut self.buffer)
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
            assert_eq!(true, result.is_ok());
            assert_eq!(scan_line.size(), buffer.len());
            assert_eq!(&scan_line.inner[1..], &buffer);
        }

        #[test]
        fn test_read_to_end() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![];

            let size = scan_line.size();
            let result = scan_line.read_to_end(&mut buffer);
            assert_eq!(true, result.is_ok());
            assert_eq!(&scan_line.inner[1..], &buffer[0..size]);
        }
    }

    mod write {
        use super::*;

        #[test]
        fn test_write() {
            let mut taget = TestTarget::new();
            let mut scan_line = taget.scan_line();
            let size = scan_line.size();

            let buffer = vec![10; size];
            let result = scan_line.write(&buffer);
            assert_eq!(true, result.is_ok());
            assert_eq!(buffer.len(), result.unwrap());
            assert_eq!(&buffer, &scan_line.inner[1..]);
        }
    }
}