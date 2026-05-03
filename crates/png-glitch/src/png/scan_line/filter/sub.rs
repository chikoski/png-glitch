use crate::png::scan_line::filter::byte::{
    add_without_overflow, byte_in_pixel, byte_in_previous_pixel, sub_without_overflow,
};
use crate::ScanLine;

/// The function applies the sub filter to a scan line.
/// The `line` parameter is the scan line to apply the filter to.
pub fn apply(line: &mut ScanLine) {
    fold_rev(line, sub_without_overflow)
}

/// The function removes the sub filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
pub fn remove(line: &mut ScanLine) {
    fold(line, add_without_overflow);
}

fn fold<F>(line: &mut ScanLine, callback: F)
where
    F: Fn(u8, u8) -> u8,
{
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();

    for pixel in range.step_by(bpp) {
        for offset in 0..bpp {
            let current = byte_in_pixel(line, pixel, offset);
            let previous = byte_in_previous_pixel(line, pixel, offset, bpp);

            line.data[pixel + offset] = callback(current, previous);
        }
    }
}

fn fold_rev<F>(line: &mut ScanLine, callback: F)
where
    F: Fn(u8, u8) -> u8,
{
    let bpp = line.bytes_per_pixel();
    let range = line.pixel_data_offset()..line.data.len();
    let pixels = range.rev().step_by(bpp);

    for pixel in pixels {
        for offset in 0..bpp {
            let index = pixel - offset;
            let previous = byte_in_previous_pixel(line, index, 0, bpp);
            let current = byte_in_pixel(line, index, 0);
            line.data[index] = callback(current, previous);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::png::ColorType;

    #[test]
    fn test_unit() {
        let mut original = vec![1, 0, 1, 2, 255, 1, 1, 1, 255];
        let original_copy = original.clone();
        let mut scanline = ScanLine::new(
            &mut original,
            ColorType::TrueColorAlpha,
            8,
        );
        apply(&mut scanline);
        remove(&mut scanline);
        for (before, after) in original_copy.iter().zip(original.iter()) {
            assert_eq!(before, after);
        }
    }
}
