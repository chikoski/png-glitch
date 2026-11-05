pub mod error;
pub mod filter;
pub mod pixel;
pub mod riff;
pub mod scan_line;

pub use error::WebpError;
pub use filter::WebpGlitchFilter;
pub use pixel::{PixelBuffer, WebpPixel};
pub use scan_line::ScanLine;

use riff::RiffContainer;
use std::path::Path;

/// Main struct for decoding, manipulating, and re-encoding WebP images.
pub struct WebpGlitch {
    riff: RiffContainer,
    pixels: PixelBuffer,
    quality: f32,
}

impl WebpGlitch {
    /// Opens a WebP image from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WebpError> {
        let data = std::fs::read(path)?;
        Self::new(data)
    }

    /// Creates a [`WebpGlitch`] instance from an in-memory WebP byte buffer.
    pub fn new(data: Vec<u8>) -> Result<Self, WebpError> {
        let riff = RiffContainer::parse(&data)?;
        let pixels = decode_pixels(&data)?;
        Ok(Self { riff, pixels, quality: 90.0 })
    }

    /// Returns the image width in pixels.
    pub fn width(&self) -> u32 {
        self.pixels.width
    }

    /// Returns the image height in pixels.
    pub fn height(&self) -> u32 {
        self.pixels.height
    }

    /// Returns `true` if the WebP image has an alpha channel.
    pub fn has_alpha(&self) -> bool {
        self.pixels.has_alpha
    }

    /// Sets the re-encoding quality (`0.0..=100.0`).
    pub fn set_quality(&mut self, quality: f32) {
        self.quality = quality.clamp(0.0, 100.0);
    }

    /// Returns mutable views over all scanlines in the decoded pixel buffer.
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        self.pixels.scan_lines()
    }

    /// Returns a mutable view over scanline `y`, or `None` if `y >= height()`.
    pub fn scan_line_mut(&mut self, y: u32) -> Option<ScanLine<'_>> {
        self.pixels.scan_line_mut(y)
    }

    /// Applies a closure sequentially to every scanline.
    pub fn foreach_scanline<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&mut ScanLine<'_>),
    {
        for mut line in self.pixels.scan_lines() {
            f(&mut line);
        }
        self
    }

    /// Applies a closure to all scanlines in parallel using `rayon`.
    pub fn par_foreach_scanline<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&mut ScanLine<'_>) + Send + Sync,
    {
        use rayon::prelude::*;
        let stride = self.pixels.stride();
        let width = self.pixels.width;
        let has_alpha = self.pixels.has_alpha;
        self.pixels
            .data
            .par_chunks_mut(stride)
            .for_each(|row| {
                let mut line = ScanLine::new(row, width, has_alpha);
                f(&mut line);
            });
        self
    }

    /// Provides mutable access to the underlying RIFF container chunks.
    pub fn with_riff<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut RiffContainer),
    {
        f(&mut self.riff);
        self
    }

    /// Encodes and saves the glitched WebP image to a file.
    pub fn save(self, path: impl AsRef<Path>) -> Result<(), WebpError> {
        let data = self.encode()?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Encodes the glitched WebP image into a byte vector.
    pub fn encode(&self) -> Result<Vec<u8>, WebpError> {
        encode_pixels(&self.pixels, self.quality)
    }
}

// --- 内部: libwebp 経由のデコード / エンコード ---

fn decode_pixels(data: &[u8]) -> Result<PixelBuffer, WebpError> {
    use webp::Decoder;
    let decoder = Decoder::new(data);
    let image = decoder
        .decode()
        .ok_or_else(|| WebpError::DecodeError("libwebp decode failed".into()))?;
    let has_alpha = image.is_alpha();
    let width = image.width();
    let height = image.height();
    let raw = if has_alpha {
        image.to_image().to_rgba8().into_raw()
    } else {
        image.to_image().to_rgb8().into_raw()
    };
    Ok(PixelBuffer::new(width, height, has_alpha, raw))
}

