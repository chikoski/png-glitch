use crate::png::ColorType;
use crate::png::png_error::PixelError;
pub use filter_type::FilterType;
pub use pixel::Pixel;
use std::io::{Read, Write};
use std::ops::{Index, IndexMut, Range};

pub(crate) mod filter;
mod filter_type;
mod pixel;

/// A type alias for a range of `usize`.
pub type UsizeRange = Range<usize>;

/// Bytes per pixel for a header's color type and bit depth (at least 1, as the PNG specification
/// uses for filtering when pixels are smaller than a byte).
pub(crate) fn bytes_per_pixel(color_type: ColorType, bit_depth: u8) -> usize {
    let bits = bit_depth as usize;
    match color_type {
        ColorType::GrayScale => std::cmp::max(bits / 8, 1),
        ColorType::GrayScaleAlpha => std::cmp::max(bits * 2 / 8, 1),
        ColorType::TrueColor => std::cmp::max(bits * 3 / 8, 1),
        ColorType::TrueColorAlpha => std::cmp::max(bits * 4 / 8, 1),
        ColorType::IndexColor => std::cmp::max(bits / 8, 1),
    }
}

/// Decodes the pixel `x` of a scan line (`data` is the filter-type tag plus the filtered bytes).
/// The sample decoder of the read-only views ([`ScanLines::pixel`](crate::ScanLines::pixel),
/// [`PngGlitch::pixel`](crate::PngGlitch::pixel)), which cannot build a `ScanLine` (that needs
/// `&mut` data), and the single decoder behind [`ScanLine::get_pixel`], which calls it. It is
/// `#[inline(always)]` because `get_pixel` is the per-pixel hot path of `PixelSort` (#201).
#[inline(always)]
pub(crate) fn pixel_at(
    data: &[u8],
    color_type: ColorType,
    bit_depth: u8,
    width: u32,
    x: usize,
) -> Option<Pixel> {
    if x >= width as usize || bit_depth == 0 {
        return None;
    }
    if bit_depth < 8 {
        let pixels_per_byte = 8 / bit_depth as usize;
        let byte_offset = 1 + x / pixels_per_byte;
        let byte = *data.get(byte_offset)?;
        let shift = (pixels_per_byte - 1 - (x % pixels_per_byte)) * bit_depth as usize;
        let mask = (1 << bit_depth) - 1;
        let value = (byte >> shift) & mask;

        return match color_type {
            ColorType::GrayScale => Some(Pixel::Gray(value as u16)),
            ColorType::IndexColor => Some(Pixel::Indexed(value)),
            _ => None,
        };
    }

    let bpp = bytes_per_pixel(color_type, bit_depth);
    let offset = 1 + x * bpp;
    decode_pixel(data.get(offset..offset + bpp)?, color_type, bit_depth)
}

/// Decodes one pixel of 8 or 16 bits per sample from its bytes. `None` for a sub-byte bit depth,
/// a colour type / bit depth pair without such pixels, or a slice of the wrong length. The slice
/// patterns check the length once, so there is no per-sample bounds check (and no panic).
#[inline(always)]
fn decode_pixel(slice: &[u8], color_type: ColorType, bit_depth: u8) -> Option<Pixel> {
    use ColorType::*;
    let be = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
    Some(match (color_type, bit_depth, slice) {
        (GrayScale, 8, &[v]) => Pixel::Gray(v.into()),
        (GrayScale, 16, &[v0, v1]) => Pixel::Gray(be(v0, v1)),
        (TrueColor, 8, &[r, g, b]) => Pixel::RGB(r.into(), g.into(), b.into()),
        (TrueColor, 16, &[r0, r1, g0, g1, b0, b1]) => {
            Pixel::RGB(be(r0, r1), be(g0, g1), be(b0, b1))
        }
        (IndexColor, 8, &[v]) => Pixel::Indexed(v),
        (GrayScaleAlpha, 8, &[v, a]) => Pixel::GrayAlpha(v.into(), a.into()),
        (GrayScaleAlpha, 16, &[v0, v1, a0, a1]) => Pixel::GrayAlpha(be(v0, v1), be(a0, a1)),
        (TrueColorAlpha, 8, &[r, g, b, a]) => Pixel::RGBA(r.into(), g.into(), b.into(), a.into()),
        (TrueColorAlpha, 16, &[r0, r1, g0, g1, b0, b1, a0, a1]) => {
            Pixel::RGBA(be(r0, r1), be(g0, g1), be(b0, b1), be(a0, a1))
        }
        _ => return None,
    })
}

