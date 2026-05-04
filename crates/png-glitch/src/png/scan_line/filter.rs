use crate::{FilterType, ScanLine};

mod paeth;
mod sub;
mod up;
mod byte;
mod average;

/// The function removes a filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `previous` parameter is the previous scan line.
pub fn remove(line: &mut ScanLine, previous_line: Option<&ScanLine>) {
    match line.filter_type() {
        FilterType::None => {},
        FilterType::Sub => sub::remove(line),
        FilterType::Up => up::remove(line, previous_line),
        FilterType::Average => average::remove(line, previous_line),
        FilterType::Paeth => paeth::remove(line, previous_line),
    }
}

/// The function applies a filter to a scan line.
/// The `filter_type` parameter is the type of the filter to apply.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(filter_type: FilterType, line: &mut ScanLine, previous: Option<&ScanLine>) {
    match filter_type {
        FilterType::Sub => sub::apply(line),
        FilterType::Up => up::apply(line, previous),
        FilterType::Average => average::apply(line, previous),
        FilterType::Paeth => paeth::apply(line, previous),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;

    #[test]
    fn test_filter_roundtrip() {
        let filter_types = [
            FilterType::None,
            FilterType::Sub,
            FilterType::Up,
            FilterType::Average,
            FilterType::Paeth,
        ];

        for &filter_type in &filter_types {
            let mut previous_data = vec![0, 10, 20, 30, 40, 50, 60, 70, 80];
            let mut current_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
            let current_copy = current_data.clone();

            let previous_line = ScanLine::new(&mut previous_data, ColorType::TrueColorAlpha, 8, 2);
            let mut current_line = ScanLine::new(&mut current_data, ColorType::TrueColorAlpha, 8, 2);

            apply(filter_type, &mut current_line, Some(&previous_line));
            current_line.data[0] = filter_type.into();
            remove(&mut current_line, Some(&previous_line));

            assert_eq!(current_line.data[1..], current_copy[1..], "Failed for {:?}", filter_type);
        }
    }
}