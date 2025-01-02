use crate::ScanLine;

pub trait Scan {
    fn scan_lines(&mut self) -> Vec<ScanLine>;

    fn foreach_scanline<F>(&mut self, callback: F)
    where
        F: FnMut(&mut ScanLine);

}