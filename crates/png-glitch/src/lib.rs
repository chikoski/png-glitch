//! Low-level library for parsing, manipulating, and re-encoding PNG scanlines to create glitch art.
//!
//! # Design principles
//!
//! png-glitch produces PNGs whose **format is valid but whose data is strange**, and enjoys the
//! changes that a standard PNG decoder produces from that data.
//!
//! - Glitches change only the **scan-line layer**: each row's filter-type byte and its filtered
//!   bytes. The compressed layer (IDAT/zlib and checksums) is regenerated cleanly on every save,
//!   so the output is always a valid PNG.
//! - The look comes from the decoder reconstructing rows according to the PNG specification:
//!   prediction errors propagate right (Sub), down (Up) and diagonally (Average, Paeth).
//! - Setting a row's filter type ([`PngGlitch::set_filter_type`]) only overwrites the tag.
//!   Operations that recompute filtered values ([`PngGlitch::remove_filter`],
//!   [`PngGlitch::apply_filter`], [`PngGlitch::reencode_with`]) bake the glitches so far into
//!   pixels and start a new generation.
//!
//! See [Design principles](https://github.com/chikoski/png-glitch/blob/master/DEVELOPER_GUIDE.md#design-principles)
//! and [ADR 0002](https://github.com/chikoski/png-glitch/blob/master/docs/adr/0002-design-principles.md)
//! for the full text.
#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub use crate::operation::Transpose;
use crate::operation::{Encode, Scan};
use crate::png::Png;
pub use crate::png::{
    ColorType, Encoding, FilterStrategy, FilterType, Limits, LivePipeline, ParseOptions, Pixel,
    PixelSource, PngError, PngWarning, Renderer, ScanLine, ScanLines,
};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Deterministic glitch effects (`Invert`, `Brighten`, `Darken`, `ShiftChannels`) and the
/// [`Effect`](effects::Effect) / [`LineEffect`](effects::LineEffect) traits.
pub mod effects;
mod operation;
mod png;

/// Old names of the [`effects`] module, kept for one release.
#[deprecated(
    since = "0.8.0",
    note = "renamed to `effects`; \"preset\" now means a named recipe"
)]
pub mod presets {
    pub use crate::effects::LineEffect as GlitchPreset;
    pub use crate::effects::{Brighten, Darken, Invert, ShiftChannels};
}

/// PngGlitch is a crate to create a glitched PNG image.
/// Please refer to ["The Art of PNG glitch"](https://ucnv.github.io/pnglitch/) for the description about what glitched PNG is.
///
/// # Filter type: tag or values?
///
/// Two different operations are easy to confuse:
///
/// | Operation | What it does | Recomputes filtered values? |
/// | :-------- | :----------- | :-------------------------- |
/// | [`PngGlitch::set_filter_type`], [`ScanLine::set_filter_type`], [`effects::SetFilterTypes`] | overwrites the per-row filter-type byte (the tag) | no |
/// | [`PngGlitch::encode`], [`PngGlitch::save`] | compresses the scan lines as they are and adds checksums | no |
/// | [`PngGlitch::reencode_with`], [`effects::Refilter`] | removes the current filter, then applies another one | **yes** |
/// | [`PngGlitch::remove_filter`], [`PngGlitch::apply_filter`] | the two halves of the above | **yes** |
///
/// # Available Effects
///
/// The library provides several built-in effects in the [`effects`] module:
/// - [`effects::Invert`]: Inverts all color channels of each pixel.
/// - [`effects::ShiftChannels`]: Shifts color channels (R, G, B) by specific amounts.
/// - [`effects::Brighten`]: Increases image brightness (saturating at the bit-depth maximum).
/// - [`effects::Darken`]: Decreases image brightness (saturating at 0).
///
/// # Examples
///
/// The following snippet shows how you can glitch "./etc/sample00.png" using a mix of effects and low-level manipulation.
///
/// ```
/// # use std::env;
/// # env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string())).expect("");
///
/// use png_glitch::{FilterType, PngGlitch, Pixel};
/// use png_glitch::effects::{Invert, Brighten};
///
/// let out_path = std::env::temp_dir().join("png_glitch_doctest_glitched.png");
/// PngGlitch::open("./etc/sample00.png")
///   .expect("The PNG file should be successfully parsed")
///   .remove_filter()
///   .apply(Invert)
///   .apply(Brighten { strength: 30 })
///   .par_foreach_scanline(|scan_line| {
///     // Custom manipulation using the Pixel API
///     if let Some(Pixel::RGB(r, g, b)) = scan_line.get_pixel(10) {
///         scan_line.set_pixel(10, Pixel::RGB(r, 0, b));
///     }
///   })
///   .save(&out_path)
///   .expect("The glitched file should be saved as a PNG file");
/// # let _ = std::fs::remove_file(out_path);
/// ```
///
pub struct PngGlitch {
    png: Png,
    /// The encoding a model built by [`from_pixels`](PngGlitch::from_pixels) was created with.
    live: Option<Encoding>,
    /// One scan line of scratch space for [`ScanLines`], allocated once at construction.
    scratch: Vec<u8>,
}

