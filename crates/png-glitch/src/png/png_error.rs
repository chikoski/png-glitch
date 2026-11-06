use crate::png::Chunk;
use thiserror::Error;

/// Non-fatal integrity warnings collected when parsing a PNG in lenient mode (the default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PngWarning {
    /// A chunk's stored CRC did not match its computed CRC32.
    CrcMismatch {
        /// 4-byte chunk type name (e.g. `"IDAT"`, `"tEXt"`).
        chunk_type: String,
    },
    /// The decompressed IDAT stream contained extra trailing bytes beyond `height * scan_line_width`.
    ExcessDecompressedData {
        /// Expected decompressed byte length.
        expected: usize,
    },
    /// A scan line had an invalid PNG filter type byte (`> 4`); normalized to `0` (`None`).
    InvalidFilterByte {
        /// Zero-based scan-line row index.
        row: u32,
        /// Original invalid filter byte.
        byte: u8,
    },
    /// Multiple `IDAT` chunks were separated by one or more non-`IDAT` chunks.
    NonConsecutiveIDAT,
    /// Extra bytes were present after the `IEND` chunk.
    TrailingDataAfterIEND {
        /// Number of trailing bytes ignored after `IEND`.
        bytes: usize,
    },
}

impl std::fmt::Display for PngWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrcMismatch { chunk_type } => {
                write!(f, "CRC mismatch in {chunk_type} chunk")
            }
            Self::ExcessDecompressedData { expected } => {
                write!(
                    f,
                    "decompressed IDAT stream is longer than expected ({expected} bytes); extra bytes were truncated"
                )
            }
            Self::InvalidFilterByte { row, byte } => {
                write!(
                    f,
                    "scan line {row} has invalid filter type byte {byte} (> 4); normalized to None (0)"
                )
            }
            Self::NonConsecutiveIDAT => {
                write!(f, "IDAT chunks are not consecutive")
            }
            Self::TrailingDataAfterIEND { bytes } => {
                write!(f, "{bytes} trailing byte(s) after IEND chunk were ignored")
            }
        }
    }
}

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
    /// A chunk CRC did not match its computed CRC32 (strict mode).
    #[error("CRC mismatch in {chunk_type} chunk.")]
    CrcMismatch {
        /// 4-byte chunk type name.
        chunk_type: String,
    },
    /// Decompressed IDAT data exceeded the expected scan-line buffer size (strict mode).
    #[error("Decompressed data is longer than expected ({expected} bytes).")]
    ExcessDecompressedData {
        /// Expected decompressed byte length.
        expected: usize,
    },
    /// `IDAT` chunks were separated by another chunk (strict mode).
    #[error("IDAT chunks must be consecutive.")]
    NonConsecutiveIDAT,
    /// Extra bytes were found after the `IEND` chunk (strict mode).
    #[error("Trailing data ({bytes} bytes) found after IEND chunk.")]
    TrailingDataAfterIEND {
        /// Number of extra bytes after `IEND`.
        bytes: usize,
    },
    /// A deflate failure occurs.
    #[error("Failed to deflate data.")]
    DeflateFailure,
    /// The decompressor produced fewer bytes than the scan-line buffer expected.
    /// Without this check, the trailing zeros would be silently re-encoded as
    /// image data on the next save.
    #[error(
        "Decompressed data is shorter than expected: got {actual} bytes, expected {expected} bytes."
    )]
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
        assert_eq!(
            PngError::InvalidSignature.to_string(),
            "Invalid signature found."
        );
        assert_eq!(
            PngError::TooShortInput.to_string(),
            "The input buffer is shorter than expectation."
        );
        assert_eq!(PngError::NoIHDRFound.to_string(), "No IHDR chunk found.");
        assert_eq!(PngError::NOIENDFound.to_string(), "No IEND chunk found.");
        assert_eq!(PngError::NoIDATFound.to_string(), "No IDAT chunk found.");
        assert_eq!(
            PngError::DuplicateIHDRFound.to_string(),
            "Another IHDR chunk found."
        );
        assert_eq!(
            PngError::DuplicateIENDFound.to_string(),
            "Another IEND chunk found."
        );
        assert_eq!(
            PngError::InvalidColorType.to_string(),
            "Invalid color type."
        );
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
        assert_eq!(
            PngError::InvalidFilterType.to_string(),
            "Invalid filter type."
        );
        assert_eq!(
            PngError::DeflateFailure.to_string(),
            "Failed to deflate data."
        );
        assert_eq!(
            PngError::IncompleteDecompression {
                expected: 100,
                actual: 50
            }
            .to_string(),
            "Decompressed data is shorter than expected: got 50 bytes, expected 100 bytes."
        );
        assert_eq!(
            PngError::UnsupportedInterlacing.to_string(),
            "Interlaced PNG is not supported."
        );
        assert_eq!(
            PngError::UnsupportedCompressionMethod.to_string(),
            "Unsupported compression method."
        );
        assert_eq!(
            PngError::UnsupportedFilterMethod.to_string(),
            "Unsupported filter method."
        );
    }
}