/// Writes `pixel` into the bytes of one pixel of 8 or 16 bits per sample. Does nothing if the
/// [`Pixel`] variant does not match the colour type, the bit depth is not 8 or 16, or the slice
/// has the wrong length. Samples are narrowed to the bit depth (`as u8` at 8 bits).
#[inline(always)]
fn encode_pixel(slice: &mut [u8], pixel: Pixel, color_type: ColorType, bit_depth: u8) {
    use ColorType::*;
    match (pixel, color_type, bit_depth) {
        (Pixel::Gray(v), GrayScale, 8) => put(slice, [v as u8]),
        (Pixel::Gray(v), GrayScale, 16) => put(slice, v.to_be_bytes()),
        (Pixel::Indexed(v), IndexColor, 8) => put(slice, [v]),
        (Pixel::RGB(r, g, b), TrueColor, 8) => put(slice, [r as u8, g as u8, b as u8]),
        (Pixel::RGB(r, g, b), TrueColor, 16) => put(slice, be3(r, g, b)),
        (Pixel::GrayAlpha(v, a), GrayScaleAlpha, 8) => put(slice, [v as u8, a as u8]),
        (Pixel::GrayAlpha(v, a), GrayScaleAlpha, 16) => {
            let ([v0, v1], [a0, a1]) = (v.to_be_bytes(), a.to_be_bytes());
            put(slice, [v0, v1, a0, a1])
        }
        (Pixel::RGBA(r, g, b, a), TrueColorAlpha, 8) => {
            put(slice, [r as u8, g as u8, b as u8, a as u8])
        }
        (Pixel::RGBA(r, g, b, a), TrueColorAlpha, 16) => {
            let [r0, r1, g0, g1, b0, b1] = be3(r, g, b);
            let [a0, a1] = a.to_be_bytes();
            put(slice, [r0, r1, g0, g1, b0, b1, a0, a1])
        }
        _ => {}
    }
}

/// Three 16-bit samples, big-endian.
#[inline(always)]
fn be3(r: u16, g: u16, b: u16) -> [u8; 6] {
    let ([r0, r1], [g0, g1], [b0, b1]) = (r.to_be_bytes(), g.to_be_bytes(), b.to_be_bytes());
    [r0, r1, g0, g1, b0, b1]
}

/// Runs `f` over the whole pixels of `data` (the bytes after the filter-type tag), `N` bytes
/// each, at most `width` of them. `decode` and `encode` are for one colour type and bit depth,
/// so that they inline into the loop (#140: no per-pixel format match, no bounds checks).
#[inline(always)]
fn process_fixed<const N: usize, F: FnMut(usize, Pixel) -> Pixel>(
    data: &mut [u8],
    width: usize,
    f: &mut F,
    decode: impl Fn(&[u8; N]) -> Pixel,
    encode: impl Fn(&mut [u8; N], Pixel),
) {
    let (pixels, _) = data.as_chunks_mut::<N>();
    for (x, bytes) in pixels.iter_mut().take(width).enumerate() {
        let pixel = decode(bytes);
        encode(bytes, f(x, pixel));
    }
}

/// Copies `bytes` into `slice` if it is exactly `N` bytes long.
#[inline(always)]
fn put<const N: usize>(slice: &mut [u8], bytes: [u8; N]) {
    if let Ok(dst) = <&mut [u8; N]>::try_from(slice) {
        *dst = bytes;
    }
}

/// A struct representing a scan line in a PNG image.
pub struct ScanLine<'a> {
    pub(crate) data: &'a mut [u8],
    pub(crate) color_type: ColorType,
    pub(crate) bit_depth: u8,
    pub(crate) width: u32,
}

