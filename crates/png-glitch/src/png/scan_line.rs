use std::io::{Read, Write};
use std::ops::{Index, IndexMut};

pub use filter_type::FilterType;

mod filter_type;

/// ScanLine represents each scan line in a PNG image.
pub struct ScanLine<'a> {
    t: FilterType,
    inner: &'a mut [u8],
}

impl<'a> ScanLine<'a> {
    fn new(filter_type: FilterType, inner: &mut [u8]) -> ScanLine {
        ScanLine {
            t: filter_type,
            inner,
        }
    }

    /// This method returns the filter method applied to the scan line.
    pub fn filter_type(&self) -> FilterType {
        self.t
    }

    /// This methods updates the filter method of the scan line with the specified one.
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.t = filter_type;
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
