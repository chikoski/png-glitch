use std::fmt::{Debug, Formatter};
use anyhow::Context;

use crate::operation::Encode;
use crate::png::parser::chunk::{Chunk, ChunkType};
pub use color_type::ColorType;
use meta_data::MetaData;

mod color_type;
mod meta_data;

pub struct Header {
    pub(crate) inner: Chunk, // for test
    metadata: MetaData,
    scanline_width: usize,
}

impl Header {
    fn new(width: u32, height: u32, bit_depth: u8, color_type: ColorType, inner: Chunk) -> Header {
        let metadata = MetaData::new(width, height, color_type, bit_depth);
        let scanline_width = metadata.bits_per_scanline() / 8;
        Header { inner, metadata, scanline_width }
    }

    pub fn width(&self) -> u32 {
        self.metadata.width
    }

    pub fn height(&self) -> u32 {
        self.metadata.height
    }

    pub fn scan_line_width(&self) -> usize {
        self.scanline_width
    }

    pub fn color_type(&self) -> ColorType {
        self.metadata.color_type
    }

    pub fn bit_depth(&self) -> u8 {
        self.metadata.bit_depth
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
        anyhow::ensure!(chunk.chunk_type == ChunkType::Start);
        let header = Header::new(
            Header::parse_width(&chunk),
            Header::parse_height(&chunk),
            Header::parse_bit_depth(&chunk),
            Header::parse_color_type(&chunk)?,
            chunk,
        );
        Ok(header)
    }
}

impl Encode for Header {
    fn encode(&self, writer: impl std::io::Write) -> anyhow::Result<()> {
        self.inner.encode(writer)
    }
}

impl Debug for Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}, scanline width = {}", self.metadata, self.scan_line_width())
    }
}