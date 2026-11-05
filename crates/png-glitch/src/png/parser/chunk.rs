use crate::operation::Encode;
pub use crate::png::parser::chunk::chunk_type::ChunkType;
use crate::png::png_error::PngError;

mod chunk_type;

/// A struct representing a PNG chunk.
///
/// # Invariant
///
/// `png-glitch` only mutates the IDAT stream; ancillary and other critical
/// chunks (IHDR / PLTE / tRNS / iTXt / IEND ...) are passed through verbatim
/// from input to output. Because of that, the parsed `crc` for misc chunks
/// remains valid at encode time and is re-emitted as-is.
///
/// To keep that invariant load-bearing rather than incidental, the inner
/// fields are `pub(crate)`. Code inside the crate that produces a *new*
/// payload (currently only the rebuilt IDAT) must construct the chunk via
/// [`Chunk::with_recomputed_crc`], which calculates the CRC from
/// `chunk_type || data` so a stale CRC cannot be observed externally.
#[derive(Debug)]
pub struct Chunk {
    /// The type of the chunk.
    pub(crate) chunk_type: ChunkType,
    /// The data of the chunk.
    pub(crate) data: Vec<u8>,
    /// The CRC of the chunk.
    pub(crate) crc: [u8; 4],
}

impl Chunk {
    /// The method returns the length of the chunk data.
    pub fn length(&self) -> usize {
        self.data.len()
    }

    /// The method returns the consumed size of the chunk.
    pub fn consumed_size(&self) -> usize {
        self.length() + 12
    }

    /// The method creates a new chunk.
    /// The `chunk_type` parameter is the type of the chunk.
    /// The `data` parameter is the data of the chunk.
    /// The `crc` parameter is the CRC of the chunk.
    ///
    /// This constructor preserves whatever CRC the caller supplies and is
    /// intended for the parser, where the CRC bytes come straight from the
    /// input file. To create a chunk from a freshly produced payload, use
    /// [`Chunk::with_recomputed_crc`] instead.
    pub(crate) fn new(chunk_type: ChunkType, data: Vec<u8>, crc: [u8; 4]) -> Chunk {
        Chunk {
            chunk_type,
            data,
            crc,
        }
    }

    /// Creates a new chunk and computes its CRC from `chunk_type || data`,
    /// per the PNG spec.
    ///
    /// Use this any time the chunk's payload is built or modified inside the
    /// crate so that the CRC cannot drift out of sync with `data`.
    pub(crate) fn with_recomputed_crc(chunk_type: ChunkType, data: Vec<u8>) -> Chunk {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(chunk_type.as_bytes());
        hasher.update(&data);
        let crc = hasher.finalize().to_be_bytes();
        Chunk {
            chunk_type,
            data,
            crc,
        }
    }

    /// The method parses a chunk from a byte array.
    /// The `buffer` parameter is a byte array of a PNG file.
    pub fn parse(buffer: &[u8]) -> Result<Chunk, PngError> {
        let length = Self::parse_length(buffer)?;
        let chunk_type = Self::parse_chunk_type(buffer.get(4..).ok_or(PngError::TooShortInput)?)?;
        let data = Self::parse_data(buffer.get(8..).ok_or(PngError::TooShortInput)?, length)?;
        let crc = Self::parse_crc(buffer.get(length + 8..).ok_or(PngError::TooShortInput)?)?;

        Ok(Chunk::new(chunk_type, data, crc))
    }

    fn parse_length(buffer: &[u8]) -> Result<usize, PngError> {
        let array: [u8; 4] = buffer
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(PngError::TooShortInput)?;
        let length = u32::from_be_bytes(array);
        Ok(length as usize)
    }

    fn parse_chunk_type(buffer: &[u8]) -> Result<ChunkType, PngError> {
        ChunkType::new(buffer)
    }

    fn parse_data(buffer: &[u8], length: usize) -> Result<Vec<u8>, PngError> {
        if buffer.len() < length {
            Err(PngError::TooShortInput)
        } else {
            Ok(buffer[..length].to_vec())
        }
    }

    fn parse_crc(buffer: &[u8]) -> Result<[u8; 4], PngError> {
        buffer
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(PngError::TooShortInput)
    }
}

impl Encode for Chunk {
    fn encode(&self, mut writer: impl std::io::Write) -> Result<(), PngError> {
        writer.write_all(&(self.length() as u32).to_be_bytes())?;
        self.chunk_type.encode(&mut writer)?;
        writer.write_all(&self.data)?;
        writer.write_all(&self.crc)?;
        writer.flush()?;
        Ok(())
    }
}