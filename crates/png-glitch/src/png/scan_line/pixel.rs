use crate::png::ColorType;

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

/// Scales a `bit_depth`-bit sample up to 16 bits by bit replication, as the PNG specification
/// recommends: `v * (65535 / (2^bit_depth - 1))`, so `0` stays `0` and the maximum becomes
/// `0xFFFF` (8 bits: `v * 257`; 4 bits: `v * 0x1111`; 2 bits: `v * 0x5555`; 1 bit: `v * 0xFFFF`).
fn widen(v: u16, bit_depth: u8) -> Option<u16> {
    match bit_depth {
        1 | 2 | 4 | 8 | 16 => {
            let max = (1u32 << bit_depth) - 1;
            Some((u32::from(v).min(max) * (0xFFFF / max)) as u16)
        }
        _ => None,
    }
}

/// Scales a 16-bit sample down to `bit_depth` bits by keeping the high bits (`v >> (16 - d)`).
fn narrow(v: u16, bit_depth: u8) -> Option<u16> {
    match bit_depth {
        1 | 2 | 4 | 8 | 16 => Some(v >> (16 - bit_depth)),
        _ => None,
    }
}

/// Integer Rec. 601 luma of 16-bit channels: `(299 r + 587 g + 114 b + 500) / 1000`.
fn luma601(r: u16, g: u16, b: u16) -> u16 {
    ((299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) + 500) / 1000) as u16
}

impl Pixel {
    /// The pixel as `[r, g, b, a]` with 16-bit channels, for an image of `bit_depth` bits per
    /// sample. A pixel's samples are raw values at the image's depth (`Gray(255)` at 8 bits,
    /// `Gray(3)` at 2 bits), so the depth is needed to scale them.
    ///
    /// Samples are scaled up by bit replication (`v * (65535 / (2^depth - 1))`; at 8 bits `v *
    /// 257`), values above the depth's maximum are clamped to it, gray is copied to `r`, `g` and
    /// `b`, and a pixel without alpha gets `0xFFFF`. Returns `None` for [`Pixel::Indexed`] (it
    /// needs the palette) and for a depth other than 1, 2, 4, 8 or 16.
    ///
    /// # Examples
    /// ```
    /// use png_glitch::Pixel;
    ///
    /// assert_eq!(Pixel::RGB(255, 0, 128).to_rgba16(8), Some([0xFFFF, 0, 128 * 257, 0xFFFF]));
    /// assert_eq!(Pixel::Gray(3).to_rgba16(2), Some([0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF]));
    /// assert_eq!(Pixel::Indexed(1).to_rgba16(8), None);
    /// ```
    pub fn to_rgba16(&self, bit_depth: u8) -> Option<[u16; 4]> {
        let w = |v: u16| widen(v, bit_depth);
        Some(match *self {
            Self::Gray(v) => {
                let g = w(v)?;
                [g, g, g, 0xFFFF]
            }
            Self::GrayAlpha(v, a) => {
                let g = w(v)?;
                [g, g, g, w(a)?]
            }
            Self::RGB(r, g, b) => [w(r)?, w(g)?, w(b)?, 0xFFFF],
            Self::RGBA(r, g, b, a) => [w(r)?, w(g)?, w(b)?, w(a)?],
            Self::Indexed(_) => return None,
        })
    }

    /// The pixel of a `color_type` image with `bit_depth` bits per sample that best matches the
    /// 16-bit `[r, g, b, a]`: the inverse of [`to_rgba16`](Self::to_rgba16).
    ///
    /// Channels are scaled down by keeping the high bits (`v >> (16 - depth)`). For a gray image
    /// the gray value is the integer Rec. 601 luma `(299 r + 587 g + 114 b + 500) / 1000`, and
    /// alpha is dropped for a type without alpha. Returns `None` for [`ColorType::IndexColor`]
    /// and for a depth other than 1, 2, 4, 8 or 16 (colour types with colour only allow 8 and 16).
    ///
    /// # Examples
    /// ```
    /// use png_glitch::{ColorType, Pixel};
    ///
    /// let rgba = [0x1234, 0xABCD, 0xFFFF, 0x8000];
    /// assert_eq!(
    ///     Pixel::from_rgba16(rgba, ColorType::TrueColorAlpha, 8),
    ///     Some(Pixel::RGBA(0x12, 0xAB, 0xFF, 0x80))
    /// );
    /// assert_eq!(Pixel::from_rgba16([0, 0, 0, 0], ColorType::IndexColor, 8), None);
    /// ```
    pub fn from_rgba16(rgba: [u16; 4], color_type: ColorType, bit_depth: u8) -> Option<Pixel> {
        let [r, g, b, a] = rgba;
        let n = |v: u16| narrow(v, bit_depth);
        let colour_depth = matches!(bit_depth, 8 | 16);
        match color_type {
            ColorType::GrayScale => Some(Self::Gray(n(luma601(r, g, b))?)),
            ColorType::GrayScaleAlpha if colour_depth => {
                Some(Self::GrayAlpha(n(luma601(r, g, b))?, n(a)?))
            }
            ColorType::TrueColor if colour_depth => Some(Self::RGB(n(r)?, n(g)?, n(b)?)),
            ColorType::TrueColorAlpha if colour_depth => {
                Some(Self::RGBA(n(r)?, n(g)?, n(b)?, n(a)?))
            }
            _ => None,
        }
    }

    /// The luma of the pixel as a 16-bit value, for an image of `bit_depth` bits per sample:
    /// the gray value scaled up as in [`to_rgba16`](Self::to_rgba16), or for a colour pixel the
    /// integer Rec. 601 luma `(299 r + 587 g + 114 b + 500) / 1000` of the scaled channels.
    /// `None` for [`Pixel::Indexed`] and for an unsupported depth.
    pub fn luma16(&self, bit_depth: u8) -> Option<u16> {
        let [r, g, b, _] = self.to_rgba16(bit_depth)?;
        Some(match self {
            Self::Gray(_) | Self::GrayAlpha(..) => r,
            _ => luma601(r, g, b),
        })
    }

    /// Applies `f` to every colour sample (the gray value, or `r`, `g` and `b`) and leaves alpha
    /// alone. `f` sees and returns **raw samples at the image's bit depth**, so it has to keep
    /// the result in range (use [`ScanLine::try_set_pixel`](crate::ScanLine::try_set_pixel) to
    /// check). An [`Pixel::Indexed`] pixel is returned unchanged.
    ///
    /// # Examples
    /// ```
    /// use png_glitch::Pixel;
    ///
    /// let inverted = Pixel::RGBA(10, 20, 30, 40).map_channels(|v| 255 - v);
    /// assert_eq!(inverted, Pixel::RGBA(245, 235, 225, 40));
    /// ```
    pub fn map_channels(self, mut f: impl FnMut(u16) -> u16) -> Pixel {
        match self {
            Self::Gray(v) => Self::Gray(f(v)),
            Self::GrayAlpha(v, a) => Self::GrayAlpha(f(v), a),
            Self::RGB(r, g, b) => Self::RGB(f(r), f(g), f(b)),
            Self::RGBA(r, g, b, a) => Self::RGBA(f(r), f(g), f(b), a),
            Self::Indexed(v) => Self::Indexed(v),
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
