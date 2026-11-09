//! Deterministic webcam-like frames, generated in memory (a 300-frame 1080p RGBA sequence would
//! be about 2.5 GB, so nothing is stored).

use std::str::FromStr;

/// Frame size presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    P360,
    P720,
    P1080,
}

impl Size {
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Size::P360 => (640, 360),
            Size::P720 => (1280, 720),
            Size::P1080 => (1920, 1080),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Size::P360 => "360p",
            Size::P720 => "720p",
            Size::P1080 => "1080p",
        }
    }
}

impl FromStr for Size {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "360p" => Ok(Size::P360),
            "720p" => Ok(Size::P720),
            "1080p" => Ok(Size::P1080),
            other => Err(format!("unknown size `{other}` (use 360p, 720p or 1080p)")),
        }
    }
}

/// SplitMix64, a tiny PRNG with a fixed-seed, platform-independent sequence.
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

const SEED: u64 = 0x706e_6720_6265_6e63;

/// Generates RGBA frame `index`: smooth gradients that drift over time, a few moving shapes and
/// low-amplitude sensor-like noise. The same arguments always give the same bytes.
pub fn frame_rgba(width: u32, height: u32, index: u32) -> Vec<u8> {
    let (w, h) = (width as i64, height as i64);
    let t = index as i64;
    let mut rng = SplitMix64::new(SEED ^ (index as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93));
    // A disc and a box moving in different directions.
    let disc = (
        (w / 4 + t * 7) % w.max(1),
        (h / 2 + t * 3) % h.max(1),
        h.max(8) / 6,
    );
    let boxr = (
        (w * 3 / 4 - t * 5).rem_euclid(w.max(1)),
        h / 3,
        w.max(8) / 10,
        h.max(8) / 8,
    );
    let mut out = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for y in 0..h {
        for x in 0..w {
            let mut r = ((x * 255) / w.max(1)) as i32;
            let mut g = ((y * 255) / h.max(1)) as i32;
            let mut b = (((x + y + t * 2) * 255) / (w + h).max(1)) as i32;
            let (dx, dy) = (x - disc.0, y - disc.1);
            if dx * dx + dy * dy < disc.2 * disc.2 {
                (r, g, b) = (230, 190, 160);
            }
            if (x - boxr.0).abs() < boxr.2 && (y - boxr.1).abs() < boxr.3 {
                (r, g, b) = (40, 90, 200);
            }
            let n = rng.next_u64();
            let noise = |shift: u32| ((n >> shift) & 7) as i32 - 3;
            out.push((r + noise(0)).clamp(0, 255) as u8);
            out.push((g + noise(8)).clamp(0, 255) as u8);
            out.push((b + noise(16)).clamp(0, 255) as u8);
            out.push(255);
        }
    }
    out
}

/// Encodes RGBA pixels as an 8-bit PNG with a fixed filter type (or `None` for the encoder's
/// adaptive choice). This is setup work and is never timed.
pub fn encode_png(width: u32, height: u32, rgba: &[u8], filter: Option<png::Filter>) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        // `set_compression` resets the filter, so it comes first.
        encoder.set_filter(filter.unwrap_or(png::Filter::Adaptive));
        let mut writer = encoder.write_header().expect("PNG header");
        writer.write_image_data(rgba).expect("PNG data");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic() {
        assert_eq!(frame_rgba(64, 36, 3), frame_rgba(64, 36, 3));
        assert_eq!(frame_rgba(64, 36, 3).len(), 64 * 36 * 4);
    }

    #[test]
    fn frames_differ_over_time() {
        assert_ne!(frame_rgba(64, 36, 0), frame_rgba(64, 36, 1));
    }

    #[test]
    fn splitmix_matches_reference_values() {
        // First outputs of SplitMix64 with seed 0 (from the reference implementation).
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn encoded_png_round_trips_through_png_glitch() {
        let rgba = frame_rgba(32, 16, 0);
        let bytes = encode_png(32, 16, &rgba, Some(png::Filter::Sub));
        let glitch = png_glitch::PngGlitch::from_bytes(&bytes).unwrap();
        assert_eq!((glitch.width(), glitch.height()), (32, 16));
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    /// The browser harness (`bench/browser/generator.mjs`) ports `frame_rgba` to JavaScript; it
    /// hard-codes these values in `bench/browser/test-generator.mjs`. Change both together.
    #[test]
    fn frame_checksums_for_the_browser_port() {
        for (size, frame, want) in [
            (Size::P360, 0, 0x5519_3fed_u32),
            (Size::P720, 0, 0x09a4_1720),
            (Size::P1080, 0, 0x79db_4110),
            (Size::P360, 5, 0x58f7_7e6f),
        ] {
            let (w, h) = size.dimensions();
            let got = crc32(&frame_rgba(w, h, frame));
            assert_eq!(got, want, "{} frame {frame}", size.name());
        }
    }

    #[test]
    fn size_parses() {
        assert_eq!("720p".parse::<Size>().unwrap().dimensions(), (1280, 720));
        assert!("4k".parse::<Size>().is_err());
    }
}
