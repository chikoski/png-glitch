//! Building a PNG model directly from pixels (`PngGlitch::from_pixels`), for live sources such
//! as webcam frames, canvases and game output.
//!
//! No compression happens here: the model holds the filtered scan lines, exactly like a model
//! parsed from a file, and compression only runs if the caller asks for PNG bytes.

use crate::png::parser::{Chunk, ChunkType, Header, Terminator};
use crate::png::parser::{DEFAULT_MAX_DECOMPRESSED_BYTES, DEFAULT_MAX_PIXELS};
use crate::png::scan_line::filter::kernels;
use crate::png::{ColorType, FilterType, Png, PngError};

/// A borrowed frame of 8-bit RGBA pixels (`R, G, B, A` per pixel, rows top to bottom, no padding).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelSource<'a> {
    width: u32,
    height: u32,
    data: &'a [u8],
}

impl<'a> PixelSource<'a> {
    /// Describes `data` as a `width` × `height` RGBA8 image. The length is checked by
    /// [`PngGlitch::from_pixels`](crate::PngGlitch::from_pixels), which must be
    /// `width * height * 4`.
    pub fn rgba8(width: u32, height: u32, data: &'a [u8]) -> Self {
        Self {
            width,
            height,
            data,
        }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The RGBA bytes.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

/// How the pixels are laid out in the PNG: the choices a PNG encoder makes, which shape the
/// glitch.
///
/// Supported from an RGBA8 source (all with `bit_depth: 8`):
///
/// | `color_type` | Conversion |
/// |---|---|
/// | [`TrueColorAlpha`](ColorType::TrueColorAlpha) | as is |
/// | [`TrueColor`](ColorType::TrueColor) | alpha dropped, no compositing |
/// | [`GrayScaleAlpha`](ColorType::GrayScaleAlpha) | `gray = (77*R + 150*G + 29*B + 128) >> 8`, alpha kept |
/// | [`GrayScale`](ColorType::GrayScale) | the same `gray`, alpha dropped |
///
/// Anything else (palettes, bit depths other than 8) is [`PngError::UnsupportedEncoding`].
#[derive(Clone, Debug, PartialEq)]
pub struct Encoding {
    /// PNG color type.
    pub color_type: ColorType,
    /// Bit depth; only `8` is supported.
    pub bit_depth: u8,
    /// Which filter type each scan line gets.
    pub filter: FilterStrategy,
}

impl Encoding {
    /// 8-bit RGBA with the given filter strategy.
    pub fn rgba8(filter: FilterStrategy) -> Self {
        Self {
            color_type: ColorType::TrueColorAlpha,
            bit_depth: 8,
            filter,
        }
    }

    /// Bytes per pixel of the encoded image.
    fn bytes_per_pixel(&self) -> Result<usize, PngError> {
        let channels = match (self.color_type, self.bit_depth) {
            (ColorType::GrayScale, 8) => 1,
            (ColorType::GrayScaleAlpha, 8) => 2,
            (ColorType::TrueColor, 8) => 3,
            (ColorType::TrueColorAlpha, 8) => 4,
            (color_type, bit_depth) => {
                return Err(PngError::UnsupportedEncoding {
                    color_type: color_type.code(),
                    bit_depth,
                });
            }
        };
        if matches!(&self.filter, FilterStrategy::Pattern(p) if p.is_empty()) {
            return Err(PngError::EmptyFilterPattern);
        }
        Ok(channels)
    }
}

/// Which filter type each scan line gets.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FilterStrategy {
    /// Every row uses this filter type.
    Fixed(FilterType),
    /// Row `y` uses `pattern[y % pattern.len()]`. An empty pattern is an error.
    Pattern(Vec<FilterType>),
    /// libpng's heuristic: for every row, the filter type with the smallest sum of
    /// `|filtered byte as i8|`. Ties go to the lowest type number (None < Sub < Up < Average <
    /// Paeth). This tries all five filters on every row, with vectorised sums and an early exit
    /// that cannot change the result: about 7 to 9 ms for a 1080p RGBA frame natively on one
    /// thread (a fixed filter takes about 4 ms), so it fits a 30 fps budget natively; `Fixed`
    /// or `Pattern` are still the cheapest.
    Adaptive,
}

const CANDIDATES: [FilterType; 5] = [
    FilterType::None,
    FilterType::Sub,
    FilterType::Up,
    FilterType::Average,
    FilterType::Paeth,
];

/// `(77*R + 150*G + 29*B + 128) >> 8`, which is exact for 8-bit input (the weights sum to 256).
fn gray(r: u8, g: u8, b: u8) -> u8 {
    ((77 * u32::from(r) + 150 * u32::from(g) + 29 * u32::from(b) + 128) >> 8) as u8
}

/// Converts one row of RGBA8 into `color_type`. `out` must hold the converted row.
fn convert_row(color_type: ColorType, rgba: &[u8], out: &mut [u8]) {
    let pixels = rgba.as_chunks::<4>().0;
    match color_type {
        ColorType::TrueColorAlpha => {
            for (o, i) in out.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
                *o = *i;
            }
        }
        ColorType::TrueColor => {
            for (o, [r, g, b, _]) in out.as_chunks_mut::<3>().0.iter_mut().zip(pixels) {
                *o = [*r, *g, *b];
            }
        }
        ColorType::GrayScaleAlpha => {
            for (o, [r, g, b, a]) in out.as_chunks_mut::<2>().0.iter_mut().zip(pixels) {
                *o = [gray(*r, *g, *b), *a];
            }
        }
        // `IndexColor` is rejected before this point.
        ColorType::GrayScale | ColorType::IndexColor => {
            for (o, [r, g, b, _]) in out.iter_mut().zip(pixels) {
                *o = gray(*r, *g, *b);
            }
        }
    }
}

