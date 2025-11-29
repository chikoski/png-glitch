use crate::{FilterType, ScanLine};

mod paeth;
mod sub;
mod up;
mod byte;
mod average;

/// The function removes a filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `previous` parameter is the previous scan line.
pub fn remove(line: &ScanLine, previous: Option<&ScanLine>) {
    match line.filter_type {
        FilterType::None => {},
        FilterType::Sub => sub::remove(line),
        FilterType::Up => up::remove(line, previous),
        FilterType::Average => average::remove(line, previous),
        FilterType::Paeth => paeth::remove(line, previous),
    }
}

/// The function applies a filter to a scan line.
/// The `filter_type` parameter is the type of the filter to apply.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(filter_type: FilterType, line: &ScanLine, previous: Option<&ScanLine>) {
    match filter_type {
        FilterType::Sub => sub::apply(line),
        FilterType::Up => up::apply(line, previous),
        FilterType::Average => average::apply(line, previous),
        FilterType::Paeth => paeth::apply(line, previous),
        _ => {}
    }
}