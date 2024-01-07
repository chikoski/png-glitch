use crate::png::chunk::{Chunk, ChunkType};
use crate::png::error::PngError;

pub struct PngTerminator {
    pub inner: Chunk
}

impl TryFrom<Chunk> for PngTerminator {
    type Error = PngError;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        if value.chunk_type == ChunkType::End {
            Ok(PngTerminator{ inner: value})
        }else{
            Err(PngError::InvalidChunkType(value))
        }
    }
}