impl PngGlitch {
    /// Creates a `PngGlitch` object to glitch the PNG image loaded from the given file path.
    pub fn open(path: impl AsRef<Path>) -> Result<PngGlitch, PngError> {
        Self::open_with_options(path, ParseOptions::default())
    }

    /// Creates a `PngGlitch` object from a file path using custom resource [`Limits`].
    pub fn open_with_limits(path: impl AsRef<Path>, limits: Limits) -> Result<PngGlitch, PngError> {
        Self::open_with_options(path, ParseOptions::default().with_limits(limits))
    }

    /// Creates a `PngGlitch` object from a file path using custom [`ParseOptions`].
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: ParseOptions,
    ) -> Result<PngGlitch, PngError> {
        let mut file = File::open(path)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        PngGlitch::from_bytes_with_options(&buf, options)
    }

    /// Creates a `PngGlitch` object from any byte slice or buffer containing PNG data.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<PngGlitch, PngError> {
        Self::from_bytes_with_options(bytes, ParseOptions::default())
    }

    /// Creates a `PngGlitch` object from a byte slice using custom resource [`Limits`].
    pub fn from_bytes_with_limits(
        bytes: impl AsRef<[u8]>,
        limits: Limits,
    ) -> Result<PngGlitch, PngError> {
        Self::from_bytes_with_options(bytes, ParseOptions::default().with_limits(limits))
    }

    /// Creates a `PngGlitch` object from a byte slice using custom [`ParseOptions`].
    pub fn from_bytes_with_options(
        bytes: impl AsRef<[u8]>,
        options: ParseOptions,
    ) -> Result<PngGlitch, PngError> {
        let png = Png::parse_with_options(bytes.as_ref(), options)?;
        let scratch = vec![0; png.header.scan_line_width()];
        Ok(PngGlitch {
            png,
            live: None,
            scratch,
        })
    }

    /// Creates a `PngGlitch` directly from pixels, running only the PNG encoder's filter step:
    /// nothing is compressed until you ask for PNG bytes ([`to_bytes`](Self::to_bytes),
    /// [`save`](Self::save)). Use this for live sources such as webcam frames, canvases or
    /// game output.
    ///
    /// `enc` holds the choices a PNG encoder makes (color type, filter strategy). They shape the
    /// glitch, so the same frame glitched the same way looks different under different
    /// encodings. See [`Encoding`] for the supported combinations and the color conversions.
    ///
    /// Use [`reload_pixels`](Self::reload_pixels) to refill the model with the next frame
    /// without allocating.
    ///
    /// # Errors
    /// [`PngError::UnsupportedEncoding`], [`PngError::EmptyFilterPattern`],
    /// [`PngError::PixelDataLength`] (the data is not `width * height * 4` bytes),
    /// [`PngError::InvalidDimensions`] or [`PngError::ImageTooLarge`].
    ///
    /// # Examples
    /// ```
    /// use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};
    ///
    /// let frame = vec![128u8; 4 * 4 * 4]; // 4x4 RGBA
    /// let png = PngGlitch::from_pixels(
    ///     PixelSource::rgba8(4, 4, &frame),
    ///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
    /// )?;
    /// assert!(png.to_bytes()?.starts_with(b"\x89PNG"));
    /// # Ok::<(), png_glitch::PngError>(())
    /// ```
    pub fn from_pixels(src: PixelSource<'_>, enc: Encoding) -> Result<PngGlitch, PngError> {
        let png = Png::from_pixels(src, &enc)?;
        let scratch = vec![0; png.header.scan_line_width()];
        Ok(PngGlitch {
            png,
            live: Some(enc),
            scratch,
        })
    }

    /// Refills a model created by [`from_pixels`](Self::from_pixels) with the next frame,
    /// re-applying the same encoding. It makes **no heap allocation**, so it is suitable for
    /// per-frame use. Any glitch applied since the last load is discarded.
    ///
    /// # Errors
    /// [`PngError::NotCreatedFromPixels`] for a model opened from a file or bytes, and
    /// [`PngError::PixelDataLength`] if `rgba` is not `width * height * 4` bytes.
    pub fn reload_pixels(&mut self, rgba: &[u8]) -> Result<(), PngError> {
        let enc = self.live.as_ref().ok_or(PngError::NotCreatedFromPixels)?;
        self.png.reload_pixels(rgba, enc)
    }

    /// Creates a `PngGlitch` object from a byte buffer (`Vec<u8>` or `&[u8]`).
    #[deprecated(
        since = "0.8.0",
        note = "use `PngGlitch::from_bytes`; removed before 1.0"
    )]
    pub fn new(buffer: impl AsRef<[u8]>) -> Result<PngGlitch, PngError> {
        Self::from_bytes(buffer)
    }

    /// Returns any non-fatal PNG integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        self.png.warnings()
    }

    /// Returns a list of scan lines in the loaded PNG image.
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        self.png.scan_lines()
    }

    /// Lends the filtered scan lines as a [`ScanLines`] view: only scan-line-layer operations, no
    /// allocation, and nothing that recomputes filtered values. The view also carries this
    /// model's scratch buffer (one scan line, allocated at construction).
    ///
    /// Unlike [`scan_lines`](Self::scan_lines), it does not build a `Vec` of rows.
    pub fn scan_lines_mut(&mut self) -> ScanLines<'_> {
        ScanLines::new(&mut self.png, &mut self.scratch)
    }

    /// Takes at most `lines` `ScanLine` objects starting from `from`, clamping to the image height.
    pub fn scan_lines_from(&mut self, from: u32, lines: u32) -> Vec<ScanLine<'_>> {
        self.png.scan_lines_from(from as usize, lines as usize)
    }

    /// Allows manipulating each scan line sequentially.
    pub fn foreach_scanline<F>(&mut self, modifier: F) -> &mut Self
    where
        F: FnMut(&mut ScanLine),
    {
        self.png.foreach_scanline(modifier);
        self
    }

    /// Allows manipulating each scan line in parallel.
    pub fn par_foreach_scanline<F>(&mut self, modifier: F) -> &mut Self
    where
        F: Fn(&mut ScanLine) + Sync + Send,
    {
        self.png.par_foreach_scanline(modifier);
        self
    }

    /// Saves the glitched image as a PNG file to the given path.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PngError> {
        self.png.save(path)
    }

    /// Encodes the glitched image as PNG data and writes it to the given `Write` sink.
    pub fn encode(&self, writer: impl Write) -> Result<(), PngError> {
        self.png.encode(writer)
    }

    /// Encodes the glitched image and returns the resulting PNG bytes as a `Vec<u8>`.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PngError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer)?;
        Ok(buffer)
    }

    /// Returns the width of the loaded PNG image.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// Returns the height of the loaded PNG image.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// Returns the color type of the loaded PNG image.
    pub fn color_type(&self) -> ColorType {
        self.png.header.color_type()
    }

    /// Returns the bit depth of the loaded PNG image.
    pub fn bit_depth(&self) -> u8 {
        self.png.header.bit_depth()
    }

    /// Swaps `lines` scan lines starting from `src` with those at `dst`, clamping out-of-range regions.
    pub fn transpose(&mut self, src: u32, dst: u32, lines: u32) -> &mut Self {
        self.png.transpose(src as usize, dst as usize, lines);
        self
    }

    /// Reconstructs raw pixel bytes by removing PNG prediction filters from all scan lines.
    pub fn remove_filter(&mut self) -> &mut Self {
        self.png.remove_filter();
        self
    }

    /// Reconstructs raw pixel bytes by removing PNG prediction filters in the specified region.
    pub fn remove_filter_from(&mut self, from: u32, lines: u32) -> &mut Self {
        self.png.remove_filter_from(from, lines);
        self
    }

    /// Applies the specified PNG prediction filter to all scan lines.
    pub fn apply_filter(&mut self, filter: FilterType) -> &mut Self {
        self.png.apply_filter(filter);
        self
    }

    /// Applies the specified PNG prediction filter to scan lines in the specified region.
    pub fn apply_filter_from(
        &mut self,
        filter_type: FilterType,
        from: u32,
        lines: u32,
    ) -> &mut Self {
        self.png.apply_filter_from(filter_type, from, lines);
        self
    }

    /// Re-encodes all scan lines with the given PNG filter type (`remove_filter` followed by `apply_filter`).
    pub fn reencode_with(&mut self, filter_type: FilterType) -> &mut Self {
        self.png.change_filter_type(filter_type);
        self
    }

    /// Re-encodes all scan lines with the given filter type (alias for [`Self::reencode_with`]).
    ///
    /// Despite the name this recomputes every filtered value; it does not just change the tag.
    /// Use [`Self::set_filter_type`] to overwrite only the tag.
    #[deprecated(
        since = "0.8.0",
        note = "use `reencode_with` (recomputes values) or `set_filter_type` (tag only); removed before 1.0"
    )]
    pub fn change_filter_type(&mut self, filter_type: FilterType) -> &mut Self {
        self.reencode_with(filter_type)
    }

    /// Overwrites only the PNG filter-type tag byte on every scan line without transforming pixel bytes.
    pub fn set_filter_type(&mut self, filter_type: FilterType) -> &mut Self {
        self.foreach_scanline(|line| line.set_filter_type(filter_type))
    }

    /// Applies an [`Effect`](effects::Effect) to the image.
    ///
    /// Per-line effects ([`LineEffect`](effects::LineEffect)) run on all scan lines in parallel.
    pub fn apply<P: effects::Effect>(&mut self, effect: P) -> &mut Self {
        effect.apply_to(self);
        self
    }

    /// Returns the raw decompressed scan-line data.
    pub fn data(&self) -> &[u8] {
        &self.png.data
    }
}

