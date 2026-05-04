use crate::png::scan_line::filter::byte;
use crate::png::scan_line::filter::byte::{add_without_overflow, sub_without_overflow};
use crate::ScanLine;

/// The function removes the average filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `previous` parameter is the previous scan line.
pub fn remove(line: &mut ScanLine, previous: Option<&ScanLine>) {
    scan(line, previous, recon)
}

/// The function applies the average filter to a scan line.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(line: &mut ScanLine, previous: Option<&ScanLine>) {
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

fn scan<F>(line: &mut ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8) -> u8{
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();
    let pixels = range.step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let current = byte::byte_in_pixel(line, pixel, offset);
            let left = byte::byte_in_previous_pixel(line, pixel, offset, bpp);
            let previous = byte::byte_in_previous_line(previous, pixel - line.pixel_data_offset(), offset);
            line.data[pixel + offset] = callback(current, left, previous);
        }
    }
}

fn scan_rev<F>(line: &mut ScanLine, previous: Option<&ScanLine>, callback: F) where F: Fn(u8, u8, u8) -> u8{
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();
    let pixels = range.rev().step_by(bpp);
    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;
            let current = byte::byte_in_pixel(line, index, 0);
            let left = byte::byte_in_previous_pixel(line, index, 0, bpp);
            let previous = byte::byte_in_previous_line(previous, index - line.pixel_data_offset(), 0);

            line.data[pixel - offset] = callback(current, left, previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;

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