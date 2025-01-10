use crate::ScanLine;

pub trait Scan {
    fn scan_lines(&self) -> Vec<ScanLine>;

    fn foreach_scanline<F>(&self, callback: F)
    where
        F: FnMut(&mut ScanLine);

    fn scan_lines_from(&self, from: usize, lines: usize) -> Vec<ScanLine>;
}