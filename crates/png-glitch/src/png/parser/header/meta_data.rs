use crate::png::parser::header::color_type::ColorType;

/// A struct representing the metadata of a PNG image.
#[derive(Debug)]
pub struct MetaData {
    /// The width of the PNG image.
    pub width: u32,
    /// The height of the PNG image.
    pub height: u32,
    /// The color type of the PNG image.
    pub color_type: ColorType,
    /// The bit depth of the PNG image.
    pub bit_depth: u8,
}

impl MetaData {
    /// The method creates a new metadata object.
    /// The `width` parameter is the width of the PNG image.
    /// The `height` parameter is the height of the PNG image.
    /// The `color_type` parameter is the color type of the PNG image.
    /// The `bit_depth` parameter is the bit depth of the PNG image.
    pub fn new(width: u32, height: u32, color_type: ColorType, bit_depth: u8) -> MetaData {
        MetaData {
            width,
            height,
            color_type,
            bit_depth,
        }
    }

    /// The method returns the number of bits per scanline.
    pub fn bits_per_scanline(&self) -> usize {
        self.color_type.bit_per_pixel(self.bit_depth) * (self.width as usize)
    }
}
