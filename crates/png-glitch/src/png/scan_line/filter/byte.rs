use crate::ScanLine;

/// The function adds two bytes without overflow.
/// The `a` parameter is the first byte.
/// The `b` parameter is the second byte.
pub fn add_without_overflow(a: u8, b: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    ((a + b) % 256) as u8
}

/// The function subtracts two bytes without overflow.
/// The `a` parameter is the first byte.
/// The `b` parameter is the second byte.
pub fn sub_without_overflow(a: u8, b: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    ((a + 256 - b) % 256) as u8
}

fn byte_at(line: &ScanLine, index: usize) -> u8 {
    if index < line.data.len() {
        line.data[index]
    } else {
        0
    }
}

/// The function returns a byte in a pixel.
/// The `line` parameter is the scan line.
/// The `index` parameter is the index of the pixel.
/// The `offset` parameter is the offset of the byte in the pixel.
pub fn byte_in_pixel(line: &ScanLine, index: usize, offset: usize) -> u8 {
    byte_at(line, index + offset)
}

/// The function returns a byte in the previous pixel.
/// The `line` parameter is the scan line.
/// The `index` parameter is the index of the pixel.
/// The `offset` parameter is the offset of the byte in the pixel.
/// The `bpp` parameter is the number of bytes per pixel.
pub fn byte_in_previous_pixel(line: &ScanLine, index: usize, offset: usize, bpp: usize) -> u8 {
    let index = index + offset;
    let limit = line.pixel_data_offset() + bpp;
    if index < limit {
        0
    } else {
        byte_at(line, index - bpp)
    }
}

/// The function returns a byte in the previous line.
/// The `line` parameter is the previous scan line.
/// The `index` parameter is the index of the pixel.
/// The `offset` parameter is the offset of the byte in the pixel.
pub fn byte_in_previous_line(line: Option<&ScanLine>, index: usize, offset: usize) -> u8 {
    match line {
        Some(line) => {
            let abs_index = index + line.pixel_data_offset() + offset;
            byte_at(line, abs_index)
        },
        _ => 0
    }
}

/// The function returns a byte in the previous pixel in the previous line.
/// The `line` parameter is the previous scan line.
/// The `index` parameter is the index of the pixel.
/// The `offset` parameter is the offset of the byte in the pixel.
/// The `bpp` parameter is the number of bytes per pixel.
pub fn byte_in_previous_pixel_in_previous_line(line: Option<&ScanLine>, index: usize, offset: usize, bpp: usize) -> u8 {
    if index < bpp {
        0
    } else {
        byte_in_previous_line(line, index - bpp, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;

    #[test]
    fn test_add_without_overflow() {
        assert_eq!(add_without_overflow(10, 20), 30);
        assert_eq!(add_without_overflow(250, 10), 4);
        assert_eq!(add_without_overflow(255, 1), 0);
    }

    #[test]
    fn test_sub_without_overflow() {
        assert_eq!(sub_without_overflow(30, 10), 20);
        assert_eq!(sub_without_overflow(10, 30), 236);
        assert_eq!(sub_without_overflow(0, 1), 255);
    }

    #[test]
    fn test_byte_in_pixel() {
        let mut data = vec![0, 1, 2, 3, 4, 5];
        let line = ScanLine::new(&mut data, ColorType::GrayScale, 8, 5);
        assert_eq!(byte_in_pixel(&line, 1, 0), 1);
        assert_eq!(byte_in_pixel(&line, 5, 0), 5);
        assert_eq!(byte_in_pixel(&line, 6, 0), 0); // Out of bounds
    }

    #[test]
    fn test_byte_in_previous_pixel() {
        let mut data = vec![0, 10, 20, 30, 40, 50]; // filter type (1 byte) + 5 data bytes
        let line = ScanLine::new(&mut data, ColorType::GrayScale, 8, 5);
        let bpp = line.bytes_per_pixel(); // 1
        assert_eq!(byte_in_previous_pixel(&line, 1, 0, bpp), 0); // First pixel has no previous
        assert_eq!(byte_in_previous_pixel(&line, 2, 0, bpp), 10);
        assert_eq!(byte_in_previous_pixel(&line, 5, 0, bpp), 40);
    }

    #[test]
    fn test_byte_in_previous_line() {
        let mut data1 = vec![0, 10, 20, 30];
        let line1 = ScanLine::new(&mut data1, ColorType::GrayScale, 8, 3);

        assert_eq!(byte_in_previous_line(Some(&line1), 0, 0), 10);
        assert_eq!(byte_in_previous_line(Some(&line1), 1, 0), 20);
        assert_eq!(byte_in_previous_line(None, 0, 0), 0);
    }

    #[test]
    fn test_byte_in_previous_pixel_in_previous_line() {
        let mut data1 = vec![0, 10, 20, 30];
        let line1 = ScanLine::new(&mut data1, ColorType::GrayScale, 8, 3);
        let bpp = 1;

        assert_eq!(byte_in_previous_pixel_in_previous_line(Some(&line1), 0, 0, bpp), 0);
        assert_eq!(byte_in_previous_pixel_in_previous_line(Some(&line1), 1, 0, bpp), 10);
        assert_eq!(byte_in_previous_pixel_in_previous_line(Some(&line1), 2, 0, bpp), 20);
        assert_eq!(byte_in_previous_pixel_in_previous_line(None, 1, 0, bpp), 0);
    }
}