/// Fills `data` (all scan lines, each with its filter-type byte) from `rgba`, then filters every
/// row according to `filter`. Allocation-free.
///
/// Rows are filtered bottom-up: row `y` needs row `y - 1` still unfiltered.
fn fill(
    data: &mut [u8],
    stride: usize,
    bpp: usize,
    color_type: ColorType,
    filter: &FilterStrategy,
    rgba: &[u8],
    width: usize,
) {
    for (row, src) in data
        .chunks_exact_mut(stride)
        .zip(rgba.chunks_exact(width * 4))
    {
        if let Some((tag, pixels)) = row.split_first_mut() {
            *tag = 0;
            convert_row(color_type, src, pixels);
        }
    }

    filter_rows(data, stride, bpp, filter);
}

/// Filters every row of `data` (unfiltered scan lines, each with a filter-type byte slot) in
/// place according to `filter` and writes the filter-type bytes. Allocation-free.
///
/// Rows are filtered bottom-up: row `y` needs row `y - 1` still unfiltered. An empty `Pattern`
/// must be rejected by the caller; here it falls back to `None` rows.
pub(crate) fn filter_rows(data: &mut [u8], stride: usize, bpp: usize, filter: &FilterStrategy) {
    if stride == 0 {
        return;
    }
    let height = data.len() / stride;
    for y in (0..height).rev() {
        let (above, rest) = data.split_at_mut(y * stride);
        let Some((tag, cur)) = rest.get_mut(..stride).and_then(|row| row.split_first_mut()) else {
            continue;
        };
        let prev = above.rchunks(stride).next().and_then(|row| row.get(1..));
        let filter_type = match filter {
            FilterStrategy::Fixed(t) => *t,
            FilterStrategy::Pattern(pattern) => pattern
                .get(y % pattern.len().max(1))
                .copied()
                .unwrap_or(FilterType::None),
            // The fast path covers every row but the first; the first row (no row above) is
            // compared with the plain cost function, which gives the same choice.
            FilterStrategy::Adaptive => {
                kernels::adaptive_filter(bpp, cur, prev).unwrap_or_else(|| {
                    let mut best = (u64::MAX, FilterType::None);
                    for candidate in CANDIDATES {
                        let cost = kernels::filter_cost(candidate, bpp, cur, prev);
                        if cost < best.0 {
                            best = (cost, candidate);
                        }
                    }
                    best.1
                })
            }
        };
        *tag = filter_type.into();
        kernels::filter(filter_type, bpp, cur, prev);
    }
}

