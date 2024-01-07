use crate::png::chunk::{Chunk, ChunkType};
use crate::png::error::PngError;

pub struct PngHeader {
    pub inner: Chunk,
}

impl TryFrom<Chunk> for PngHeader {
    type Error = PngError;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        if value.chunk_type == ChunkType::Start {
            Ok(PngHeader { inner: value })
        } else {
            Err(PngError::InvalidChunkType(value))
        }
    }
}