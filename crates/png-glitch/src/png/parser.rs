use fdeflate::Decompressor;

use crate::png::png_error::PngError;
use crate::png::{Png, SIGNATURE};

pub use crate::png::parser::chunk::{Chunk, ChunkType};
pub use crate::png::parser::header::Header;
pub use crate::png::parser::terminator::Terminator;
pub use header::ColorType;

mod chunk;
mod header;
mod terminator;

/// Default maximum decompressed scan-line buffer size (`256 MiB`).
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
/// Default maximum number of pixels (`64,000,000`, e.g. `8000 x 8000`).
pub const DEFAULT_MAX_PIXELS: u64 = 64_000_000;

/// Resource limits enforced when parsing and decompressing a PNG image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum allowed decompressed scan-line buffer size in bytes.
    pub max_bytes: usize,
    /// Maximum allowed total pixel count (`width * height`).
    pub max_pixels: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
            max_pixels: DEFAULT_MAX_PIXELS,
        }
    }
}

/// A struct to parse a PNG file.
pub struct Parser {
    header: Option<Header>,
    terminator: Option<Terminator>,
    data: Vec<u8>,
    misc: Vec<Chunk>,
    limits: Limits,
}

impl Parser {
    /// The method parses a PNG file with default [`Limits`] and returns a `Png` object.
    pub fn parse(buffer: &[u8]) -> Result<Png, PngError> {
        Self::parse_with_limits(buffer, Limits::default())
    }

    /// Parses a PNG file using the specified resource [`Limits`].
    pub fn parse_with_limits(buffer: &[u8], limits: Limits) -> Result<Png, PngError> {
        if buffer.starts_with(SIGNATURE) {
            let mut parser = Self::new(limits);
            parser.parse_chunks(&buffer[8..])?;
            parser.build()
        } else {
            Err(PngError::InvalidSignature)
        }
    }

    fn parse_chunks(&mut self, buffer: &[u8]) -> Result<(), PngError> {
        let mut index = 0;
        while index < buffer.len() {
            let chunk = Chunk::parse(&buffer[index..])?;
            index = index
                .checked_add(chunk.consumed_size())
                .ok_or(PngError::TooShortInput)?;
            self.found_chunk(chunk)?;
            if self.has_iend() {
                break;
            }
        }
        Ok(())
    }

    fn build(mut self) -> Result<Png, PngError> {
        if self.header.is_none() {
            return Err(PngError::NoIHDRFound);
        }
        if self.terminator.is_none() {
            return Err(PngError::NOIENDFound);
        }

        let data = self.deflate()?;
        let header = self.header.take().ok_or(PngError::NoIHDRFound)?;
        let terminator = self.terminator.take().ok_or(PngError::NOIENDFound)?;

        Ok(Png::new(header, terminator, self.misc, data))
    }

    fn new(limits: Limits) -> Parser {
        Parser {
            header: None,
            terminator: None,
            data: vec![],
            misc: vec![],
            limits,
        }
    }

    fn has_ihdr(&self) -> bool {
        self.header.is_some()
    }

    fn has_iend(&self) -> bool {
        self.terminator.is_some()
    }

    fn has_idat(&self) -> bool {
        !self.data.is_empty()
    }

    fn found_chunk(&mut self, chunk: Chunk) -> Result<(), PngError> {
        match chunk.chunk_type {
            ChunkType::Start => self.found_ihdr(chunk),
            ChunkType::End => self.found_iend(chunk),
            ChunkType::Data => self.found_idat(chunk),
            _ => {
                self.found_misc_chunk(chunk);
                Ok(())
            }
        }
    }

    fn found_ihdr(&mut self, chunk: Chunk) -> Result<(), PngError> {
        if self.has_ihdr() {
            Err(PngError::DuplicateIHDRFound)
        } else {
            self.header = Some(chunk.try_into()?);
            Ok(())
        }
    }

    fn found_idat(&mut self, mut chunk: Chunk) -> Result<(), PngError> {
        if chunk.chunk_type == ChunkType::Data {
            self.data.append(&mut chunk.data);
            Ok(())
        } else {
            Err(PngError::InvalidChunkType(chunk))
        }
    }

