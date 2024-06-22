use crate::png::parser::chunk::{Chunk, ChunkType};
use crate::png::png_error::PngError;

pub struct Terminator {
    pub inner: Chunk,
}

impl TryFrom<Chunk> for Terminator {
    type Error = PngError;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        if value.chunk_type == ChunkType::End {
            Ok(Terminator { inner: value })
        } else {
            Err(PngError::InvalidChunkType(value))
        }
    }
}
