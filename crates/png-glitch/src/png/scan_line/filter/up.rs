use crate::png::scan_line::filter::byte;
use crate::png::scan_line::filter::byte::{byte_in_pixel, byte_in_previous_line};
use crate::ScanLine;

/// The function removes the up filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `other` parameter is the previous scan line.
pub fn remove(line: &mut ScanLine, other: Option<&ScanLine>) {
    scan(line, other, byte::add_without_overflow)
}

/// The function applies the up filter to a scan line.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(line: &mut ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, byte::sub_without_overflow)
}

fn scan<F>(line: &mut ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8) -> u8 {
    let range = line.pixel_data_offset()..line.data.len();
    for index in range {
        let current = byte_in_pixel(line, index, 0);
        let previous = byte_in_previous_line(previous, index - line.pixel_data_offset(), 0);

        line.data[index] = callback(current, previous);
    }
}