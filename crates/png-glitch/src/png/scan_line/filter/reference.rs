//! The original per-byte implementation of the PNG filters, kept unchanged as a test oracle for
//! the slice-based kernels in `kernels.rs` (#97). Compiled for tests only.
#![allow(dead_code)]

use crate::{FilterType, ScanLine};

pub mod average;
pub mod byte;
pub mod paeth;
pub mod sub;
pub mod up;

pub fn remove(line: &mut ScanLine, previous_line: Option<&ScanLine>) {
    match line.filter_type() {
        FilterType::None => {}
        FilterType::Sub => sub::remove(line),
        FilterType::Up => up::remove(line, previous_line),
        FilterType::Average => average::remove(line, previous_line),
        FilterType::Paeth => paeth::remove(line, previous_line),
    }
}

pub fn apply(filter_type: FilterType, line: &mut ScanLine, previous: Option<&ScanLine>) {
    match filter_type {
        FilterType::Sub => sub::apply(line),
        FilterType::Up => up::apply(line, previous),
        FilterType::Average => average::apply(line, previous),
        FilterType::Paeth => paeth::apply(line, previous),
        _ => {}
    }
}
