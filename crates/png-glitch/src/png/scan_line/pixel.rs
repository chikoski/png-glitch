/// An enum representing a pixel in a PNG image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixel {
    /// Grayscale pixel.
    Gray(u16),
    /// Grayscale pixel with alpha.
    GrayAlpha(u16, u16),
    /// Truecolor pixel.
    RGB(u16, u16, u16),
    /// Truecolor pixel with alpha.
    RGBA(u16, u16, u16, u16),
    /// Indexed-color pixel.
    Indexed(u8),
}

impl Pixel {
    /// Returns the red (or gray) component of the pixel.
    pub fn r(&self) -> u16 {
        match self {
            Self::Gray(v) => *v,
            Self::GrayAlpha(v, _) => *v,
            Self::RGB(r, _, _) => *r,
            Self::RGBA(r, _, _, _) => *r,
            Self::Indexed(v) => *v as u16,
        }
    }

    /// Returns the green component of the pixel.
    pub fn g(&self) -> u16 {
        match self {
            Self::RGB(_, g, _) => *g,
            Self::RGBA(_, g, _, _) => *g,
            _ => 0,
        }
    }

    /// Returns the blue component of the pixel.
    pub fn b(&self) -> u16 {
        match self {
            Self::RGB(_, _, b) => *b,
            Self::RGBA(_, _, b, _) => *b,
            _ => 0,
        }
    }

    /// Returns the alpha component of the pixel.
    pub fn a(&self) -> u16 {
        match self {
            Self::GrayAlpha(_, a) => *a,
            Self::RGBA(_, _, _, a) => *a,
            _ => 0xFFFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_accessors() {
        let gray = Pixel::Gray(100);
        assert_eq!(gray.r(), 100);
        assert_eq!(gray.g(), 0);
        assert_eq!(gray.b(), 0);
        assert_eq!(gray.a(), 0xFFFF);

        let gray_alpha = Pixel::GrayAlpha(100, 200);
        assert_eq!(gray_alpha.r(), 100);
        assert_eq!(gray_alpha.g(), 0);
        assert_eq!(gray_alpha.b(), 0);
        assert_eq!(gray_alpha.a(), 200);

        let rgb = Pixel::RGB(10, 20, 30);
        assert_eq!(rgb.r(), 10);
        assert_eq!(rgb.g(), 20);
        assert_eq!(rgb.b(), 30);
        assert_eq!(rgb.a(), 0xFFFF);

        let rgba = Pixel::RGBA(10, 20, 30, 40);
        assert_eq!(rgba.r(), 10);
        assert_eq!(rgba.g(), 20);
        assert_eq!(rgba.b(), 30);
        assert_eq!(rgba.a(), 40);

        let indexed = Pixel::Indexed(50);
        assert_eq!(indexed.r(), 50);
        assert_eq!(indexed.g(), 0);
        assert_eq!(indexed.b(), 0);
        assert_eq!(indexed.a(), 0xFFFF);
    }
}

