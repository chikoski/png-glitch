use crate::png::png_error::PngError;

/// An enum representing the color type of a PNG image.
#[derive(Copy, Clone, Debug)]
pub enum ColorType {
    /// Grayscale image.
    GrayScale,
    /// Truecolor image.
    TrueColor,
    /// Indexed-color image.
    IndexColor,
    /// Grayscale image with alpha.
    GrayScaleAlpha,
    /// Truecolor image with alpha.
    TrueColorAlpha,
}

impl ColorType {
    /// The method returns the number of bits per pixel.
    /// The `bit_depth` parameter is the bit depth of the PNG image.
    pub fn bit_per_pixel(&self, bit_depth: u8) -> usize {
        match self {
            Self::GrayScale => bit_depth as usize,
            Self::TrueColor => (bit_depth * 3) as usize,
            Self::IndexColor => bit_depth as usize,
            Self::GrayScaleAlpha => (bit_depth * 2) as usize,
            Self::TrueColorAlpha => (bit_depth * 4) as usize,
        }
    }
}

impl TryFrom<u8> for ColorType {
    type Error = PngError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ColorType::GrayScale),
            2 => Ok(ColorType::TrueColor),
            3 => Ok(ColorType::IndexColor),
            4 => Ok(ColorType::GrayScaleAlpha),
            6 => Ok(ColorType::TrueColorAlpha),
            _ => Err(PngError::InvalidColorType),
        }
    }
}
