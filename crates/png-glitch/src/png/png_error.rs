use thiserror::Error;
use crate::png::Chunk;

#[derive(Error, Debug)]
pub enum PngError {
    #[error("Invalid signature found.")]
    InvalidSignature,
    #[error("The input buffer is shorter than expectation.")]
    TooShortInput,
    #[error("No IHDR chunk found.")]
    NoIHDRFound,
    #[error("No IEND chunk found.")]
    NOIENDFound,
    #[error("No IDAT chunk found.")]
    NoIDATFound,
    #[error("Another IHDR chunk found.")]
    DuplicateIHDRFound,
    #[error("Another IEND chunk found.")]
    DuplicateIENDFound,
    #[error("Invalid chunk type.")]
    InvalidChunkType(Chunk),
    #[error("Invalid color type.")]
    InvalidColorType,
    #[error("Invalid filter type.")]
    InvalidFilterType,
}