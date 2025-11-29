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
    if line.pixel_data_range().contains(&index) {
        line.decoded_data.borrow()[index]
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
    if index < bpp {
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
            let index = index + line.pixel_data_offset() + offset;
            byte_at(line, index)
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