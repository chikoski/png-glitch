use std::fmt::{Debug, Formatter};

use anyhow::Context;

use crate::png::png_error::PngError;

#[derive(PartialEq)]
pub enum ChunkType {
    Start,
    Data,
    End,
    Other([u8; 4]),
}

impl ChunkType {
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

    pub const IHDR: &'static [u8] = &[73, 72, 68, 82];
    pub const IDAT: &'static [u8] = &[73, 68, 65, 84];
    pub const IEND: &'static [u8] = &[73, 69, 78, 68];
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
