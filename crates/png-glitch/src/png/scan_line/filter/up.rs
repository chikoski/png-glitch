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