impl<'a> ScanLine<'a> {
    pub(crate) fn new(
        data: &'a mut [u8],
        color_type: ColorType,
        bit_depth: u8,
        width: u32,
    ) -> ScanLine<'a> {
        ScanLine {
            data,
            color_type,
            bit_depth,
            width,
        }
    }

    /// The method returns the number of pixels in the scan line.
    pub fn pixels_count(&self) -> u32 {
        self.width
    }

    pub(crate) fn pixel_data_offset(&self) -> usize {
        1
    }

    pub(crate) fn pixel_data_range(&self) -> UsizeRange {
        self.pixel_data_offset()..self.data.len()
    }

    /// The method returns the number of bytes per pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        bytes_per_pixel(self.color_type, self.bit_depth)
    }

    /// The method returns a pixel at the specified index.
    ///
    /// The value is the *stored* sample, at the image's bit depth. If the scan line is still
    /// filtered it is a filtered byte, not the decoded colour; call
    /// [`PngGlitch::remove_filter`](crate::PngGlitch::remove_filter) first to read real pixels.
    ///
    /// The decoder is the crate-private `pixel_at`, shared with the read-only views
    /// ([`ScanLines::pixel`](crate::ScanLines::pixel)); it is `#[inline(always)]`, so the
    /// per-pixel hot path of `PixelSort` and similar effects has no call. See #201 and #202 for
    /// the benchmark history of this function.
    #[inline(always)]
    pub fn get_pixel(&self, x: usize) -> Option<Pixel> {
        pixel_at(self.data, self.color_type, self.bit_depth, self.width, x)
    }

    /// Sets the pixel `x`, or says why it cannot.
    ///
    /// Unlike [`set_pixel`](Self::set_pixel), which silently does nothing, this checks that
    /// `x` is inside the row, that the [`Pixel`] variant matches the colour type, and that every
    /// sample fits the bit depth (`Gray(5)` does not fit 2 bits; `Gray(300)` does not fit 8).
    /// For [`Pixel::Indexed`] the index must fit the bit depth too (it is not checked against
    /// the palette).
    ///
    /// The sample written is the *stored* one: on a scan line that is still filtered it is a
    /// filtered byte. Call [`PngGlitch::remove_filter`](crate::PngGlitch::remove_filter) first
    /// to write real pixels.
    ///
    /// # Errors
    /// [`PixelError::OutOfBounds`] (with `y` = 0), [`PixelError::ColorTypeMismatch`],
    /// [`PixelError::SampleOutOfRange`].
    ///
    /// # Examples
    /// ```
    /// use png_glitch::{Encoding, FilterStrategy, FilterType, Pixel, PixelError, PixelSource, PngGlitch};
    ///
    /// let frame = vec![0u8; 3 * 2 * 4];
    /// let mut png = PngGlitch::from_pixels(
    ///     PixelSource::rgba8(3, 2, &frame),
    ///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
    /// )?;
    /// let mut lines = png.scan_lines_mut();
    /// let mut row = lines.get_mut(0).unwrap();
    /// assert_eq!(row.try_set_pixel(1, Pixel::RGBA(1, 2, 3, 4)), Ok(()));
    /// assert_eq!(row.get_pixel(1), Some(Pixel::RGBA(1, 2, 3, 4)));
    /// assert_eq!(row.try_set_pixel(1, Pixel::Gray(1)), Err(PixelError::ColorTypeMismatch));
    /// assert_eq!(row.try_set_pixel(1, Pixel::RGBA(300, 0, 0, 0)), Err(PixelError::SampleOutOfRange));
    /// assert_eq!(row.try_set_pixel(3, Pixel::RGBA(0, 0, 0, 0)), Err(PixelError::OutOfBounds { x: 3, y: 0 }));
    /// # Ok::<(), png_glitch::PngError>(())
    /// ```
    pub fn try_set_pixel(&mut self, x: usize, pixel: Pixel) -> Result<(), PixelError> {
        let out_of_bounds = PixelError::OutOfBounds {
            x: u32::try_from(x).unwrap_or(u32::MAX),
            y: 0,
        };
        if x >= self.width as usize || self.bit_depth == 0 {
            return Err(out_of_bounds);
        }
        let matches = matches!(
            (pixel, self.color_type),
            (Pixel::Gray(_), ColorType::GrayScale)
                | (Pixel::GrayAlpha(..), ColorType::GrayScaleAlpha)
                | (Pixel::RGB(..), ColorType::TrueColor)
                | (Pixel::RGBA(..), ColorType::TrueColorAlpha)
                | (Pixel::Indexed(_), ColorType::IndexColor)
        );
        if !matches {
            return Err(PixelError::ColorTypeMismatch);
        }
        let max = (1u32 << self.bit_depth.min(16)) - 1;
        let fits = |v: u16| u32::from(v) <= max;
        let in_range = match pixel {
            Pixel::Gray(v) => fits(v),
            Pixel::GrayAlpha(v, a) => fits(v) && fits(a),
            Pixel::RGB(r, g, b) => fits(r) && fits(g) && fits(b),
            Pixel::RGBA(r, g, b, a) => fits(r) && fits(g) && fits(b) && fits(a),
            Pixel::Indexed(v) => fits(u16::from(v)),
        };
        if !in_range {
            return Err(PixelError::SampleOutOfRange);
        }
        // The row may be shorter than its header says (a malformed image).
        if pixel_at(self.data, self.color_type, self.bit_depth, self.width, x).is_none() {
            return Err(out_of_bounds);
        }
        self.set_pixel(x, pixel);
        Ok(())
    }

    /// The method sets a pixel at the specified index.
    ///
    /// It silently does nothing if `x` is out of range or the [`Pixel`] variant does not match
    /// the colour type, and it masks or narrows samples that are too large for the bit depth.
    /// Use [`try_set_pixel`](Self::try_set_pixel) to be told about those cases.
    pub fn set_pixel(&mut self, x: usize, pixel: Pixel) {
        if x >= self.width as usize || self.bit_depth == 0 {
            return;
        }
        if self.bit_depth < 8 {
            let pixels_per_byte = 8 / self.bit_depth as usize;
            let byte_offset = self.pixel_data_offset() + x / pixels_per_byte;
            if byte_offset >= self.data.len() {
                return;
            }

            let value = match (pixel, self.color_type) {
                (Pixel::Gray(v), ColorType::GrayScale) => v as u8,
                (Pixel::Indexed(v), ColorType::IndexColor) => v,
                _ => return,
            };

            let shift = (pixels_per_byte - 1 - (x % pixels_per_byte)) * self.bit_depth as usize;
            let mask = (1 << self.bit_depth) - 1;
            let value = (value & mask) << shift;

            if let Some(byte) = self.data.get_mut(byte_offset) {
                let clean_mask = !(mask << shift);
                *byte = (*byte & clean_mask) | value;
            }
            return;
        }

        let bpp = self.bytes_per_pixel();
        let offset = self.pixel_data_offset() + x * bpp;
        if let Some(slice) = self.data.get_mut(offset..offset + bpp) {
            encode_pixel(slice, pixel, self.color_type, self.bit_depth);
        }
    }

    /// The method applies a filter to the scan line.
    /// The `filter_type` parameter is the type of the filter to apply.
    /// The `previous` parameter is the previous scan line.
    pub fn apply_filter(&mut self, filter_type: FilterType, previous_line: Option<&ScanLine>) {
        filter::apply(filter_type, self, previous_line);
        self.set_filter_type(filter_type);
    }

    /// Reconstructs raw pixel bytes by removing the PNG prediction filter from this scan line.
    pub fn remove_filter(&mut self, previous_line: Option<&ScanLine>) {
        filter::remove(self, previous_line);
        self.set_filter_type(FilterType::None);
    }

    /// The method changes the filter type of the ScanLine.
    /// It first calculates the original pixel value (decodes it),
    /// and then calculates the scan line data based on the new filter type.
    pub fn change_filter_type(
        &mut self,
        filter_type: FilterType,
        previous_line: Option<&ScanLine>,
    ) {
        self.remove_filter(previous_line);
        self.apply_filter(filter_type, previous_line);
    }

    /// This method returns the filter method applied to the scan line.
    pub fn filter_type(&self) -> FilterType {
        self.data
            .first()
            .copied()
            .and_then(|b| FilterType::try_from(b).ok())
            .unwrap_or(FilterType::None)
    }

    /// This method updates the filter method of the scan line with the specified one.
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        if let Some(first) = self.data.first_mut() {
            *first = filter_type.into();
        }
    }

    /// This method returns the byte size of the scan line.
    pub fn size(&self) -> usize {
        self.data.len().saturating_sub(1)
    }

    /// This method returns the color type of the scan line.
    pub fn color_type(&self) -> ColorType {
        self.color_type
    }

    /// This method returns the bit_depth of each pixel.
    pub fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    /// The method returns a byte in a pixel_data specified with the index parameter.
    pub fn index(&self, index: usize) -> Option<u8> {
        let pixel_data_offset = self.pixel_data_offset();
        let index = pixel_data_offset + index;
        self.data.get(index).copied()
    }

    /// The method updates a value of the pixel specified by the index with the given value.
    pub fn update(&mut self, index: usize, value: u8) {
        let pixel_data_offset = self.pixel_data_offset();
        let index = pixel_data_offset + index;
        if let Some(byte) = self.data.get_mut(index) {
            *byte = value;
        }
    }

    /// The method applies a closure to each pixel in the scan line.
    /// This is more efficient than get_pixel/set_pixel in a loop because it
    /// can optimize the color type and bit depth checks.
    pub fn process_pixels<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, Pixel) -> Pixel,
    {
        if self.bit_depth == 0 {
            return;
        }
        let width = self.width as usize;
        let color_type = self.color_type;
        let bit_depth = self.bit_depth;
        let offset = self.pixel_data_offset();

        if bit_depth < 8 {
            let pixels_per_byte = 8 / bit_depth as usize;
            let mask = (1 << bit_depth) - 1;

            for x in 0..width {
                let byte_offset = offset + x / pixels_per_byte;
                let Some(&byte) = self.data.get(byte_offset) else {
                    break;
                };
                let shift = (pixels_per_byte - 1 - (x % pixels_per_byte)) * bit_depth as usize;
                let value = (byte >> shift) & mask;

                let pixel = match color_type {
                    ColorType::GrayScale => Pixel::Gray(value as u16),
                    ColorType::IndexColor => Pixel::Indexed(value),
                    _ => continue,
                };

                let updated = f(x, pixel);

                let new_value = match (updated, color_type) {
                    (Pixel::Gray(v), ColorType::GrayScale) => v as u8,
                    (Pixel::Indexed(v), ColorType::IndexColor) => v,
                    _ => continue,
                };

                let clean_mask = !(mask << shift);
                let value_to_set = (new_value & mask) << shift;
                if let Some(byte) = self.data.get_mut(byte_offset) {
                    *byte = (*byte & clean_mask) | value_to_set;
                }
            }
            return;
        }

        let Some(data) = self.data.get_mut(offset..) else {
            return;
        };
        let f = &mut f;
        let be = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
        // One loop per format; a `Pixel` of another variant than the format's leaves the pixel
        // as it was.
        use ColorType::*;
        match (color_type, bit_depth) {
            (GrayScale, 8) => process_fixed(
                data,
                width,
                f,
                |&[v]: &[u8; 1]| Pixel::Gray(v.into()),
                |c, p| {
                    if let Pixel::Gray(v) = p {
                        *c = [v as u8];
                    }
                },
            ),
            (GrayScale, 16) => process_fixed(
                data,
                width,
                f,
                |&[v0, v1]: &[u8; 2]| Pixel::Gray(be(v0, v1)),
                |c, p| {
                    if let Pixel::Gray(v) = p {
                        *c = v.to_be_bytes();
                    }
                },
            ),
            (IndexColor, 8) => process_fixed(
                data,
                width,
                f,
                |&[v]: &[u8; 1]| Pixel::Indexed(v),
                |c, p| {
                    if let Pixel::Indexed(v) = p {
                        *c = [v];
                    }
                },
            ),
            (TrueColor, 8) => process_fixed(
                data,
                width,
                f,
                |&[r, g, b]: &[u8; 3]| Pixel::RGB(r.into(), g.into(), b.into()),
                |c, p| {
                    if let Pixel::RGB(r, g, b) = p {
                        *c = [r as u8, g as u8, b as u8];
                    }
                },
            ),
            (TrueColor, 16) => process_fixed(
                data,
                width,
                f,
                |&[r0, r1, g0, g1, b0, b1]: &[u8; 6]| {
                    Pixel::RGB(be(r0, r1), be(g0, g1), be(b0, b1))
                },
                |c, p| {
                    if let Pixel::RGB(r, g, b) = p {
                        *c = be3(r, g, b);
                    }
                },
            ),
            (GrayScaleAlpha, 8) => process_fixed(
                data,
                width,
                f,
                |&[v, a]: &[u8; 2]| Pixel::GrayAlpha(v.into(), a.into()),
                |c, p| {
                    if let Pixel::GrayAlpha(v, a) = p {
                        *c = [v as u8, a as u8];
                    }
                },
            ),
            (GrayScaleAlpha, 16) => process_fixed(
                data,
                width,
                f,
                |&[v0, v1, a0, a1]: &[u8; 4]| Pixel::GrayAlpha(be(v0, v1), be(a0, a1)),
                |c, p| {
                    if let Pixel::GrayAlpha(v, a) = p {
                        let ([v0, v1], [a0, a1]) = (v.to_be_bytes(), a.to_be_bytes());
                        *c = [v0, v1, a0, a1];
                    }
                },
            ),
            (TrueColorAlpha, 8) => process_fixed(
                data,
                width,
                f,
                |&[r, g, b, a]: &[u8; 4]| Pixel::RGBA(r.into(), g.into(), b.into(), a.into()),
                |c, p| {
                    if let Pixel::RGBA(r, g, b, a) = p {
                        *c = [r as u8, g as u8, b as u8, a as u8];
                    }
                },
            ),
            (TrueColorAlpha, 16) => process_fixed(
                data,
                width,
                f,
                |&[r0, r1, g0, g1, b0, b1, a0, a1]: &[u8; 8]| {
                    Pixel::RGBA(be(r0, r1), be(g0, g1), be(b0, b1), be(a0, a1))
                },
                |c, p| {
                    if let Pixel::RGBA(r, g, b, a) = p {
                        let [r0, r1, g0, g1, b0, b1] = be3(r, g, b);
                        let [a0, a1] = a.to_be_bytes();
                        *c = [r0, r1, g0, g1, b0, b1, a0, a1];
                    }
                },
            ),
            // No such pixels: nothing was decoded before either.
            _ => {}
        }
    }
}

