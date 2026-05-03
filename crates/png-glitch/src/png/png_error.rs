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
}
