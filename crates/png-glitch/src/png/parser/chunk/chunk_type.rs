use std::fmt::{Debug, Formatter};
use anyhow::Context;
use crate::operation::Encode;
use crate::png::png_error::PngError;

/// An enum representing the type of a PNG chunk.
#[derive(PartialEq)]
pub enum ChunkType {
    /// The IHDR chunk.
    Start,
    /// The IDAT chunk.
    Data,
    /// The IEND chunk.
    End,
    /// Other chunk types.
    Other([u8; 4]),
}

impl ChunkType {
    /// The method creates a new chunk type from a byte array.
    /// The `bytes` parameter is a byte array of a PNG file.
    pub fn new(bytes: &[u8]) -> anyhow::Result<ChunkType> {
        if bytes.len() < 4 {
            Err(PngError::TooShortInput).context(format!(
                "Input has only {} bytes, while 4 bytes input is expected",
                bytes.len()
            ))
        } else {
            let bytes = &bytes[0..4];
            let t = match bytes {
                Self::IHDR => Self::Start,
                Self::IDAT => Self::Data,
                Self::IEND => Self::End,
                _ => Self::Other([bytes[0], bytes[1], bytes[2], bytes[3]]),
            };
            Ok(t)
        }
    }

    /// The IHDR chunk type.
    pub const IHDR: &'static [u8] = &[73, 72, 68, 82];
    /// The IDAT chunk type.
    pub const IDAT: &'static [u8] = &[73, 68, 65, 84];
    /// The IEND chunk type.
    pub const IEND: &'static [u8] = &[73, 69, 78, 68];

    /// Returns the 4-byte on-the-wire representation of this chunk type.
    /// Useful for CRC computation, which is taken over `chunk_type || data`.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Start => Self::IHDR,
            Self::Data => Self::IDAT,
            Self::End => Self::IEND,
            Self::Other(bytes) => bytes,
        }
    }
}

impl Debug for ChunkType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Start => "IHDR".to_string(),
            Self::Data => "IDAT".to_string(),
            Self::End => "IEND".to_string(),
            Self::Other(bytes) => {
                String::from_utf8(bytes.to_vec()).unwrap_or("Unknown".to_string())
            }
        };
        write!(f, "chunk type = {}", label)
    }
}

impl Encode for ChunkType {
    fn encode(&self, mut writer: impl std::io::Write) -> anyhow::Result<()> {
        match self {
            Self::Start => writer.write_all(ChunkType::IHDR),
            Self::End => writer.write_all(ChunkType::IEND),
            Self::Data => writer.write_all(ChunkType::IDAT),
            Self::Other(t) => writer.write_all(t),
        }?;
        Ok(())
    }
}