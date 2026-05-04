use crate::png::scan_line::filter::byte::{add_without_overflow, byte_in_pixel, byte_in_previous_line, byte_in_previous_pixel, byte_in_previous_pixel_in_previous_line, sub_without_overflow};
use crate::ScanLine;

/// The function removes the paeth filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `previous` parameter is the previous scan line.
pub fn remove(line: &mut ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, recon)
}

/// The function applies the paeth filter to a scan line.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(line: &mut ScanLine, previous: Option<&ScanLine>) {
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

fn scan<F>(line: &mut ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();
    let pixels = range.step_by(bpp);

    for pixel in pixels {
        let pixel_offset = pixel - line.pixel_data_offset();
        for offset in 0..bpp {
            let current = byte_in_pixel(line, pixel, offset);
            let left = byte_in_previous_pixel(line, pixel, offset, bpp);
            let top = byte_in_previous_line(previous, pixel_offset, offset);
            let top_left = byte_in_previous_pixel_in_previous_line(previous, pixel_offset, offset, bpp);

            let updated = callback(current, left, top, top_left);
            line.data[pixel + offset] = updated;
        }
    }
}

fn scan_rev<F>(line: &mut ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();
    let pixels = range.rev().step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;

            let current = byte_in_pixel(line, index, 0);
            let left = byte_in_previous_pixel(line, index, 0, bpp);

            let index_in_previous_line = index - line.pixel_data_offset();
            let top = byte_in_previous_line(previous, index_in_previous_line, 0);
            let top_left = byte_in_previous_pixel_in_previous_line(previous, index_in_previous_line, 0, bpp);

            let updated = callback(current, left, top, top_left);
            line.data[index] = updated;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;

    #[test]
    fn test_predict() {
        assert_eq!(predict(10, 20, 30), 10); // p = 10+20-30 = 0. pa=10, pb=20, pc=30. a is closest.
        assert_eq!(predict(100, 100, 100), 100);
        assert_eq!(predict(0, 0, 0), 0);
    }

    #[test]
    fn test_unit() {
        let mut previous_data = vec![0, 10, 20, 30, 40, 50, 60, 70, 80];
        let mut current_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
        let current_copy = current_data.clone();

        let previous_line = ScanLine::new(&mut previous_data, ColorType::TrueColorAlpha, 8, 2);
        let mut current_line = ScanLine::new(&mut current_data, ColorType::TrueColorAlpha, 8, 2);

        apply(&mut current_line, Some(&previous_line));
        remove(&mut current_line, Some(&previous_line));

        assert_eq!(current_data, current_copy);
    }

    #[test]
    fn test_unit_no_previous() {
        let mut current_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
        let current_copy = current_data.clone();

        let mut current_line = ScanLine::new(&mut current_data, ColorType::TrueColorAlpha, 8, 2);

        apply(&mut current_line, None);
        remove(&mut current_line, None);

        assert_eq!(current_data, current_copy);
    }
}

