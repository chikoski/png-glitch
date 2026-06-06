use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebpError {
    #[error("invalid RIFF signature")]
    InvalidSignature,

    #[error("invalid WEBP form type")]
    InvalidFormType,

    #[error("chunk not found: {}", format_id(.0))]
    ChunkNotFound([u8; 4]),

    #[error("WebP decode error: {0}")]
    DecodeError(String),

    #[error("WebP encode error: {0}")]
    EncodeError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn format_id(id: &[u8; 4]) -> String {
    String::from_utf8_lossy(id).into_owned()
}