impl<'a> Index<usize> for ScanLine<'a> {
    type Output = u8;

    #[expect(
        clippy::indexing_slicing,
        reason = "`Index` must panic out of range, as a slice does; `ScanLine::index` and \
                  `ScanLine::update` are the checked forms"
    )]
    fn index(&self, index: usize) -> &Self::Output {
        let index = index + self.pixel_data_offset();
        &self.data[index]
    }
}

impl<'a> IndexMut<usize> for ScanLine<'a> {
    #[expect(
        clippy::indexing_slicing,
        reason = "`IndexMut` must panic out of range, as a slice does; `ScanLine::update` is \
                  the checked form"
    )]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let index = index + self.pixel_data_offset();
        &mut self.data[index]
    }
}

impl<'a> Read for ScanLine<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut buffer = self.data.get(self.pixel_data_range()).unwrap_or_default();
        buffer.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut buffer = self.data.get(self.pixel_data_range()).unwrap_or_default();
        buffer.read_to_end(buf)
    }
}

impl<'a> Write for ScanLine<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let pixel_data_range = self.pixel_data_range();
        let mut buffer = self.data.get_mut(pixel_data_range).unwrap_or_default();
        buffer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.data.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TestTarget {
        buffer: Vec<u8>,
    }

    impl TestTarget {
        fn new() -> Self {
            let buffer = vec![0, 1, 2, 3, 4, 5];
            TestTarget { buffer }
        }

        fn scan_line(&mut self) -> ScanLine<'_> {
            ScanLine::new(&mut self.buffer, ColorType::TrueColorAlpha, 8, 1)
        }
    }

    mod index {
        use super::*;

        #[test]
        fn test_index() {
            let mut target = TestTarget::new();
            let scan_line = target.scan_line();

            assert_eq!(scan_line[0], 1);
        }

        #[test]
        fn test_index_mut() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            scan_line[0] = 10;

            assert_eq!(scan_line[0], 10);
            assert_eq!(target.buffer[1], 10);
        }
    }

    mod read {
        use std::io::Read;

        use super::*;

        #[test]
        fn test_read() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![0; scan_line.size()];

            let result = scan_line.read(&mut buffer);
            assert!(result.is_ok());
            assert_eq!(scan_line.size(), buffer.len());
            assert_eq!(&target.buffer[1..], &buffer);
        }

        #[test]
        fn test_read_to_end() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();

            let mut buffer = vec![];

            let size = scan_line.size();
            let result = scan_line.read_to_end(&mut buffer);
            assert!(result.is_ok());
            assert_eq!(&target.buffer[1..], &buffer[0..size]);
        }
    }

    mod write {
        use super::*;

        #[test]
        fn test_write() {
            let mut target = TestTarget::new();
            let mut scan_line = target.scan_line();
            let size = scan_line.size();

            let buffer = vec![10; size];
            let result = scan_line.write(&buffer);
            assert!(result.is_ok());
            assert_eq!(buffer.len(), result.unwrap());
            assert_eq!(&buffer, &target.buffer[1..]);
        }
    }

    mod change_filter_type {
        use super::*;

        #[test]
        fn test_change_filter_type() {
            let mut buffer = vec![0, 1, 2, 3, 4, 5];
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 8, 5);

                scan_line.change_filter_type(FilterType::Sub, None);
                assert_eq!(FilterType::Sub, scan_line.filter_type());
            }
            let encoded_sub = vec![1, 1, 1, 1, 1];
            assert_eq!(encoded_sub, &buffer[1..]);

            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 8, 5);
                scan_line.change_filter_type(FilterType::None, None);
                assert_eq!(FilterType::None, scan_line.filter_type());
            }
            let raw_data = vec![1, 2, 3, 4, 5];
            assert_eq!(raw_data, &buffer[1..]);
        }
    }

    mod pixel {
        use super::*;

        #[test]
        fn test_get_set_pixel_rgba8() {
            let mut buffer = vec![0, 10, 20, 30, 40, 50, 60, 70, 80];
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::TrueColorAlpha, 8, 2);

                let p = scan_line.get_pixel(0).unwrap();
                assert_eq!(p, Pixel::RGBA(10, 20, 30, 40));

                scan_line.set_pixel(0, Pixel::RGBA(100, 110, 120, 130));
            }
            assert_eq!(buffer[1], 100);
            assert_eq!(buffer[4], 130);

            {
                let scan_line = ScanLine::new(&mut buffer, ColorType::TrueColorAlpha, 8, 2);
                let p2 = scan_line.get_pixel(1).unwrap();
                assert_eq!(p2, Pixel::RGBA(50, 60, 70, 80));
            }
        }

        #[test]
        fn test_get_set_pixel_gray8() {
            let mut buffer = vec![0, 10, 20, 30];
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 8, 3);

                let p = scan_line.get_pixel(1).unwrap();
                assert_eq!(p, Pixel::Gray(20));

                scan_line.set_pixel(2, Pixel::Gray(200));
            }
            assert_eq!(buffer[3], 200);
        }

        #[test]
        fn test_get_set_pixel_sub_8bit() {
            // 1-bit GrayScale
            let mut buffer = vec![0, 0b10110000]; // filter type + 1 byte (8 pixels)
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 1, 8);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Gray(0));
                assert_eq!(scan_line.get_pixel(2).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(4).unwrap(), Pixel::Gray(0));

                scan_line.set_pixel(4, Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(4).unwrap(), Pixel::Gray(1));
            }
            assert_eq!(buffer[1], 0b10111000);

            // 2-bit GrayScale
            let mut buffer = vec![0, 0b11011000]; // 11, 01, 10, 00
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::GrayScale, 2, 4);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Gray(3));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(2).unwrap(), Pixel::Gray(2));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(0));

                scan_line.set_pixel(3, Pixel::Gray(1));
                assert_eq!(scan_line.get_pixel(3).unwrap(), Pixel::Gray(1));
            }
            assert_eq!(buffer[1], 0b11011001);

            // 4-bit IndexColor
            let mut buffer = vec![0, 0b10100101]; // 1010, 0101 (10, 5)
            {
                let mut scan_line = ScanLine::new(&mut buffer, ColorType::IndexColor, 4, 2);
                assert_eq!(scan_line.get_pixel(0).unwrap(), Pixel::Indexed(10));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Indexed(5));

                scan_line.set_pixel(1, Pixel::Indexed(15));
                assert_eq!(scan_line.get_pixel(1).unwrap(), Pixel::Indexed(15));
            }
            assert_eq!(buffer[1], 0b10101111);
        }
    }
}

