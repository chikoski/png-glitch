use std::fmt::{Debug, Formatter};

use crate::operation::Encode;
use crate::png::parser::chunk::{Chunk, ChunkType};
use crate::png::png_error::PngError;
pub use color_type::ColorType;
use meta_data::MetaData;

mod color_type;
mod meta_data;

/// Maximum valid PNG dimension (`2^31 - 1` per PNG specification §11.2.2).
const MAX_PNG_DIMENSION: u32 = 0x7FFF_FFFF;

/// A struct representing the IHDR chunk of a PNG file.
pub struct Header {
    #[doc(hidden)]
    pub(crate) inner: Chunk, // for test
    metadata: MetaData,
    scanline_width: usize,
}

impl Header {
    fn new(
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: ColorType,
        inner: Chunk,
    ) -> Result<Header, PngError> {
        let metadata = MetaData::new(width, height, color_type, bit_depth);
        let scanline_width = metadata
            .checked_bits_per_scanline()
            .map(|bits| bits.div_ceil(8))
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(PngError::ImageTooLarge { width, height })?;
        Ok(Header {
            inner,
            metadata,
            scanline_width,
        })
    }

    /// The method returns the width of the PNG image.
    pub fn width(&self) -> u32 {
        self.metadata.width
    }

    /// The method returns the height of the PNG image.
    pub fn height(&self) -> u32 {
        self.metadata.height
    }

    /// The method returns the width of a scan line in bytes (including the 1-byte filter tag).
    pub fn scan_line_width(&self) -> usize {
        self.scanline_width
    }

    /// The method returns the color type of the PNG image.
    pub fn color_type(&self) -> ColorType {
        self.metadata.color_type
    }

    /// The method returns the bit depth of the PNG image.
    pub fn bit_depth(&self) -> u8 {
        self.metadata.bit_depth
    }

    fn parse_width(chunk: &Chunk) -> u32 {
        u32::from_be_bytes([chunk.data[0], chunk.data[1], chunk.data[2], chunk.data[3]])
    }

    fn parse_height(chunk: &Chunk) -> u32 {
        u32::from_be_bytes([chunk.data[4], chunk.data[5], chunk.data[6], chunk.data[7]])
    }

    fn parse_bit_depth(chunk: &Chunk) -> u8 {
        chunk.data[8]
    }

    fn parse_color_type(chunk: &Chunk) -> Result<ColorType, PngError> {
        ColorType::try_from(chunk.data[9])
    }

    fn parse_compression_method(chunk: &Chunk) -> Result<u8, PngError> {
        let method = chunk.data[10];
        if method == 0 {
            Ok(method)
        } else {
            Err(PngError::UnsupportedCompressionMethod)
        }
    }

    fn parse_filter_method(chunk: &Chunk) -> Result<u8, PngError> {
        let method = chunk.data[11];
        if method == 0 {
            Ok(method)
        } else {
            Err(PngError::UnsupportedFilterMethod)
        }
    }

    fn parse_interlace_method(chunk: &Chunk) -> Result<u8, PngError> {
        let method = chunk.data[12];
        if method == 0 {
            Ok(method)
        } else {
            Err(PngError::UnsupportedInterlacing)
        }
    }
}

impl TryFrom<Chunk> for Header {
    type Error = PngError;

    fn try_from(chunk: Chunk) -> Result<Self, Self::Error> {
        if chunk.chunk_type != ChunkType::Start {
            return Err(PngError::InvalidChunkType(chunk));
        }
        if chunk.data.len() < 13 {
            return Err(PngError::TooShortInput);
        }
        if chunk.data.len() != 13 {
            return Err(PngError::InvalidHeaderLength(chunk.data.len()));
        }
        Header::parse_compression_method(&chunk)?;
        Header::parse_filter_method(&chunk)?;
        Header::parse_interlace_method(&chunk)?;

        let width = Header::parse_width(&chunk);
        let height = Header::parse_height(&chunk);
        if width == 0 || height == 0 || width > MAX_PNG_DIMENSION || height > MAX_PNG_DIMENSION {
            return Err(PngError::InvalidDimensions { width, height });
        }

        let color_type = Header::parse_color_type(&chunk)?;
        let bit_depth = Header::parse_bit_depth(&chunk);
        if !color_type.is_valid_bit_depth(bit_depth) {
            return Err(PngError::InvalidBitDepth {
                color_type: color_type.code(),
                bit_depth,
            });
        }

        Header::new(width, height, bit_depth, color_type, chunk)
    }
}

