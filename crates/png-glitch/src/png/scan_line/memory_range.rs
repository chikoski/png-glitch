use crate::png::scan_line::UsizeRange;
use crate::png::{ColorType, SharedDecodedData};

/// A struct representing a memory range of a scan line.
pub struct MemoryRange {
    pub(super) decoded_data: SharedDecodedData,
    pub(super) range: UsizeRange,
    pub(super) color_type: ColorType,
    pub(super) bit_depth: u8,
}

impl MemoryRange {
    /// The method creates a new memory range.
    /// The `decoded_data` parameter is the decoded data of the PNG image.
    /// The `range` parameter is the range of the scan line in the decoded data.
    /// The `color_type` parameter is the color type of the PNG image.
    /// The `bit_depth` parameter is the bit depth of the PNG image.
    pub fn new(decoded_data: SharedDecodedData, range: UsizeRange, color_type: ColorType, bit_depth: u8) -> MemoryRange {
        MemoryRange {
            decoded_data,
            range,
            color_type,
            bit_depth
        }
    }

    pub(super) fn first_byte(&self) -> Option<u8> {
        let borrowed_decoded_data = self.decoded_data.borrow();
        let index = self.range.start;
        if index < borrowed_decoded_data.len() {
            Some(borrowed_decoded_data[index])
        } else {
            None
        }
    }
}