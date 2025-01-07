use crate::ScanLine;

pub fn add_without_overflow(a: u8, b: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    ((a + b) % 256) as u8
}

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

pub fn byte_in_pixel(line: &ScanLine, index: usize, offset: usize) -> u8 {
    byte_at(line, index + offset)
}

pub fn byte_in_previous_pixel(line: &ScanLine, index: usize, offset: usize, bpp: usize) -> u8 {
    let index = index + offset;
    if index < bpp {
        0
    } else {
        byte_at(line, index - bpp)
    }
}

pub fn byte_in_previous_line(line: Option<&ScanLine>, index: usize, offset: usize) -> u8 {
    match line {
        Some(line) => {
            let index = index + line.pixel_data_offset() + offset;
            byte_at(line, index)
        },
        _ => 0
    }
}

pub fn byte_in_previous_pixel_in_previous_line(line: Option<&ScanLine>, index: usize, offset: usize, bpp: usize) -> u8 {
    if index < bpp {
        0
    } else {
        byte_in_previous_line(line, index - bpp, offset)
    }
}