    fn found_iend(&mut self, chunk: Chunk) -> Result<(), PngError> {
        if self.has_iend() {
            Err(PngError::DuplicateIENDFound)
        } else {
            self.terminator = Some(chunk.try_into()?);
            Ok(())
        }
    }

    fn found_misc_chunk(&mut self, chunk: Chunk) {
        self.misc.push(chunk)
    }

    fn deflate(&self) -> Result<Vec<u8>, PngError> {
        if !self.has_idat() {
            return Err(PngError::NoIDATFound);
        }
        let header = self.header.as_ref().ok_or(PngError::NoIHDRFound)?;
        let width = header.width();
        let height = header.height();
        let pixels = (width as u64).saturating_mul(height as u64);
        if pixels > self.limits.max_pixels {
            return Err(PngError::ImageTooLarge { width, height });
        }

        let expected = header
            .scan_line_width()
            .checked_mul(height as usize)
            .ok_or(PngError::ImageTooLarge { width, height })?;
        if expected > self.limits.max_bytes {
            return Err(PngError::ImageTooLarge { width, height });
        }

        // Deflate's theoretical maximum compression ratio is 1032:1 (RFC 1951).
        // Avoid allocating huge buffers when a tiny IDAT stream cannot possibly
        // decompress to `expected` bytes.
        let max_deflate_output = self.data.len().saturating_mul(1040).saturating_add(65536);
        let alloc_len = if expected > max_deflate_output && expected > 1024 * 1024 {
            max_deflate_output
        } else {
            expected
        };

        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(alloc_len)
            .map_err(|_| PngError::ImageTooLarge { width, height })?;
        buffer.resize(alloc_len, 0);

        let mut decompressor = Decompressor::new();
        let (_, written) = decompressor
            .read(&self.data, &mut buffer, 0, true)
            .map_err(|_| PngError::DeflateFailure)?;
        if written != expected {
            return Err(PngError::IncompleteDecompression {
                expected,
                actual: written,
            });
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Encode;

    #[test]
    fn test_invalid_signature() {
        let data = vec![0; 10];
        let result = Parser::parse(&data);
        assert!(matches!(result, Err(PngError::InvalidSignature)));
    }

    #[test]
    fn test_missing_ihdr() {
        let mut data = SIGNATURE.to_vec();
        // IEND chunk
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(b"IEND");
        data.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);

        let result = Parser::parse(&data);
        assert!(matches!(result, Err(PngError::NoIHDRFound)));
    }

    #[test]
    fn test_duplicate_ihdr() {
        let bytes = include_bytes!("../../etc/sample00.png");
        let mut data = bytes[0..33].to_vec(); // Signature + IHDR
        data.extend_from_slice(&bytes[8..33]); // Duplicate IHDR
        data.extend_from_slice(&bytes[33..]);

        let result = Parser::parse(&data);
        assert!(matches!(result, Err(PngError::DuplicateIHDRFound)));
    }

    #[test]
    fn test_truncated_inputs_return_error_never_panic() {
        let bytes = include_bytes!("../../etc/sample00.png");
        // Test every prefix of the first 64 bytes (covers signature, IHDR length, IHDR type, IHDR payload, IHDR CRC, and next chunk header)
        for len in 0..=64.min(bytes.len()) {
            let res = Parser::parse(&bytes[..len]);
            assert!(res.is_err(), "truncated prefix of length {len} must fail");
        }
    }

    #[test]
    fn test_huge_ihdr_dimensions_rejected_without_oom_or_panic() {
        let mut data = SIGNATURE.to_vec();
        // IHDR: 60000 x 60000 RGBA8 (would require ~14.4 GB)
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&60000u32.to_be_bytes());
        ihdr.extend_from_slice(&60000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let ihdr_chunk = Chunk::with_recomputed_crc(ChunkType::Start, ihdr);
        ihdr_chunk.encode(&mut data).unwrap();

        let idat_chunk = Chunk::with_recomputed_crc(ChunkType::Data, vec![0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
        idat_chunk.encode(&mut data).unwrap();

        let iend_chunk = Chunk::with_recomputed_crc(ChunkType::End, vec![]);
        iend_chunk.encode(&mut data).unwrap();

        let res = Parser::parse(&data);
        assert!(matches!(res, Err(PngError::ImageTooLarge { .. })));
    }
}