impl Png {
    /// Builds a model from RGBA8 pixels. See [`crate::PngGlitch::from_pixels`].
    pub(crate) fn from_pixels(src: PixelSource<'_>, enc: &Encoding) -> Result<Png, PngError> {
        let (width, height) = (src.width, src.height);
        let bpp = enc.bytes_per_pixel()?;
        let header = Header::from_parts(width, height, enc.bit_depth, enc.color_type)?;

        let too_large = || PngError::ImageTooLarge { width, height };
        if u64::from(width) * u64::from(height) > DEFAULT_MAX_PIXELS {
            return Err(too_large());
        }
        let stride = header.scan_line_width();
        let len = stride
            .checked_mul(height as usize)
            .filter(|&len| len <= DEFAULT_MAX_DECOMPRESSED_BYTES)
            .ok_or_else(too_large)?;
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|p| p.checked_mul(4))
            .ok_or_else(too_large)?;
        if src.data.len() != expected {
            return Err(PngError::PixelDataLength {
                expected,
                actual: src.data.len(),
            });
        }

        let mut data = Vec::new();
        data.try_reserve_exact(len).map_err(|_| too_large())?;
        data.resize(len, 0);
        fill(
            &mut data,
            stride,
            bpp,
            enc.color_type,
            &enc.filter,
            src.data,
            width as usize,
        );

