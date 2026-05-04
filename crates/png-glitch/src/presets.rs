use crate::{Pixel, ScanLine};

/// A trait for defining reusable glitch effects.
pub trait GlitchPreset {
    /// Applies the effect to a single scan line.
    fn apply_to_line(&self, line: &mut ScanLine);
}

/// A preset that inverts the colors of each pixel.
#[derive(Clone, Copy, Debug)]
pub struct Invert;

impl GlitchPreset for Invert {
    fn apply_to_line(&self, line: &mut ScanLine) {
        line.process_pixels(|_, pixel| {
            match pixel {
                Pixel::Gray(v) => Pixel::Gray(!v),
                Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(!v, a),
                Pixel::RGB(r, g, b) => Pixel::RGB(!r, !g, !b),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(!r, !g, !b, a),
                Pixel::Indexed(v) => Pixel::Indexed(!v),
            }
        });
    }
}

/// A preset that shifts the color channels of each pixel.
#[derive(Clone, Copy, Debug)]
pub struct ShiftChannels {
    pub r: i16,
    pub g: i16,
    pub b: i16,
}

impl GlitchPreset for ShiftChannels {
    fn apply_to_line(&self, line: &mut ScanLine) {
        let r_shift = self.r;
        let g_shift = self.g;
        let b_shift = self.b;
        line.process_pixels(|_, pixel| {
            match pixel {
                Pixel::RGB(r, g, b) => Pixel::RGB(
                    r.wrapping_add_signed(r_shift),
                    g.wrapping_add_signed(g_shift),
                    b.wrapping_add_signed(b_shift),
                ),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                    r.wrapping_add_signed(r_shift),
                    g.wrapping_add_signed(g_shift),
                    b.wrapping_add_signed(b_shift),
                    a,
                ),
                _ => pixel,
            }
        });
    }
}

/// A preset that brightens the colors of each pixel.
#[derive(Clone, Copy, Debug)]
pub struct Brighten {
    pub strength: u16,
}

impl GlitchPreset for Brighten {
    fn apply_to_line(&self, line: &mut ScanLine) {
        let strength = self.strength;
        line.process_pixels(|_, pixel| {
            match pixel {
                Pixel::Gray(v) => Pixel::Gray(v.saturating_add(strength)),
                Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(v.saturating_add(strength), a),
                Pixel::RGB(r, g, b) => Pixel::RGB(
                    r.saturating_add(strength),
                    g.saturating_add(strength),
                    b.saturating_add(strength),
                ),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                    r.saturating_add(strength),
                    g.saturating_add(strength),
                    b.saturating_add(strength),
                    a,
                ),
                Pixel::Indexed(v) => Pixel::Indexed(v.saturating_add(strength as u8)),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;

    #[test]
    fn test_invert() {
        let mut data = vec![0, 10, 20, 30]; // filter type + RGB
        let mut line = ScanLine::new(&mut data, ColorType::TrueColor, 8, 1);
        Invert.apply_to_line(&mut line);
        // !10 as u16 is 0xFFF5, which as u8 is 245.
        assert_eq!(line.get_pixel(0).unwrap(), Pixel::RGB(245, 235, 225));
    }

    #[test]
    fn test_shift_channels() {
        let mut data = vec![0, 10, 20, 30];
        let mut line = ScanLine::new(&mut data, ColorType::TrueColor, 8, 1);
        ShiftChannels { r: 1, g: -1, b: 0 }.apply_to_line(&mut line);
        assert_eq!(line.get_pixel(0).unwrap(), Pixel::RGB(11, 19, 30));
    }

    #[test]
    fn test_brighten() {
        let mut data = vec![0, 100, 200, 250];
        let mut line = ScanLine::new(&mut data, ColorType::TrueColor, 8, 1);
        Brighten { strength: 10 }.apply_to_line(&mut line);
        // 250 + 10 = 260. 260 as u8 is 4.
        assert_eq!(line.get_pixel(0).unwrap(), Pixel::RGB(110, 210, 4));
    }
}
