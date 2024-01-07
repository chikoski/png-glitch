use anyhow::Context;

pub use crate::png::chunk::chunk_type::ChunkType;
use crate::png::error::PngError;

mod chunk_type;

#[derive(Debug)]
pub struct Chunk {
    pub chunk_type: ChunkType,
    pub data: Vec<u8>,
    pub crc: [u8; 4],
}

impl Chunk {
    pub fn length(&self) -> usize {
        self.data.len()
    }

    pub fn consumed_size(&self) -> usize {
        self.length() + 12
    }

    pub fn new(chunk_type: ChunkType, data: Vec<u8>, crc: [u8; 4]) -> Chunk {
        Chunk { chunk_type, data, crc }
    }

    pub fn parse(buffer: &[u8]) -> anyhow::Result<Chunk> {
        let length = Self::parse_length(buffer)?;
        let chunk_type = Self::parse_chunk_type(&buffer[4..])?;
        let data = Self::parse_data(&buffer[8..], length)?;
        let crc = Self::parse_crc(&buffer[length + 8..])?;
        Ok(Chunk::new(chunk_type, data, crc))
    }

    fn parse_length(buffer: &[u8]) -> anyhow::Result<usize> {
        parse_u32(buffer)
            .map(|value| value as usize)
            .context("Failed to retrieve data size of a chunk")
    }

    fn parse_chunk_type(buffer: &[u8]) -> anyhow::Result<ChunkType> {
        ChunkType::new(buffer)
    }

    fn parse_data(buffer: &[u8], length: usize) -> anyhow::Result<Vec<u8>> {
        if buffer.len() < length {
            Err(PngError::TooShortInput)
                .context("Failed to parse payload of a chunk")
        } else {
            Ok(buffer[..length].to_vec())
        }
    }

    fn parse_crc(buffer: &[u8]) -> anyhow::Result<[u8; 4]> {
        if buffer.len() < 4 {
            Err(PngError::TooShortInput)
                .context("Failed to parse crc of a chunk")
        } else {
            Ok([buffer[0], buffer[1], buffer[2], buffer[3]])
        }
    }
}

fn parse_u32(buffer: &[u8]) -> Result<u32, PngError> {
    if buffer.len() < 4 {
        Err(PngError::TooShortInput)
    } else {
        let value = u32::from_be_bytes([
            buffer[0],
            buffer[1],
            buffer[2],
            buffer[3],
        ]);
        Ok(value)
    }
}