        Ok(Png {
            header,
            terminator: Terminator {
                inner: Chunk::with_recomputed_crc(ChunkType::End, Vec::new()),
            },
            pre_idat_chunks: Vec::new(),
            post_idat_chunks: Vec::new(),
            data,
            warnings: Vec::new(),
        })
    }

    /// `D` then `E` in place: see [`crate::PngGlitch::regenerate`].
    pub(crate) fn regenerate(&mut self, strategy: &FilterStrategy) -> Result<(), PngError> {
        if matches!(strategy, FilterStrategy::Pattern(p) if p.is_empty()) {
            return Err(PngError::EmptyFilterPattern);
        }
        self.remove_filter();
        let bpp = crate::png::scan_line::bytes_per_pixel(
            self.header.color_type(),
            self.header.bit_depth(),
        );
        let stride = self.header.scan_line_width();
        filter_rows(&mut self.data, stride, bpp, strategy);
        Ok(())
    }

    /// Copies the scan-line data (filter-type bytes included) of `other` into `self` without
    /// allocating. Both must have the same size, colour type and bit depth.
    pub(crate) fn copy_data_from(&mut self, other: &Png) -> Result<(), PngError> {
        let same = (
            self.header.width(),
            self.header.height(),
            self.header.color_type(),
            self.header.bit_depth(),
        ) == (
            other.header.width(),
            other.header.height(),
            other.header.color_type(),
            other.header.bit_depth(),
        );
        if !same || self.data.len() != other.data.len() {
            return Err(PngError::HeaderMismatch);
        }
        self.data.copy_from_slice(&other.data);
        Ok(())
    }

    /// Refills the scan lines from `rgba` without allocating. `enc` must be the encoding the
    /// model was created with.
    pub(crate) fn reload_pixels(&mut self, rgba: &[u8], enc: &Encoding) -> Result<(), PngError> {
        let width = self.header.width() as usize;
        let expected = width
            .saturating_mul(self.header.height() as usize)
            .saturating_mul(4);
        if rgba.len() != expected {
            return Err(PngError::PixelDataLength {
                expected,
                actual: rgba.len(),
            });
        }
        let bpp = enc.bytes_per_pixel()?;
        let stride = self.header.scan_line_width();
        fill(
            &mut self.data,
            stride,
            bpp,
            enc.color_type,
            &enc.filter,
            rgba,
            width,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PngGlitch;
    use proptest::prelude::*;

    const COLOR_TYPES: [ColorType; 4] = [
        ColorType::TrueColorAlpha,
        ColorType::TrueColor,
        ColorType::GrayScaleAlpha,
        ColorType::GrayScale,
    ];

    fn strategies() -> Vec<FilterStrategy> {
        let mut all: Vec<_> = CANDIDATES
            .iter()
            .map(|&t| FilterStrategy::Fixed(t))
            .collect();
        all.push(FilterStrategy::Pattern(vec![
            FilterType::Sub,
            FilterType::Up,
            FilterType::Paeth,
        ]));
        all.push(FilterStrategy::Adaptive);
        all
    }

    /// The documented conversion, written independently of `convert_row`.
    fn expected_pixels(color_type: ColorType, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for px in rgba.as_chunks::<4>().0 {
            let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3]);
            let y = ((77 * r + 150 * g + 29 * b + 128) >> 8) as u8;
            match color_type {
                ColorType::TrueColorAlpha => out.extend_from_slice(px),
                ColorType::TrueColor => out.extend_from_slice(&px[..3]),
                ColorType::GrayScaleAlpha => out.extend_from_slice(&[y, a]),
                _ => out.push(y),
            }
        }
        out
    }

    /// Decodes PNG bytes with the independent `png` crate.
    fn decode(bytes: &[u8]) -> (png::ColorType, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        buf.truncate(info.buffer_size());
        (info.color_type, buf)
    }

    fn png_color(color_type: ColorType) -> png::ColorType {
        match color_type {
            ColorType::TrueColorAlpha => png::ColorType::Rgba,
            ColorType::TrueColor => png::ColorType::Rgb,
            ColorType::GrayScaleAlpha => png::ColorType::GrayscaleAlpha,
            _ => png::ColorType::Grayscale,
        }
    }

    fn frame(width: u32, height: u32, seed: u8) -> Vec<u8> {
        (0..width * height * 4)
            .map(|i| {
                (i as u8)
                    .wrapping_mul(31)
                    .wrapping_add(seed)
                    .wrapping_add((i >> 5) as u8)
            })
            .collect()
    }

    fn row_filter_types(png: &PngGlitch) -> Vec<u8> {
        let stride = png.png.header.scan_line_width();
        png.data().chunks_exact(stride).map(|r| r[0]).collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn round_trips_through_an_independent_decoder(
            width in 1u32..=33,
            height in 1u32..=5,
            seed in any::<u64>(),
        ) {
            let pixels: Vec<u8> = {
                // Cheap deterministic noise from the seed, including alpha != 255.
                let mut x = seed | 1;
                (0..width * height * 4).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; (x >> 24) as u8 }).collect()
            };
            for color_type in COLOR_TYPES {
                for filter in strategies() {
                    let enc = Encoding { color_type, bit_depth: 8, filter };
                    let png = PngGlitch::from_pixels(PixelSource::rgba8(width, height, &pixels), enc.clone()).unwrap();
                    let (decoded_type, decoded) = decode(&png.to_bytes().unwrap());
                    prop_assert_eq!(decoded_type, png_color(color_type));
                    prop_assert_eq!(decoded, expected_pixels(color_type, &pixels), "{:?}", enc);
                }
            }
        }
    }

    #[test]
    fn row_filter_types_follow_the_strategy() {
        let pixels = frame(9, 7, 3);
        let src = PixelSource::rgba8(9, 7, &pixels);

        let fixed =
            PngGlitch::from_pixels(src, Encoding::rgba8(FilterStrategy::Fixed(FilterType::Up)))
                .unwrap();
        assert_eq!(row_filter_types(&fixed), vec![2; 7]);

        let pattern = Encoding::rgba8(FilterStrategy::Pattern(vec![
            FilterType::Sub,
            FilterType::Up,
            FilterType::Paeth,
        ]));
        let png = PngGlitch::from_pixels(src, pattern).unwrap();
        assert_eq!(row_filter_types(&png), vec![1, 2, 4, 1, 2, 4, 1]);
    }

    #[test]
    fn adaptive_is_never_worse_than_another_type_and_ties_go_to_the_lowest() {
        for color_type in COLOR_TYPES {
            let pixels = frame(17, 6, 9);
            let enc = Encoding {
                color_type,
                bit_depth: 8,
                filter: FilterStrategy::Adaptive,
            };
            let bpp = enc.bytes_per_pixel().unwrap();
            let png = PngGlitch::from_pixels(PixelSource::rgba8(17, 6, &pixels), enc).unwrap();
            let stride = png.png.header.scan_line_width();
            let converted = expected_pixels(color_type, &pixels);
            let row_len = stride - 1;
            for (y, chosen) in row_filter_types(&png).into_iter().enumerate() {
                let cur = &converted[y * row_len..(y + 1) * row_len];
                let prev = (y > 0).then(|| &converted[(y - 1) * row_len..y * row_len]);
                // The cost by actually filtering a copy, independent of `filter_cost`.
                let cost = |t: FilterType| -> u64 {
                    let mut row = cur.to_vec();
                    kernels::filter(t, bpp, &mut row, prev);
                    row.iter()
                        .map(|&b| u64::from((b as i8).unsigned_abs()))
                        .sum()
                };
                let costs: Vec<u64> = CANDIDATES.iter().map(|&t| cost(t)).collect();
                let min = *costs.iter().min().unwrap();
                assert_eq!(costs[chosen as usize], min, "row {y} {color_type:?}");
                assert_eq!(
                    costs.iter().position(|&c| c == min),
                    Some(chosen as usize),
                    "tie rule, row {y}"
                );
            }
        }
    }

    #[test]
    fn reload_equals_a_fresh_model() {
        let (w, h) = (13, 5);
        let (first, second) = (frame(w, h, 1), frame(w, h, 200));
        for filter in strategies() {
            let enc = Encoding {
                color_type: ColorType::TrueColor,
                bit_depth: 8,
                filter,
            };
            let mut png =
                PngGlitch::from_pixels(PixelSource::rgba8(w, h, &first), enc.clone()).unwrap();
            png.reload_pixels(&second).unwrap();
            let fresh = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &second), enc).unwrap();
            assert_eq!(png.data(), fresh.data());
        }
    }

    #[test]
    fn reload_discards_earlier_glitches() {
        let pixels = frame(8, 8, 0);
        let mut png = PngGlitch::from_pixels(
            PixelSource::rgba8(8, 8, &pixels),
            Encoding::rgba8(FilterStrategy::Adaptive),
        )
        .unwrap();
        let clean = png.data().to_vec();
        png.apply(crate::effects::Invert);
        assert_ne!(png.data(), clean.as_slice());
        png.reload_pixels(&pixels).unwrap();
        assert_eq!(png.data(), clean.as_slice());
    }

    #[test]
    fn errors() {
        let pixels = frame(4, 4, 0);
        let src = PixelSource::rgba8(4, 4, &pixels);
        let rgba = |f| Encoding::rgba8(f);
        assert!(matches!(
            PngGlitch::from_pixels(
                src,
                Encoding {
                    color_type: ColorType::IndexColor,
                    bit_depth: 8,
                    filter: FilterStrategy::Adaptive
                }
            ),
            Err(PngError::UnsupportedEncoding {
                color_type: 3,
                bit_depth: 8
            })
        ));
        assert!(matches!(
            PngGlitch::from_pixels(
                src,
                Encoding {
                    color_type: ColorType::TrueColor,
                    bit_depth: 16,
                    filter: FilterStrategy::Adaptive
                }
            ),
            Err(PngError::UnsupportedEncoding { .. })
        ));
        assert!(matches!(
            PngGlitch::from_pixels(src, rgba(FilterStrategy::Pattern(vec![]))),
            Err(PngError::EmptyFilterPattern)
        ));
        assert!(matches!(
            PngGlitch::from_pixels(
                PixelSource::rgba8(4, 4, &pixels[1..]),
                rgba(FilterStrategy::Adaptive)
            ),
            Err(PngError::PixelDataLength {
                expected: 64,
                actual: 63
            })
        ));
        assert!(matches!(
            PngGlitch::from_pixels(
                PixelSource::rgba8(0, 4, &[]),
                rgba(FilterStrategy::Adaptive)
            ),
            Err(PngError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            PngGlitch::from_pixels(
                PixelSource::rgba8(100_000, 100_000, &[]),
                rgba(FilterStrategy::Adaptive)
            ),
            Err(PngError::ImageTooLarge { .. })
        ));
        let mut ok = PngGlitch::from_pixels(src, rgba(FilterStrategy::Adaptive)).unwrap();
        assert!(matches!(
            ok.reload_pixels(&pixels[1..]),
            Err(PngError::PixelDataLength { .. })
        ));
        let mut from_file =
            PngGlitch::from_bytes(include_bytes!("../../etc/sample00.png")).unwrap();
        assert!(matches!(
            from_file.reload_pixels(&pixels),
            Err(PngError::NotCreatedFromPixels)
        ));
    }

    #[test]
    fn grayscale_conversion_is_exact_at_the_extremes() {
        assert_eq!((gray(0, 0, 0), gray(255, 255, 255)), (0, 255));
        assert_eq!(gray(255, 0, 0), 77);
    }

    /// One full-size frame, only in release builds (`cargo test --release`).
    #[test]
    #[cfg_attr(debug_assertions, ignore)]
    fn full_hd_round_trip() {
        let (w, h) = (1920, 1080);
        let pixels = frame(w, h, 5);
        let enc = Encoding::rgba8(FilterStrategy::Adaptive);
        let png = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &pixels), enc).unwrap();
        let (_, decoded) = decode(&png.to_bytes().unwrap());
        assert_eq!(decoded, pixels);
    }
}
