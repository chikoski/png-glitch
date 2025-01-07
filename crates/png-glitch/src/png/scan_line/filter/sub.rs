use crate::png::scan_line::filter::byte::{add_without_overflow, byte_in_pixel, byte_in_previous_pixel, sub_without_overflow};
use crate::ScanLine;

pub fn apply(line: &ScanLine) {
    fold_rev(line, sub_without_overflow)
}

pub fn remove(line: &ScanLine) {
    fold(line, add_without_overflow);
}

fn fold<F>(line: &ScanLine, callback: F) where F: Fn(u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();

    for pixel in line.pixel_data_range().step_by(bpp) {
        for offset in 0..bpp {
            let current = byte_in_pixel(line, pixel, offset);
            let previous = byte_in_previous_pixel(line, pixel, offset, bpp);
            let mut buffer = line.decoded_data.borrow_mut();

            buffer[pixel + offset] = callback(current, previous);
        }
    }
}

fn fold_rev<F>(line: &ScanLine, callback: F) where F: Fn(u8, u8) -> u8 {
    let bpp = line.bytes_per_pixel();
    let pixels = line.pixel_data_range().rev().step_by(bpp);

    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;
            let previous = byte_in_previous_pixel(line, index, 0, bpp);
            let current = byte_in_pixel(line, index, 0);
            let mut buffer = line.decoded_data.borrow_mut();
            buffer[index] = callback(current, previous);
        }
    }
}


#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::FilterType;
    use crate::png::ColorType;
    use super::*;

    #[test]
    fn test_unit() {
        let original = vec![1, 0, 1, 2, 255, 1, 1, 1, 255];
        let target = Rc::new(RefCell::new(original.clone()));
        let scanline = ScanLine::new(FilterType::Sub, target, 0..original.len(), ColorType::TrueColorAlpha, 8);
        apply(&scanline);
        remove(&scanline);
        for (before, after) in original.iter().zip(scanline.decoded_data.borrow().iter()) {
            assert_eq!(before, after);
        }
    }
}