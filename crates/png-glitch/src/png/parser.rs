use fdeflate::Decompressor;

use crate::png::png_error::{PngError, PngWarning};
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

/// Options controlling PNG parser strictness and resource limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// When `true`, reject CRC mismatches, non-consecutive `IDAT` chunks,
    /// excess decompressed IDAT bytes, scan-line filter bytes `> 4`, and
    /// trailing bytes after `IEND`. When `false` (default), accept and
    /// record [`PngWarning`]s.
    pub strict: bool,
    /// Resource limits enforced on image dimensions and decompressed size.
    pub limits: Limits,
}

impl ParseOptions {
    /// Creates default (lenient) [`ParseOptions`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates strict [`ParseOptions`] that reject any integrity anomaly.
    pub fn strict() -> Self {
        Self {
            strict: true,
            limits: Limits::default(),
        }
    }

    /// Sets whether strict validation is enabled.
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Sets the resource [`Limits`].
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// A struct to parse a PNG file.
pub struct Parser {
    header: Option<Header>,
    terminator: Option<Terminator>,
    data: Vec<u8>,
    seen_idat: bool,
    idat_finished: bool,
    pre_idat_chunks: Vec<Chunk>,
    post_idat_chunks: Vec<Chunk>,
    warnings: Vec<PngWarning>,
    options: ParseOptions,
}

impl Parser {
    /// The method parses a PNG file with default [`ParseOptions`] and returns a `Png` object.
    #[cfg(test)]
    pub fn parse(buffer: &[u8]) -> Result<Png, PngError> {
        Self::parse_with_options(buffer, ParseOptions::default())
    }

    /// Parses a PNG file using the specified [`ParseOptions`].
    pub fn parse_with_options(buffer: &[u8], options: ParseOptions) -> Result<Png, PngError> {
        if buffer.starts_with(SIGNATURE) {
            let mut parser = Self::new(options);
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
                if index < buffer.len() {
                    let trailing = buffer.len() - index;
                    if self.options.strict {
                        return Err(PngError::TrailingDataAfterIEND { bytes: trailing });
                    }
                    self.warnings
                        .push(PngWarning::TrailingDataAfterIEND { bytes: trailing });
                }
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

        let mut data = self.deflate()?;
        let header = self.header.take().ok_or(PngError::NoIHDRFound)?;
        let terminator = self.terminator.take().ok_or(PngError::NOIENDFound)?;

        // Check scan-line filter bytes (> 4).
        let stride = header.scan_line_width();
        let height = header.height() as usize;
        for row in 0..height {
            let filter_idx = row * stride;
            if let Some(&filter_byte) = data.get(filter_idx)
                && filter_byte > 4
            {
                if self.options.strict {
                    return Err(PngError::InvalidFilterType);
                }
                self.warnings.push(PngWarning::InvalidFilterByte {
                    row: row as u32,
                    byte: filter_byte,
                });
                data[filter_idx] = 0;
            }
        }

        Ok(Png::new(
            header,
            terminator,
            self.pre_idat_chunks,
            self.post_idat_chunks,
            data,
            self.warnings,
        ))
    }

    fn new(options: ParseOptions) -> Parser {
        Parser {
            header: None,
            terminator: None,
            data: vec![],
            seen_idat: false,
            idat_finished: false,
            pre_idat_chunks: vec![],
            post_idat_chunks: vec![],
            warnings: vec![],
            options,
        }
    }

    fn has_ihdr(&self) -> bool {
        self.header.is_some()
    }

    fn has_iend(&self) -> bool {
        self.terminator.is_some()
    }

    fn has_idat(&self) -> bool {
        self.seen_idat
    }

    fn check_chunk_crc(&mut self, chunk: &Chunk) -> Result<(), PngError> {
        if !chunk.has_valid_crc() {
            let chunk_type = chunk.type_name();
            if self.options.strict {
                return Err(PngError::CrcMismatch { chunk_type });
            }
            self.warnings.push(PngWarning::CrcMismatch { chunk_type });
        }
        Ok(())
    }

    fn found_chunk(&mut self, chunk: Chunk) -> Result<(), PngError> {
        self.check_chunk_crc(&chunk)?;
        match chunk.chunk_type {
            ChunkType::Start => self.found_ihdr(chunk),
            ChunkType::End => self.found_iend(chunk),
            ChunkType::Data => self.found_idat(chunk),
            _ => {
                if self.seen_idat {
                    self.idat_finished = true;
                }
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
            if self.idat_finished {
                if self.options.strict {
                    return Err(PngError::NonConsecutiveIDAT);
                }
                self.warnings.push(PngWarning::NonConsecutiveIDAT);
            }
            self.seen_idat = true;
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
        if self.seen_idat {
            self.post_idat_chunks.push(chunk);
        } else {
            self.pre_idat_chunks.push(chunk);
        }
    }

    fn deflate(&mut self) -> Result<Vec<u8>, PngError> {
        if !self.has_idat() {
            return Err(PngError::NoIDATFound);
        }
        let header = self.header.as_ref().ok_or(PngError::NoIHDRFound)?;
        let width = header.width();
        let height = header.height();
        let pixels = (width as u64).saturating_mul(height as u64);
        if pixels > self.options.limits.max_pixels {
            return Err(PngError::ImageTooLarge { width, height });
        }

        let expected = header
            .scan_line_width()
            .checked_mul(height as usize)
            .ok_or(PngError::ImageTooLarge { width, height })?;
        if expected > self.options.limits.max_bytes {
            return Err(PngError::ImageTooLarge { width, height });
        }

        // Deflate's theoretical maximum compression ratio is 1032:1 (RFC 1951).
        // Avoid allocating huge buffers when a tiny IDAT stream cannot possibly
        // decompress to `expected` bytes.
        let max_deflate_output = self.data.len().saturating_mul(1040).saturating_add(65536);
        // Allocate 1 extra sentinel byte (`expected + 1`) so we can detect if the
        // decompressed stream produces more bytes than `expected`.
        let target_len = expected.saturating_add(1);
        let alloc_len = if target_len > max_deflate_output && expected > 1024 * 1024 {
            max_deflate_output
        } else {
            target_len
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
        if written < expected {
            return Err(PngError::IncompleteDecompression {
                expected,
                actual: written,
            });
        }
        if written > expected || !decompressor.is_done() {
            if self.options.strict {
                return Err(PngError::ExcessDecompressedData { expected });
            }
            self.warnings
                .push(PngWarning::ExcessDecompressedData { expected });
        }
        buffer.truncate(expected);
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
        for len in 0..=64.min(bytes.len()) {
            let res = Parser::parse(&bytes[..len]);
            assert!(res.is_err(), "truncated prefix of length {len} must fail");
        }
    }

    #[test]
    fn test_huge_ihdr_dimensions_rejected_without_oom_or_panic() {
        let mut data = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&60000u32.to_be_bytes());
        ihdr.extend_from_slice(&60000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let ihdr_chunk = Chunk::with_recomputed_crc(ChunkType::Start, ihdr);
        ihdr_chunk.encode(&mut data).unwrap();

        let idat_chunk = Chunk::with_recomputed_crc(
            ChunkType::Data,
            vec![0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
        );
        idat_chunk.encode(&mut data).unwrap();

        let iend_chunk = Chunk::with_recomputed_crc(ChunkType::End, vec![]);
        iend_chunk.encode(&mut data).unwrap();

        let res = Parser::parse(&data);
        assert!(matches!(res, Err(PngError::ImageTooLarge { .. })));
    }
}