/// The public pixel API with out-of-range arguments and malformed rows (#140): for every colour
/// type and bit depth, rows of any length (also shorter or longer than the header width says),
/// and indices past the end. Nothing may panic, and the checked accessors must agree.
#[cfg(test)]
mod out_of_range {
    use super::*;
    use proptest::prelude::*;

    const FORMATS: [(ColorType, u8); 15] = [
        (ColorType::GrayScale, 1),
        (ColorType::GrayScale, 2),
        (ColorType::GrayScale, 4),
        (ColorType::GrayScale, 8),
        (ColorType::GrayScale, 16),
        (ColorType::TrueColor, 8),
        (ColorType::TrueColor, 16),
        (ColorType::IndexColor, 1),
        (ColorType::IndexColor, 2),
        (ColorType::IndexColor, 4),
        (ColorType::IndexColor, 8),
        (ColorType::GrayScaleAlpha, 8),
        (ColorType::GrayScaleAlpha, 16),
        (ColorType::TrueColorAlpha, 8),
        (ColorType::TrueColorAlpha, 16),
    ];

    fn pixel_for(color_type: ColorType, v: u16) -> Pixel {
        match color_type {
            ColorType::GrayScale => Pixel::Gray(v),
            ColorType::TrueColor => Pixel::RGB(v, v.wrapping_add(1), v.wrapping_add(2)),
            ColorType::IndexColor => Pixel::Indexed(v as u8),
            ColorType::GrayScaleAlpha => Pixel::GrayAlpha(v, v.wrapping_add(3)),
            ColorType::TrueColorAlpha => Pixel::RGBA(v, 1, 2, v.wrapping_add(4)),
        }
    }