fn encode_pixels(buf: &PixelBuffer, quality: f32) -> Result<Vec<u8>, WebpError> {
    use webp::Encoder;
    let img: image::DynamicImage = if buf.has_alpha {
        let rgba = image::RgbaImage::from_raw(buf.width, buf.height, buf.data.clone())
            .ok_or_else(|| WebpError::EncodeError("failed to build RgbaImage".into()))?;
        image::DynamicImage::ImageRgba8(rgba)
    } else {
        let rgb = image::RgbImage::from_raw(buf.width, buf.height, buf.data.clone())
            .ok_or_else(|| WebpError::EncodeError("failed to build RgbImage".into()))?;
        image::DynamicImage::ImageRgb8(rgb)
    };
    let encoder = Encoder::from_image(&img)
        .map_err(|e| WebpError::EncodeError(e.to_string()))?;
    let webp_data = encoder.encode(quality);
    Ok(webp_data.to_vec())
}

// --- テストヘルパー: 最小 WebP バイト列を生成 ---

#[cfg(test)]
fn make_test_webp(width: u32, height: u32) -> Vec<u8> {
    use webp::Encoder;
    let pixels: Vec<u8> = (0..(width * height * 3))
        .map(|i| (i % 256) as u8)
        .collect();
    let img = image::RgbImage::from_raw(width, height, pixels).unwrap();
    let dyn_img = image::DynamicImage::ImageRgb8(img);
    let enc = Encoder::from_image(&dyn_img).unwrap();
    enc.encode(90.0).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riff::RiffContainer;

    #[test]
    fn test_riff_parse_roundtrip() {
        let data = make_test_webp(4, 4);
        let container = RiffContainer::parse(&data).expect("should parse");
        assert!(!container.chunks.is_empty(), "should have chunks");
        let reencoded = container.encode();
        // 再エンコードしても先頭シグネチャは保たれる
        assert_eq!(&reencoded[0..4], b"RIFF");
        assert_eq!(&reencoded[8..12], b"WEBP");
    }

    #[test]
    fn test_riff_invalid_signature() {
        let result = RiffContainer::parse(b"INVALID DATA");
        assert!(matches!(result, Err(WebpError::InvalidSignature)));
    }

    #[test]
    fn test_webp_glitch_new_and_dimensions() {
        let data = make_test_webp(8, 6);
        let glitch = WebpGlitch::new(data).expect("should load");
        assert_eq!(glitch.width(), 8);
        assert_eq!(glitch.height(), 6);
    }

    #[test]
    fn test_pixel_get_set_roundtrip() {
        let data = make_test_webp(4, 4);
        let mut glitch = WebpGlitch::new(data).expect("should load");
        let lines = glitch.scan_lines();
        // 最初のスキャンラインでピクセル取得が可能
        assert!(lines[0].get_pixel(0).is_some());
    }

    #[test]
    fn test_foreach_scanline_invert() {
        let data = make_test_webp(4, 4);
        let mut glitch = WebpGlitch::new(data).expect("should load");
        // 反転を2回かけると元に戻る
        let original: Vec<u8> = glitch.pixels.data.clone();
        glitch.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        });
        glitch.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        });
        assert_eq!(glitch.pixels.data, original);
    }

    #[test]
    fn test_encode_produces_valid_webp() {
        let data = make_test_webp(4, 4);
        let glitch = WebpGlitch::new(data).expect("should load");
        let encoded = glitch.encode().expect("encode should succeed");
        // 再エンコードした結果も有効な RIFF/WEBP
        assert_eq!(&encoded[0..4], b"RIFF");
        assert_eq!(&encoded[8..12], b"WEBP");
    }

    #[test]
    fn test_riff_remove_chunk() {
        let data = make_test_webp(4, 4);
        let mut container = RiffContainer::parse(&data).unwrap();
        let before = container.chunks.len();
        // 存在しないチャンクの削除はパニックしない
        container.remove_chunk(b"ICCP");
        assert_eq!(container.chunks.len(), before);
    }

    #[test]
    fn test_set_quality() {
        let data = make_test_webp(4, 4);
        let mut glitch = WebpGlitch::new(data).expect("should load");
        glitch.set_quality(10.0);
        let low_q = glitch.encode().expect("encode ok").len();

        let data2 = make_test_webp(4, 4);
        let mut glitch2 = WebpGlitch::new(data2).expect("should load");
        glitch2.set_quality(95.0);
        let high_q = glitch2.encode().expect("encode ok").len();

        // 低品質の方がファイルサイズが小さいはず
        assert!(low_q <= high_q, "low quality should produce smaller or equal file");
    }
}