impl Encode for Header {
    fn encode(&self, writer: impl std::io::Write) -> Result<(), PngError> {
        self.inner.encode(writer)
    }
}

impl Debug for Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}, scanline width = {}",
            self.metadata,
            self.scan_line_width()
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn valid_ihdr_bytes() -> Vec<u8> {
        let mut data = vec![0; 13];
        data[3] = 4; // width = 4
        data[7] = 4; // height = 4
        data[8] = 8; // bit_depth = 8
        data[9] = 2; // color_type = TrueColor
        data
    }

    #[test]
    fn test_unsupported_interlacing() {
        let mut data = valid_ihdr_bytes();
        data[12] = 1; // Interlace method 1 (Adam7)
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        let result = Header::try_from(chunk);
        assert!(matches!(result, Err(PngError::UnsupportedInterlacing)));
    }

    #[test]
    fn test_unsupported_compression() {
        let mut data = valid_ihdr_bytes();
        data[10] = 1; // Unsupported compression method
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        let result = Header::try_from(chunk);
        assert!(matches!(
            result,
            Err(PngError::UnsupportedCompressionMethod)
        ));
    }

    #[test]
    fn test_unsupported_filter() {
        let mut data = valid_ihdr_bytes();
        data[11] = 1; // Unsupported filter method
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        let result = Header::try_from(chunk);
        assert!(matches!(result, Err(PngError::UnsupportedFilterMethod)));
    }

    #[test]
    fn test_invalid_dimensions_and_bit_depth() {
        // Zero width
        let mut data = valid_ihdr_bytes();
        data[3] = 0;
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        assert!(matches!(
            Header::try_from(chunk),
            Err(PngError::InvalidDimensions {
                width: 0,
                height: 4
            })
        ));

        // Bit depth 0
        let mut data = valid_ihdr_bytes();
        data[8] = 0;
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        assert!(matches!(
            Header::try_from(chunk),
            Err(PngError::InvalidBitDepth {
                color_type: 2,
                bit_depth: 0
            })
        ));

        // Bit depth 255 on TrueColor (previously overflowed u8 in bit_per_pixel)
        let mut data = valid_ihdr_bytes();
        data[8] = 255;
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        assert!(matches!(
            Header::try_from(chunk),
            Err(PngError::InvalidBitDepth {
                color_type: 2,
                bit_depth: 255
            })
        ));

        // Bit depth 1 on TrueColor (disallowed by PNG spec)
        let mut data = valid_ihdr_bytes();
        data[8] = 1;
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        assert!(matches!(
            Header::try_from(chunk),
            Err(PngError::InvalidBitDepth {
                color_type: 2,
                bit_depth: 1
            })
        ));
    }

    #[test]
    fn test_non_byte_aligned_stride_uses_div_ceil() {
        // 3 pixels wide, 1-bit Grayscale -> ceil(3 * 1 / 8) + 1 = 1 + 1 = 2 bytes per scanline
        let mut data = valid_ihdr_bytes();
        data[3] = 3; // width = 3
        data[8] = 1; // bit_depth = 1
        data[9] = 0; // GrayScale
        let chunk = Chunk::new(ChunkType::Start, data, [0; 4]);
        let header = Header::try_from(chunk).expect("valid 3px 1-bit grayscale IHDR");
        assert_eq!(header.scan_line_width(), 2);
    }
}