    proptest! {
        #[test]
        fn the_pixel_api_never_panics_and_agrees(
            f in 0..FORMATS.len(),
            width in 0u32..40,
            mut data in proptest::collection::vec(any::<u8>(), 0..120),
            xs in proptest::collection::vec(0usize..200, 1..12),
            v in any::<u16>(),
        ) {
            let (color_type, bit_depth) = FORMATS[f];
            let original = data.clone();
            let mut line = ScanLine::new(&mut data, color_type, bit_depth, width);
            for &x in &xs {
                // `get_pixel` is `pixel_at`, and is `None` exactly past the width or the data.
                let got = line.get_pixel(x);
                prop_assert_eq!(got, pixel_at(line.data, color_type, bit_depth, width, x));
                let bits = usize::from(bit_depth) * match color_type {
                    ColorType::TrueColor => 3,
                    ColorType::GrayScaleAlpha => 2,
                    ColorType::TrueColorAlpha => 4,
                    _ => 1,
                };
                let fits = x < width as usize && 8 + (x + 1) * bits <= original.len() * 8;
                prop_assert_eq!(got.is_some(), fits, "x {} width {} len {}", x, width, original.len());
                let _ = line.index(x);
                line.update(x, 7);
                let p = pixel_for(color_type, v);
                line.set_pixel(x, p);
                let _ = line.try_set_pixel(x, p);
                let _ = line.try_set_pixel(x, Pixel::Gray(1));
            }
            let _ = line.filter_type();
            let mut buf = [0u8; 8];
            let _ = std::io::Read::read(&mut line, &mut buf);
            let _ = std::io::Write::write(&mut line, &buf);
            // Decoding and encoding again changes nothing.
            let before = line.data.to_vec();
            line.process_pixels(|_, p| p);
            prop_assert_eq!(line.data.to_vec(), before);
            line.process_pixels(|_, _| Pixel::Gray(3));
        }

        #[test]
        fn set_then_get_round_trips_in_range(
            f in 0..FORMATS.len(),
            width in 1u32..20,
            x in 0usize..20,
            v in any::<u16>(),
        ) {
            let (color_type, bit_depth) = FORMATS[f];
            let x = x % width as usize;
            let len = 1 + (width as usize * usize::from(bit_depth) * 4).div_ceil(8);
            let mut data = vec![0u8; len];
            let mut line = ScanLine::new(&mut data, color_type, bit_depth, width);
            let max = ((1u32 << bit_depth.min(16)) - 1) as u16;
            let p = pixel_for(color_type, v & max);
            if line.try_set_pixel(x, p).is_ok() {
                prop_assert_eq!(line.get_pixel(x), Some(p));
            }
        }
    }
}
