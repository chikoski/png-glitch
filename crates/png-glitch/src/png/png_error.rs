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
    /// An invalid color type is found.
    #[error("Invalid color type.")]
    InvalidColorType,
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

