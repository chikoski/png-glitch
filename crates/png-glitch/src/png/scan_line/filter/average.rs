use crate::png::scan_line::filter::byte;
use crate::png::scan_line::filter::byte::{add_without_overflow, sub_without_overflow};
use crate::ScanLine;

pub fn remove(line: &ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, recon)
}

pub fn apply(line: &ScanLine, previous: Option<&ScanLine>) {
    scan_rev(line, previous, filter)
}

fn recon(current: u8, left: u8, previous: u8) -> u8 {
    let left = left as u16;
    let previous = previous as u16;
    let average = ((left + previous) / 2) % 256;
    add_without_overflow(current, average as u8)
}

fn filter(current: u8, left: u8, previous: u8) -> u8 {
    let left = left as u16;
    let previous = previous as u16;
    let average = ((left + previous) / 2) % 256;
    sub_without_overflow(current, average as u8)
}

fn scan<F>(line: &ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8) -> u8{
    let bpp = line.bytes_per_pixel();
    let pixels = line.pixel_data_range().step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let current = byte::byte_in_pixel(line, pixel, offset);
            let left = byte::byte_in_previous_pixel(line, pixel, offset, bpp);
            let previous = byte::byte_in_previous_line(previous, pixel - line.pixel_data_offset(), offset);
            let mut buffer = line.decoded_data.borrow_mut();
            buffer[pixel + offset] = callback(current, left, previous);
        }
    }
}

fn scan_rev<F>(line: &ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8) -> u8{
    let bpp = line.bytes_per_pixel();
    let pixels = line.pixel_data_range().rev().step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;
            let current = byte::byte_in_pixel(line, index, 0);
            let left = byte::byte_in_previous_pixel(line, index, 0, bpp);
            let previous = byte::byte_in_previous_line(previous, index - line.pixel_data_offset(), 0);

            let mut buffer = line.decoded_data.borrow_mut();
            buffer[pixel - offset] = callback(current, left, previous);
        }
    }
}