use crate::png::parser::header::color_type::ColorType;

#[derive(Debug)]
pub struct MetaData {
    pub width: u32,
    pub height: u32,
    pub color_type: ColorType,
    pub bit_depth: u8,
}

impl MetaData {
    pub fn new(
        width: u32,
        height: u32,
        color_type: ColorType,
        bit_depth: u8,
    ) -> MetaData {
        MetaData{width, height, color_type, bit_depth}
    }

    pub fn bits_per_scanline(&self) -> usize {
        self.color_type.bit_per_pixel(self.bit_depth) * (self.width as usize)
    }
}