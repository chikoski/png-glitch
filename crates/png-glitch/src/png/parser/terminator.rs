use crate::operation::Encode;
use crate::png::parser::chunk::{Chunk, ChunkType};
use crate::png::png_error::PngError;

/// A struct representing the IEND chunk of a PNG file.
#[derive(Debug)]
pub struct Terminator {
    /// The inner chunk of the terminator.
    pub inner: Chunk,
}

impl TryFrom<Chunk> for Terminator {
    type Error = PngError;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        if value.chunk_type != ChunkType::End {
            return Err(PngError::InvalidChunkType(value));
        }
        Ok(Terminator { inner: value })
    }
}

impl Encode for Terminator {
    fn encode(&self, writer: impl std::io::Write) -> Result<(), PngError> {
        self.inner.encode(writer)
    }
}