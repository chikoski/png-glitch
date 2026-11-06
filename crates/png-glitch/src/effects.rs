use crate::{Pixel, PngGlitch, ScanLine};

/// A deterministic operation on a PNG, with parameters.
///
/// An effect receives the whole image, so it can look at more than one scan line.
/// Effects are infallible: out-of-range parameters are clamped or turn the effect
/// into a no-op. Apply an effect with [`PngGlitch::apply`].
///
/// Effects that only transform one scan line at a time should implement [`LineEffect`]
/// instead; every [`LineEffect`] is an [`Effect`] automatically.
pub trait Effect {
    /// Applies the effect to the whole image.
    fn apply_to(&self, png: &mut PngGlitch);
}

/// A helper for implementing [`Effect`]s that transform each scan line independently.
///
/// Anything that implements `LineEffect` (and is `Sync + Send`) implements [`Effect`]
/// automatically, and is applied to all scan lines in parallel.
pub trait LineEffect {
    /// Applies the effect to a single scan line.
    fn apply_to_line(&self, line: &mut ScanLine);
}

impl<T: LineEffect + Sync + Send> Effect for T {
    fn apply_to(&self, png: &mut PngGlitch) {
        png.par_foreach_scanline(|line| self.apply_to_line(line));
    }
}

/// A effect that inverts the colors of each pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Invert;

impl Invert {
    /// Creates a new [`Invert`] effect.
    pub fn new() -> Self {
        Self
    }
}

impl LineEffect for Invert {
    fn apply_to_line(&self, line: &mut ScanLine) {
        line.process_pixels(|_, pixel| match pixel {
            Pixel::Gray(v) => Pixel::Gray(!v),
            Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(!v, a),
            Pixel::RGB(r, g, b) => Pixel::RGB(!r, !g, !b),
            Pixel::RGBA(r, g, b, a) => Pixel::RGBA(!r, !g, !b, a),
            Pixel::Indexed(v) => Pixel::Indexed(!v),
        });
    }
}

/// A effect that shifts the color channels of each pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShiftChannels {
    /// Amount added to the red channel (`-32768..=32767`, wrapping).
    pub r: i16,
    /// Amount added to the green channel (`-32768..=32767`, wrapping).
    pub g: i16,
    /// Amount added to the blue channel (`-32768..=32767`, wrapping).
    pub b: i16,
}

impl ShiftChannels {
    /// Creates a new [`ShiftChannels`] preset with the given channel offsets.
    pub fn new(r: i16, g: i16, b: i16) -> Self {
        Self { r, g, b }
    }
}

impl LineEffect for ShiftChannels {
    fn apply_to_line(&self, line: &mut ScanLine) {
        let r_shift = self.r;
        let g_shift = self.g;
        let b_shift = self.b;
        line.process_pixels(|_, pixel| match pixel {
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
        });
    }
}

/// A effect that increases the brightness of each pixel (saturating at the bit-depth maximum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brighten {
    /// Brightness increment added to each channel (saturating at the bit-depth maximum).
    pub strength: u16,
}

impl Default for Brighten {
    fn default() -> Self {
        Self { strength: 20 }
    }
}

impl Brighten {
    /// Creates a new [`Brighten`] preset with the given strength.
    pub fn new(strength: u16) -> Self {
        Self { strength }
    }
}

impl LineEffect for Brighten {
    fn apply_to_line(&self, line: &mut ScanLine) {
        let strength = self.strength;
        let max_val = ((1u32 << line.bit_depth() as u32).saturating_sub(1)) as u16;
        line.process_pixels(|_, pixel| {
            let brighten = |v: u16| v.saturating_add(strength).min(max_val);
            match pixel {
                Pixel::Gray(v) => Pixel::Gray(brighten(v)),
                Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(brighten(v), a),
                Pixel::RGB(r, g, b) => Pixel::RGB(brighten(r), brighten(g), brighten(b)),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(brighten(r), brighten(g), brighten(b), a),
                Pixel::Indexed(v) => Pixel::Indexed(v.saturating_add(strength as u8)),
            }
        });
    }
}

/// A effect that decreases the brightness of each pixel (saturating at 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Darken {
    /// Brightness decrement subtracted from each channel (saturating at `0`).
    pub strength: u16,
}

impl Default for Darken {
    fn default() -> Self {
        Self { strength: 20 }
    }
}

impl Darken {
    /// Creates a new [`Darken`] preset with the given strength.
    pub fn new(strength: u16) -> Self {
        Self { strength }
    }
}

impl LineEffect for Darken {
    fn apply_to_line(&self, line: &mut ScanLine) {
        let strength = self.strength;
        line.process_pixels(|_, pixel| {
            let darken = |v: u16| v.saturating_sub(strength);
            match pixel {
                Pixel::Gray(v) => Pixel::Gray(darken(v)),
                Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(darken(v), a),
                Pixel::RGB(r, g, b) => Pixel::RGB(darken(r), darken(g), darken(b)),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(darken(r), darken(g), darken(b), a),
                Pixel::Indexed(v) => Pixel::Indexed(v.saturating_sub(strength as u8)),
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
        assert_eq!(line.get_pixel(0).unwrap(), Pixel::RGB(110, 210, 255));
    }

    #[test]
    fn test_darken() {
        let mut data = vec![0, 100, 200, 5];
        let mut line = ScanLine::new(&mut data, ColorType::TrueColor, 8, 1);
        Darken { strength: 10 }.apply_to_line(&mut line);
        assert_eq!(line.get_pixel(0).unwrap(), Pixel::RGB(90, 190, 0));
    }
}

#[cfg(test)]
mod effect_trait_tests {
    use super::*;

    struct FirstRowZero;
    impl Effect for FirstRowZero {
        fn apply_to(&self, png: &mut PngGlitch) {
            if let Some(line) = png.scan_lines().first_mut() {
                line.process_pixels(|_, p| match p {
                    Pixel::RGBA(_, _, _, a) => Pixel::RGBA(0, 0, 0, a),
                    other => other,
                });
            }
        }
    }

    #[test]
    #[allow(deprecated)]
    fn whole_image_effect_and_deprecated_alias_compile() {
        fn takes_effect<E: Effect>(_: E) {}
        takes_effect(FirstRowZero);
        takes_effect(Invert);
        struct Old;
        impl crate::presets::GlitchPreset for Old {
            fn apply_to_line(&self, _: &mut ScanLine) {}
        }
        takes_effect(Old);
    }
}
