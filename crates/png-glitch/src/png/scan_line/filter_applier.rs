use crate::{FilterType, ScanLine};

pub fn apply(filter_type: FilterType, line: &ScanLine, _: Option<&ScanLine>) {
    match filter_type {
        FilterType::Sub => apply_sub(line),
        _ => apply_none(line)
    }
}

fn apply_none(_: &ScanLine) {}

fn apply_sub(line: &ScanLine) {
    let pixel_size = line.bytes_per_pixel();
    for index in line.pixel_data_range().step_by(pixel_size) {
        if index == 0 {
            continue;
        }
        let mut buffer = line.decoded_data.borrow_mut();
        for offset in 0..pixel_size {
            let i = index + offset;
            let previous = buffer[i - pixel_size];
            let current = buffer[i];
            let filtered = if current > previous { current - previous } else { current + 255 - previous + 1 };
            buffer[i] = filtered;
        }
    }
}