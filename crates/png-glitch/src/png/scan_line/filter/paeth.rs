use crate::png::scan_line::filter::byte::{add_without_overflow, byte_in_pixel, byte_in_previous_line, byte_in_previous_pixel, byte_in_previous_pixel_in_previous_line, sub_without_overflow};
use crate::ScanLine;

pub fn remove(line: &ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, recon)
}

pub fn apply(line: &ScanLine, previous: Option<&ScanLine>) {
    scan_rev(line, previous, filter)
}

fn recon(current: u8, left: u8, top: u8, top_left: u8) -> u8 {
    let p = predict(left, top, top_left);
    add_without_overflow(current, p)
}

fn filter(current: u8, left: u8, top: u8, top_left: u8) -> u8 {
    let p = predict(left, top, top_left);
    sub_without_overflow(current, p)
}

fn scan<F>(line: &ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();
    let pixels = line.pixel_data_range().step_by(bpp);

    for pixel in pixels {
        let pixel_offset = pixel - line.pixel_data_offset();
        for offset in 0..bpp {
            let current = byte_in_pixel(line, pixel, offset);
            let left = byte_in_previous_pixel(line, pixel, offset, bpp);
            let top = byte_in_previous_line(previous, pixel_offset, offset);
            let top_left = byte_in_previous_pixel_in_previous_line(previous, pixel_offset, offset, bpp);

            let updated = callback(current, left, top, top_left);

            let mut buffer = line.decoded_data.borrow_mut();
            let index = pixel + offset;
            buffer[index] = updated;
        }
    }
}

fn scan_rev<F>(line: &ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();
    let pixels = line.pixel_data_range().rev().step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;

            let current = byte_in_pixel(line, index, 0);
            let left = byte_in_previous_pixel(line, index, 0, bpp);

            let index_in_previous_line = index - line.pixel_data_offset();
            let top = byte_in_previous_line(previous, index_in_previous_line, 0);
            let top_left = byte_in_previous_pixel_in_previous_line(previous, index_in_previous_line, 0, bpp);

            let updated = callback(current, left, top, top_left);

            let mut buffer = line.decoded_data.borrow_mut();
            buffer[index] = updated;
        }
    }
}

fn predict(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i16;
    let b = b as i16;
    let c = c as i16;

    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    let pr = if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    };
    pr as u8
}
