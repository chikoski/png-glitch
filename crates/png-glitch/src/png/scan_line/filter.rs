use crate::{FilterType, ScanLine};

mod paeth;
mod sub;
mod up;
mod byte;
mod average;

pub fn remove(line: &ScanLine, previous: Option<&ScanLine>) {
    match line.filter_type {
        FilterType::None => {},
        FilterType::Sub => sub::remove(line),
        FilterType::Up => up::remove(line, previous),
        FilterType::Average => average::remove(line, previous),
        FilterType::Paeth => paeth::remove(line, previous),
    }
}

pub fn apply(filter_type: FilterType, line: &ScanLine, previous: Option<&ScanLine>) {
    match filter_type {
        FilterType::Sub => sub::apply(line),
        FilterType::Up => up::apply(line, previous),
        FilterType::Average => average::apply(line, previous),
        FilterType::Paeth => paeth::apply(line, previous),
        _ => {}
    }
}