use thiserror::Error;

/// Errors that can occur while parsing, decoding, or encoding WebP images.
#[derive(Debug, Error)]
pub enum WebpError {
    /// The input data does not start with a valid `RIFF` header.
    #[error("invalid RIFF signature")]
    InvalidSignature,

    /// The RIFF container form type is not `WEBP`.
    #[error("invalid WEBP form type")]
    InvalidFormType,

    /// A required RIFF chunk was not found.
    #[error("chunk not found: {}", format_id(.0))]
    ChunkNotFound([u8; 4]),

    /// Failed to decode WebP image data via `libwebp`.
    #[error("WebP decode error: {0}")]
    DecodeError(String),

    /// Failed to encode WebP image data via `libwebp`.
    #[error("WebP encode error: {0}")]
    EncodeError(String),

    /// An I/O error occurred while reading or writing a WebP file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn format_id(id: &[u8; 4]) -> String {
    String::from_utf8_lossy(id).into_owned()
}
