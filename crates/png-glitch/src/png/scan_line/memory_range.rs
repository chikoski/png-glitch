use crate::png::scan_line::UsizeRange;
use crate::png::SharedDecodedData;

pub struct MemoryRange {
    pub(super) decoded_data: SharedDecodedData,
    pub(super) range: UsizeRange,
}

impl MemoryRange {
    pub fn new(decoded_data: SharedDecodedData, range: UsizeRange) -> MemoryRange {
        MemoryRange {
            decoded_data,
            range,
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