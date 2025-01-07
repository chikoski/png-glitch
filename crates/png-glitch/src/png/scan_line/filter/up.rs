use crate::png::scan_line::filter::byte;
use crate::png::scan_line::filter::byte::{byte_in_pixel, byte_in_previous_line};
use crate::ScanLine;

pub fn remove(line: &ScanLine, other: Option<&ScanLine>) {
    scan(line, other, byte::add_without_overflow)
}

pub fn apply(line: &ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, byte::sub_without_overflow)
}

fn scan<F>(line: &ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8) -> u8 {
    for index in line.pixel_data_range() {
        let current = byte_in_pixel(line, index, 0);
        let previous = byte_in_previous_line(previous, index - line.pixel_data_offset(), 0);

        let mut buffer = line.decoded_data.borrow_mut();
        buffer[index] = callback(current, previous);
    }
}