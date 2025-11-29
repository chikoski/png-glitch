use crate::operation::Encode;
use crate::png::parser::chunk::{Chunk, ChunkType};

/// A struct representing the IEND chunk of a PNG file.
pub struct Terminator {
    /// The inner chunk of the terminator.
    pub inner: Chunk,
}

impl TryFrom<Chunk> for Terminator {
    type Error = anyhow::Error;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        anyhow::ensure!(value.chunk_type == ChunkType::End);
        Ok(Terminator { inner: value })
    }
}

impl Encode for Terminator {
    fn encode(&self, writer: impl std::io::Write) -> anyhow::Result<()> {

        self.inner.encode(writer)
    }
}