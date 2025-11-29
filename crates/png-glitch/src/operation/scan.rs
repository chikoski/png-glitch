use crate::ScanLine;

/// The trait provides methods to read scan lines from a PNG image.
pub trait Scan {
    /// The method returns all scan lines in a PNG image.
    fn scan_lines(&self) -> Vec<ScanLine>;

    /// The method iterates all scan lines in a PNG image.
    /// The `callback` parameter is a function that is called for each scan line.
    fn foreach_scanline<F>(&self, callback: F)
    where
        F: FnMut(&mut ScanLine);

    /// The method returns a chunk of scan lines from a PNG image.
    /// The `from` parameter specifies the starting line of the chunk.
    /// The `lines` parameter specifies the number of lines to be returned.
    fn scan_lines_from(&self, from: usize, lines: usize) -> Vec<ScanLine>;
}