use crate::{FilterType, ScanLine};
use crate::png::scan_line::paeth;

pub fn remove(line: &ScanLine, previous: Option<&ScanLine>) {
    match line.filter_type {
        FilterType::None => FilterRemover::remove_none(line),
        FilterType::Sub => FilterRemover::remove_sub(line),
        FilterType::Up => FilterRemover::remove_up(line, previous),
        FilterType::Average => FilterRemover::remove_average(line, previous),
        FilterType::Paeth => FilterRemover::remove_paeth(line, previous),
    }
}

struct FilterRemover;

impl FilterRemover {
    fn remove_none(_: &ScanLine) {}

    fn remove_sub(line: &ScanLine) {
        let bpp = line.bytes_per_pixel();
        let pixel_start = line.pixel_data_offset();
        let mut buffer = line.decoded_data.borrow_mut();

        for base in line.pixel_data_range().step_by(bpp) {
            for offset in 0..bpp {
                let index = base + offset;
                let previous = if index < bpp || index - bpp < pixel_start {
                    0
                } else {
                    buffer[index - bpp] as u16
                };
                let current = buffer[index] as u16;
                buffer[index] = ((previous + current) % 256) as u8;
            }
        }
    }

    fn remove_up(line: &ScanLine, other: Option<&ScanLine>) {
        if let Some(other) = other {
            if other.filter_type == FilterType::Up {
                let mut buffer = line.decoded_data.borrow_mut();
                for (pixel, previous) in line.pixel_data_range().zip(other.pixel_data_range()) {
                    buffer[pixel] = buffer[pixel] + buffer[previous];
                }
            }
        }
    }

    fn remove_average(line: &ScanLine, other: Option<&ScanLine>) {
        let mut buffer = line.decoded_data.borrow_mut();
        for index in line.pixel_data_range() {
            let previous = if index == line.pixel_data_offset() {
                0
            } else {
                buffer[index - 1]
            };
            let up = match other {
                Some(other) => {
                    if other.filter_type == FilterType::Average {
                        let other = other.pixel_data_offset() + index;
                        buffer[other]
                    } else {
                        0
                    }
                }
                None => 0
            };
            buffer[index] = buffer[index] + (up + previous) / 2
        }
    }

    fn remove_paeth(line: &ScanLine, other: Option<&ScanLine>) {
        if let Some(other) = other {
            if other.filter_type == FilterType::Paeth {
                let mut buffer = line.decoded_data.borrow_mut();
                for (pixel, previous) in line.pixel_data_range().zip(other.pixel_data_range()) {
                    let top_left = if previous == other.pixel_data_offset() {
                        0
                    } else {
                        buffer[previous - 1]
                    };
                    let top = buffer[previous];
                    let left = if pixel == line.pixel_data_offset() {
                        0
                    } else {
                        buffer[pixel - 1]
                    };
                    buffer[pixel] += paeth::predict(top_left, top, left);
                }
            }
        }
    }    
}