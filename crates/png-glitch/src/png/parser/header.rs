use anyhow::Context;

use crate::png::parser::chunk::{Chunk, ChunkType};
use crate::png::png_error::PngError;
pub use color_type::ColorType;
use meta_data::MetaData;
use crate::operation::Encode;

pub mod color_type;
mod meta_data;

pub struct Header {
    pub inner: Chunk,
    metadata: MetaData,
}

impl Header {
    fn new(width: u32, height: u32, bit_depth: u8, color_type: ColorType, inner: Chunk) -> Header {
        let metadata = MetaData::new(width, height, color_type, bit_depth);
        Header { inner, metadata }
    }

    pub fn width(&self) -> u32 {
        self.metadata.width
    }

    pub fn height(&self) -> u32 {
        self.metadata.height
    }

    pub fn scan_line_width(&self) -> usize {
        self.metadata.bits_per_scanline() / 8 + 1
    }

    fn parse_width(chunk: &Chunk) -> u32 {
        u32::from_be_bytes([chunk.data[0], chunk.data[1], chunk.data[2], chunk.data[3]])
    }

    fn parse_height(chunk: &Chunk) -> u32 {
        u32::from_be_bytes([chunk.data[4], chunk.data[5], chunk.data[6], chunk.data[7]])
    }

    fn parse_bit_depth(chunk: &Chunk) -> u8 {
        chunk.data[8]
    }

    fn parse_color_type(chunk: &Chunk) -> anyhow::Result<ColorType> {
        ColorType::try_from(chunk.data[9]).context("Failed to retrieve color type.")
    }
}

impl TryFrom<Chunk> for Header {
    type Error = anyhow::Error;

    fn try_from(chunk: Chunk) -> Result<Self, Self::Error> {
        if chunk.chunk_type == ChunkType::Start {
            let header = Header::new(
                Header::parse_width(&chunk),
                Header::parse_height(&chunk),
                Header::parse_bit_depth(&chunk),
                Header::parse_color_type(&chunk)?,
                chunk,
            );
            Ok(header)
        } else {
            Err(PngError::InvalidChunkType(chunk)).context("IHDR is expected")
        }
    }
}

impl Encode for Header {
    fn encode(&self, writer: impl std::io::Write) -> anyhow::Result<()> {

        self.inner.encode(writer)
    }
}