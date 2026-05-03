use crate::{Pixel, ScanLine};

/// A trait for defining reusable glitch effects.
pub trait GlitchPreset {
    /// Applies the effect to a single scan line.
    fn apply_to_line(&self, line: &mut ScanLine);
}

/// A preset that inverts the colors of each pixel.
pub struct Invert;

impl GlitchPreset for Invert {
    fn apply_to_line(&self, line: &mut ScanLine) {
        for x in 0..line.size() / line.bytes_per_pixel() {
            if let Some(pixel) = line.get_pixel(x) {
                let inverted = match pixel {
                    Pixel::Gray(v) => Pixel::Gray(!v),
                    Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(!v, a),
                    Pixel::RGB(r, g, b) => Pixel::RGB(!r, !g, !b),
                    Pixel::RGBA(r, g, b, a) => Pixel::RGBA(!r, !g, !b, a),
                    Pixel::Indexed(v) => Pixel::Indexed(!v),
                };
                line.set_pixel(x, inverted);
            }
        }
    }
}

/// A preset that shifts the color channels of each pixel.
pub struct ShiftChannels {
    pub r: i16,
    pub g: i16,
    pub b: i16,
}

impl GlitchPreset for ShiftChannels {
    fn apply_to_line(&self, line: &mut ScanLine) {
        for x in 0..line.size() / line.bytes_per_pixel() {
            if let Some(pixel) = line.get_pixel(x) {
                let shifted = match pixel {
                    Pixel::RGB(r, g, b) => Pixel::RGB(
                        r.wrapping_add_signed(self.r),
                        g.wrapping_add_signed(self.g),
                        b.wrapping_add_signed(self.b),
                    ),
                    Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                        r.wrapping_add_signed(self.r),
                        g.wrapping_add_signed(self.g),
                        b.wrapping_add_signed(self.b),
                        a,
                    ),
                    _ => pixel,
                };
                line.set_pixel(x, shifted);
            }
        }
    }
}

/// A preset that brightens the colors of each pixel.
pub struct Brighten {
    pub strength: u16,
}

impl GlitchPreset for Brighten {
    fn apply_to_line(&self, line: &mut ScanLine) {
        for x in 0..line.size() / line.bytes_per_pixel() {
            if let Some(pixel) = line.get_pixel(x) {
                let brightened = match pixel {
                    Pixel::Gray(v) => Pixel::Gray(v.saturating_add(self.strength)),
                    Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(v.saturating_add(self.strength), a),
                    Pixel::RGB(r, g, b) => Pixel::RGB(
                        r.saturating_add(self.strength),
                        g.saturating_add(self.strength),
                        b.saturating_add(self.strength),
                    ),
                    Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                        r.saturating_add(self.strength),
                        g.saturating_add(self.strength),
                        b.saturating_add(self.strength),
                        a,
                    ),
                    Pixel::Indexed(v) => Pixel::Indexed(v.saturating_add(self.strength as u8)),
                };
                line.set_pixel(x, brightened);
            }
        }
    }
}
