use crate::png::Chunk;
use thiserror::Error;

/// An enum representing a PNG error.
#[derive(Error, Debug)]
pub enum PngError {
    /// The signature is invalid.
    #[error("Invalid signature found.")]
    InvalidSignature,
    /// The input is too short.
    #[error("The input buffer is shorter than expectation.")]
    TooShortInput,
    /// No IHDR chunk is found.
    #[error("No IHDR chunk found.")]
    NoIHDRFound,
    /// No IEND chunk is found.
    #[error("No IEND chunk found.")]
    NOIENDFound,
    /// No IDAT chunk is found.
    #[error("No IDAT chunk found.")]
    NoIDATFound,
    /// A duplicate IHDR chunk is found.
    #[error("Another IHDR chunk found.")]
    DuplicateIHDRFound,
    /// A duplicate IEND chunk is found.
    #[error("Another IEND chunk found.")]
    DuplicateIENDFound,
    /// An invalid chunk type is found.
    #[error("Invalid chunk type.")]
    InvalidChunkType(Chunk),
    /// An invalid IHDR chunk payload length is found (must be 13 bytes).
    #[error("Invalid IHDR chunk length: expected 13 bytes, got {0}.")]
    InvalidHeaderLength(usize),
    /// Invalid image dimensions (width or height is 0 or exceeds `0x7FFF_FFFF`).
    #[error("Invalid image dimensions: {width}x{height}.")]
    InvalidDimensions {
        /// Width from IHDR.
        width: u32,
        /// Height from IHDR.
        height: u32,
    },
    /// An invalid color type is found.
    #[error("Invalid color type.")]
    InvalidColorType,
    /// An invalid bit depth or unsupported `(color_type, bit_depth)` combination is found.
    #[error("Invalid bit depth {bit_depth} for color type {color_type}.")]
    InvalidBitDepth {
        /// Raw color type value or discriminant.
        color_type: u8,
        /// Bit depth value from IHDR.
        bit_depth: u8,
    },
    /// Image dimensions exceed the configured memory or pixel limit.
    #[error("Image dimensions {width}x{height} exceed decode limits.")]
    ImageTooLarge {
        /// Width from IHDR.
        width: u32,
        /// Height from IHDR.
        height: u32,
    },
    /// An invalid filter type is found.
    #[error("Invalid filter type.")]
    InvalidFilterType,
    /// A deflate failure occurs.
    #[error("Failed to deflate data.")]
    DeflateFailure,
    /// The decompressor produced fewer bytes than the scan-line buffer expected.
    /// Without this check, the trailing zeros would be silently re-encoded as
    /// image data on the next save.
    #[error("Decompressed data is shorter than expected: got {actual} bytes, expected {expected} bytes.")]
    IncompleteDecompression {
        /// The expected scan-line buffer size in bytes.
        expected: usize,
        /// The number of bytes actually written by the decompressor.
        actual: usize,
    },
    /// Interlaced PNG is not supported.
    #[error("Interlaced PNG is not supported.")]
    UnsupportedInterlacing,
    /// Unsupported compression method.
    #[error("Unsupported compression method.")]
    UnsupportedCompressionMethod,
    /// Unsupported filter method.
    #[error("Unsupported filter method.")]
    UnsupportedFilterMethod,
    /// An I/O error occurred while reading or writing a PNG file or stream.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        assert_eq!(PngError::InvalidSignature.to_string(), "Invalid signature found.");
        assert_eq!(PngError::TooShortInput.to_string(), "The input buffer is shorter than expectation.");
        assert_eq!(PngError::NoIHDRFound.to_string(), "No IHDR chunk found.");
        assert_eq!(PngError::NOIENDFound.to_string(), "No IEND chunk found.");
        assert_eq!(PngError::NoIDATFound.to_string(), "No IDAT chunk found.");
        assert_eq!(PngError::DuplicateIHDRFound.to_string(), "Another IHDR chunk found.");
        assert_eq!(PngError::DuplicateIENDFound.to_string(), "Another IEND chunk found.");
        assert_eq!(PngError::InvalidColorType.to_string(), "Invalid color type.");
        assert_eq!(
            PngError::InvalidBitDepth {
                color_type: 2,
                bit_depth: 1
            }
            .to_string(),
            "Invalid bit depth 1 for color type 2."
        );
        assert_eq!(
            PngError::InvalidDimensions {
                width: 0,
                height: 10
            }
            .to_string(),
            "Invalid image dimensions: 0x10."
        );
        assert_eq!(PngError::InvalidFilterType.to_string(), "Invalid filter type.");
        assert_eq!(PngError::DeflateFailure.to_string(), "Failed to deflate data.");
        assert_eq!(
            PngError::IncompleteDecompression { expected: 100, actual: 50 }.to_string(),
            "Decompressed data is shorter than expected: got 50 bytes, expected 100 bytes."
        );
        assert_eq!(PngError::UnsupportedInterlacing.to_string(), "Interlaced PNG is not supported.");
        assert_eq!(PngError::UnsupportedCompressionMethod.to_string(), "Unsupported compression method.");
        assert_eq!(PngError::UnsupportedFilterMethod.to_string(), "Unsupported filter method.");
    }
}
