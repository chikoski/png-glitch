use crate::png::png_error::PngError;

/// An enum representing the color type of a PNG image.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
        let depth = bit_depth as usize;
        match self {
            Self::GrayScale => depth,
            Self::TrueColor => depth * 3,
            Self::IndexColor => depth,
            Self::GrayScaleAlpha => depth * 2,
            Self::TrueColorAlpha => depth * 4,
        }
    }

    /// Returns the numeric PNG color type code (`0`, `2`, `3`, `4`, or `6`).
    pub fn code(&self) -> u8 {
        match self {
            Self::GrayScale => 0,
            Self::TrueColor => 2,
            Self::IndexColor => 3,
            Self::GrayScaleAlpha => 4,
            Self::TrueColorAlpha => 6,
        }
    }

    /// Returns `true` if `bit_depth` is permitted for this color type by the PNG specification (§11.2.2).
    pub fn is_valid_bit_depth(&self, bit_depth: u8) -> bool {
        match self {
            Self::GrayScale => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
            Self::TrueColor => matches!(bit_depth, 8 | 16),
            Self::IndexColor => matches!(bit_depth, 1 | 2 | 4 | 8),
            Self::GrayScaleAlpha => matches!(bit_depth, 8 | 16),
            Self::TrueColorAlpha => matches!(bit_depth, 8 | 16),
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