impl TryFrom<&[u8]> for PngGlitch {
    type Error = PngError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn write_test_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(chunk_type);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
    }

    fn build_synthetic_png(width: u32, height: u32, color_type: u8, bit_depth: u8) -> Vec<u8> {
        let channels: usize = match color_type {
            0 => 1,
            2 => 3,
            3 => 1,
            4 => 2,
            6 => 4,
            _ => unreachable!(),
        };
        let bits_per_pixel = channels * (bit_depth as usize);
        let row_bytes = (width as usize * bits_per_pixel).div_ceil(8);
        let mut raw = Vec::with_capacity(height as usize * (1 + row_bytes));
        for y in 0..(height as usize) {
            raw.push(0); // FilterType::None
            for x in 0..row_bytes {
                // Clear unused padding bits in the last byte of sub-8-bit rows so independent decoders compare cleanly
                let mut byte = ((y * 37 + x * 19 + 11) & 0xFF) as u8;
                if bit_depth < 8 && x + 1 == row_bytes {
                    let valid_bits = (width as usize * bits_per_pixel) % 8;
                    if valid_bits != 0 {
                        let mask = !((1u8 << (8 - valid_bits)) - 1);
                        byte &= mask;
                    }
                }
                raw.push(byte);
            }
        }

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(bit_depth);
        ihdr.push(color_type);
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0);
        write_test_chunk(&mut png, b"IHDR", &ihdr);

        if color_type == 3 {
            let entries = 1usize << bit_depth;
            let mut plte = Vec::with_capacity(entries * 3);
            for i in 0..entries {
                plte.push(((i * 53) & 0xFF) as u8);
                plte.push(((i * 97 + 40) & 0xFF) as u8);
                plte.push(((i * 151 + 80) & 0xFF) as u8);
            }
            write_test_chunk(&mut png, b"PLTE", &plte);
        }

        let mut compressor = fdeflate::Compressor::new(Vec::new()).unwrap();
        compressor.write_data(&raw).unwrap();
        let idat = compressor.finish().unwrap();
        write_test_chunk(&mut png, b"IDAT", &idat);
        write_test_chunk(&mut png, b"IEND", &[]);
        png
    }

    #[test]
    fn test_change_filter_type() -> Result<(), PngError> {
        let bytes = include_bytes!("../etc/sample00.png");

        // 1. Baseline: Raw (removed filter)
        let mut png_raw = PngGlitch::from_bytes(bytes)?;
        png_raw.remove_filter();

        // 2. Test Subject: cycle through all 5 filter types and back to None
        let mut png_glitch = PngGlitch::try_from(bytes as &[u8])?;
        for filter in [
            FilterType::Sub,
            FilterType::Up,
            FilterType::Average,
            FilterType::Paeth,
            FilterType::None,
        ] {
            png_glitch.reencode_with(filter);
            for scan_line in png_glitch.scan_lines() {
                assert_eq!(filter, scan_line.filter_type());
            }
        }

        // Compare with Baseline (png_raw)
        assert_eq!(png_glitch.data(), png_raw.data());
        Ok(())
    }

    #[test]
    fn test_roundtrip_filter_reencoding_across_color_types_bit_depths_and_odd_widths() {
        let formats: &[(u8, &[u8])] = &[
            (0, &[1, 2, 4, 8, 16]),
            (2, &[8, 16]),
            (3, &[1, 2, 4, 8]),
            (4, &[8, 16]),
            (6, &[8, 16]),
        ];
        let filter_types = [
            FilterType::None,
            FilterType::Sub,
            FilterType::Up,
            FilterType::Average,
            FilterType::Paeth,
        ];

        for &(color_type, depths) in formats {
            for &bit_depth in depths {
                // Test non-byte-aligned and odd widths (1..=9 and 17)
                for width in [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 17] {
                    let height = 5u32;
                    let src_bytes = build_synthetic_png(width, height, color_type, bit_depth);
                    let expected_img =
                        image::load_from_memory_with_format(&src_bytes, image::ImageFormat::Png)
                            .expect("synthetic PNG should decode with `image` crate")
                            .into_rgba8();

                    let mut png = PngGlitch::from_bytes(&src_bytes).unwrap();
                    let unfiltered_raw = png.data().to_vec();

                    // 1. Round-trip every filter type and verify decoded pixels match the original
                    for &ft in &filter_types {
                        png.reencode_with(ft);
                        let encoded = png.to_bytes().unwrap();
                        let decoded_img = image::load_from_memory_with_format(
                            &encoded,
                            image::ImageFormat::Png,
                        )
                        .unwrap_or_else(|e| {
                            panic!(
                                "re-encoded PNG (color_type={color_type}, bit_depth={bit_depth}, width={width}, filter={ft:?}) failed to decode: {e}"
                            )
                        })
                        .into_rgba8();
                        assert_eq!(
                            decoded_img, expected_img,
                            "pixel mismatch after re-encoding with {ft:?} (color_type={color_type}, bit_depth={bit_depth}, width={width})"
                        );
                    }

                    // 2. Removing filter brings back the exact raw scanline bytes
                    png.remove_filter();
                    assert_eq!(png.data(), unfiltered_raw.as_slice());

                    // 3. Double Invert restores original pixels
                    png.apply(effects::Invert);
                    png.apply(effects::Invert);
                    assert_eq!(png.data(), unfiltered_raw.as_slice());
                }
            }
        }
    }

    #[test]
    fn test_range_apis_clamp_and_never_panic_on_out_of_bounds_or_overflow() {
        let bytes = build_synthetic_png(8, 6, 2, 8);
        let mut png = PngGlitch::from_bytes(&bytes).unwrap();
        let h = png.height();

        // Out-of-bounds and overflowing ranges must clamp cleanly without panicking
        png.transpose(0, h - 1, 5);
        png.transpose(h + 10, 0, 1);
        png.transpose(0, 0, u32::MAX);

        assert!(png.scan_lines_from(h, 10).is_empty());
        assert_eq!(png.scan_lines_from(h - 2, 10).len(), 2);
        assert_eq!(png.scan_lines_from(0, u32::MAX).len(), h as usize);

        png.remove_filter_from(h - 1, 5);
        png.remove_filter_from(h + 5, u32::MAX);
        png.apply_filter_from(FilterType::Up, 0, u32::MAX);
        png.apply_filter_from(FilterType::Paeth, u32::MAX, 2);
    }

    #[test]
    fn test_presets_and_to_bytes() -> Result<(), PngError> {
        let bytes = include_bytes!("../etc/sample00.png");
        let mut png = PngGlitch::from_bytes(bytes)?;
        let original_data = png.data().to_vec();

        png.apply(effects::Invert);
        assert_ne!(png.data(), original_data);

        png.apply(effects::Invert);
        assert_eq!(png.data(), original_data);

        let encoded = png.to_bytes()?;
        assert!(!encoded.is_empty());

        Ok(())
    }

    #[test]
    fn test_typed_png_error_matching() {
        let err = match PngGlitch::from_bytes(b"not a png") {
            Err(e) => e,
            Ok(_) => panic!("expected PngError::InvalidSignature"),
        };
        assert!(matches!(err, PngError::InvalidSignature));
    }

    #[test]
    fn test_chunk_order_preserved_before_and_after_idat() {
        let raw = vec![0u8, 10, 20, 30]; // 1x1 RGB
        let mut compressor = fdeflate::Compressor::new(Vec::new()).unwrap();
        compressor.write_data(&raw).unwrap();
        let idat = compressor.finish().unwrap();

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        write_test_chunk(&mut png, b"IHDR", &ihdr);
        write_test_chunk(&mut png, b"gAMA", &[0, 0, 0xB1, 0x8F]);
        write_test_chunk(&mut png, b"tEXt", b"Comment\0before-idat");
        write_test_chunk(&mut png, b"IDAT", &idat);
        write_test_chunk(&mut png, b"tEXt", b"Comment\0after-idat");
        write_test_chunk(&mut png, b"IEND", &[]);

        let pg = PngGlitch::from_bytes(&png).unwrap();
        let encoded = pg.to_bytes().unwrap();

        // Collect chunk order from the re-encoded PNG
        let mut pos = 8usize;
        let mut seen = Vec::new();
        while pos + 12 <= encoded.len() {
            let len = u32::from_be_bytes(encoded[pos..pos + 4].try_into().unwrap()) as usize;
            let tag = std::str::from_utf8(&encoded[pos + 4..pos + 8])
                .unwrap()
                .to_string();
            let payload = &encoded[pos + 8..pos + 8 + len];
            seen.push((tag, payload.to_vec()));
            pos += 12 + len;
        }

        let tags: Vec<&str> = seen.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(tags, vec!["IHDR", "gAMA", "tEXt", "IDAT", "tEXt", "IEND"]);
        assert_eq!(seen[2].1, b"Comment\0before-idat");
        assert_eq!(seen[4].1, b"Comment\0after-idat");
    }

    #[test]
    fn test_integrity_checks_lenient_warnings_and_strict_errors() {
        // 1. CRC mismatch on IHDR
        let mut bad_crc = build_synthetic_png(2, 2, 2, 8);
        bad_crc[29] ^= 0xFF; // flip last byte of IHDR CRC
        let lenient = PngGlitch::from_bytes(&bad_crc).unwrap();
        assert!(lenient.warnings().iter().any(
            |w| matches!(w, PngWarning::CrcMismatch { chunk_type, .. } if chunk_type == "IHDR")
        ));
        assert!(matches!(
            PngGlitch::from_bytes_with_options(&bad_crc, ParseOptions::strict()),
            Err(PngError::CrcMismatch { .. })
        ));

        // 2. Invalid filter byte (> 4) and excess decompressed IDAT bytes
        let mut raw = vec![99u8, 1, 2, 3, 0u8, 4, 5, 6]; // 1x2 RGB with filter=99 on row 0
        raw.extend_from_slice(&[0xAA, 0xBB]); // 2 excess bytes
        let mut comp = fdeflate::Compressor::new(Vec::new()).unwrap();
        comp.write_data(&raw).unwrap();
        let idat = comp.finish().unwrap();

        let mut png_excess = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        write_test_chunk(&mut png_excess, b"IHDR", &ihdr);
        write_test_chunk(&mut png_excess, b"IDAT", &idat);
        write_test_chunk(&mut png_excess, b"IEND", &[]);
        png_excess.extend_from_slice(b"trailing");

        let mut lenient_pg = PngGlitch::from_bytes(&png_excess).unwrap();
        assert!(
            lenient_pg
                .warnings()
                .iter()
                .any(|w| matches!(w, PngWarning::ExcessDecompressedData { .. }))
        );
        assert!(
            lenient_pg
                .warnings()
                .iter()
                .any(|w| matches!(w, PngWarning::InvalidFilterByte { row: 0, byte: 99 }))
        );
        assert!(
            lenient_pg
                .warnings()
                .iter()
                .any(|w| matches!(w, PngWarning::TrailingDataAfterIEND { bytes: 8 }))
        );
        // Filter byte 99 was normalized to FilterType::None (0)
        assert_eq!(lenient_pg.scan_lines()[0].filter_type(), FilterType::None);

        // Strict mode rejects trailing data / excess / invalid filter byte
        assert!(PngGlitch::from_bytes_with_options(&png_excess, ParseOptions::strict()).is_err());

        // 3. Non-consecutive IDAT chunks
        let split_a = &idat[..idat.len() / 2];
        let split_b = &idat[idat.len() / 2..];
        let mut non_consec = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        write_test_chunk(&mut non_consec, b"IHDR", &ihdr);
        write_test_chunk(&mut non_consec, b"IDAT", split_a);
        write_test_chunk(&mut non_consec, b"tEXt", b"interleaved");
        write_test_chunk(&mut non_consec, b"IDAT", split_b);
        write_test_chunk(&mut non_consec, b"IEND", &[]);
        assert!(matches!(
            PngGlitch::from_bytes_with_options(&non_consec, ParseOptions::strict()),
            Err(PngError::NonConsecutiveIDAT)
        ));
    }

    /// Encodes `width` x `height` RGBA pixels with the `png` crate, with a fixed filter or its
    /// adaptive per-row choice.
    fn encode_rgba(width: u32, height: u32, rgba: &[u8], filter: ::png::Filter) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = ::png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(::png::ColorType::Rgba);
            encoder.set_depth(::png::BitDepth::Eight);
            encoder.set_compression(::png::Compression::Fast);
            // `set_compression` resets the filter, so it comes first.
            encoder.set_filter(filter);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(rgba).unwrap();
        }
        bytes
    }

    /// RGBA images of 1..=40 x 1..=12 pixels: a gradient plus noise of a random amplitude, so
    /// the adaptive encoder mixes filter types from row to row.
    fn rgba_images() -> impl proptest::strategy::Strategy<Value = (u32, u32, Vec<u8>)> {
        use proptest::prelude::*;
        (1u32..=40, 1u32..=12, 0u8..=255, any::<u64>()).prop_map(|(w, h, noise, seed)| {
            let mut state = seed | 1;
            let mut next = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            };
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    for ch in 0..4u32 {
                        let base = (x * 7 + y * 5 + ch * 60) as u8;
                        let n = if noise == 0 { 0 } else { next() % noise };
                        rgba.push(base.wrapping_add(n));
                    }
                }
            }
            (w, h, rgba)
        })
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(128))]

        /// `remove_filter` gives the pixels the `png` crate decodes, for whole images whose rows
        /// all use Paeth or Average (the multi-row wavefront kernels on wasm with `simd128`) and
        /// for adaptive, mixed-filter images.
        #[test]
        fn remove_filter_matches_png_decoder((w, h, rgba) in rgba_images()) {
            for filter in [::png::Filter::Paeth, ::png::Filter::Avg, ::png::Filter::Adaptive] {
                let bytes = encode_rgba(w, h, &rgba, filter);
                let mut png = PngGlitch::from_bytes(&bytes).unwrap();
                png.remove_filter();
                let expected: Vec<u8> = rgba
                    .chunks(w as usize * 4)
                    .flat_map(|row| std::iter::once(0).chain(row.iter().copied()))
                    .collect();
                proptest::prop_assert_eq!(png.data(), expected.as_slice(), "{:?}", filter);
            }
        }

        /// `remove_filter_from` over a range equals removing the filters one row at a time,
        /// including when the row above the range is still filtered.
        #[test]
        fn remove_filter_from_matches_row_by_row(
            (w, h, rgba) in rgba_images(),
            from in 0u32..14,
            lines in 0u32..14,
        ) {
            for filter in [::png::Filter::Paeth, ::png::Filter::Avg, ::png::Filter::Adaptive] {
                let bytes = encode_rgba(w, h, &rgba, filter);
                let mut together = PngGlitch::from_bytes(&bytes).unwrap();
                together.remove_filter_from(from, lines);
                let mut one_by_one = PngGlitch::from_bytes(&bytes).unwrap();
                for row in from..from.saturating_add(lines) {
                    one_by_one.remove_filter_from(row, 1);
                }
                proptest::prop_assert_eq!(together.data(), one_by_one.data(), "{:?}", filter);
            }
        }
    }
}
