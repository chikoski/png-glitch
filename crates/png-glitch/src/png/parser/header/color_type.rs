use crate::png::png_error::PngError;

#[derive(Copy, Clone, Debug)]
pub enum ColorType {
    GrayScale,
    TrueColor,
    IndexColor,
    GrayScaleAlpha,
    TrueColorAlpha,
}

impl ColorType {
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
