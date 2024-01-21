use anyhow::Context;
use fdeflate::Decompressor;

use crate::png::png_error::PngError;
use crate::png::{Png, SIGNATURE};

pub use crate::png::parser::chunk::{Chunk, ChunkType};
pub use crate::png::parser::header::Header;
pub use crate::png::parser::terminator::Terminator;

mod chunk;
mod header;
mod terminator;

pub struct Parser {
    header: Option<Header>,
    terminator: Option<Terminator>,
    data: Vec<u8>,
    misc: Vec<Chunk>,
}

impl Parser {
    pub fn parse(buffer: &[u8]) -> anyhow::Result<Png> {
        if buffer.starts_with(SIGNATURE) {
            let mut parser = Self::new();
            parser.parse_chunks(&buffer[8..])?;
            parser.build()
        } else {
            Err(PngError::InvalidSignature).context("Invalid signature found on parsing png file.")
        }
    }

    fn parse_chunks(&mut self, buffer: &[u8]) -> anyhow::Result<()> {
        let mut index = 0;
        while index < buffer.len() {
            let chunk = Chunk::parse(&buffer[index..])?;
            index += chunk.consumed_size();
            self.found_chunk(chunk)?;
            if self.has_iend() {
                break;
            }
        }
        Ok(())
    }

    fn build(self) -> anyhow::Result<Png> {
        let data = self.deflate()?;
        let header = self.header.ok_or(PngError::NoIHDRFound)?;
        let terminator = self.terminator.ok_or(PngError::NOIENDFound)?;

        Ok(Png::new(header, terminator, self.misc, data))
    }

    fn new() -> Parser {
        Parser {
            header: None,
            terminator: None,
            data: vec![],
            misc: vec![],
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

    fn found_chunk(&mut self, chunk: Chunk) -> anyhow::Result<()> {
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

    fn found_ihdr(&mut self, chunk: Chunk) -> anyhow::Result<()> {
        if self.has_ihdr() {
            Err(PngError::DuplicateIHDRFound).context("IHDR should appear only once.")
        } else {
            self.header = Some(chunk.try_into()?);
            Ok(())
        }
    }

    fn found_idat(&mut self, mut chunk: Chunk) -> anyhow::Result<()> {
        if chunk.chunk_type == ChunkType::Data {
            self.data.append(&mut chunk.data);
            Ok(())
        } else {
            Err(PngError::InvalidChunkType(chunk)).context("IDAT is expected")
        }
    }

    fn found_iend(&mut self, chunk: Chunk) -> anyhow::Result<()> {
        if self.has_iend() {
            Err(PngError::DuplicateIENDFound).context("IEND should appear only once.")
        } else {
            self.terminator = Some(chunk.try_into()?);
            Ok(())
        }
    }

    fn found_misc_chunk(&mut self, chunk: Chunk) {
        self.misc.push(chunk)
    }

    fn deflate(&self) -> anyhow::Result<Vec<u8>> {
        if !self.has_idat() {
            Err(PngError::NoIDATFound).context("Failed on parsing a PNG file.")
        } else {
            let header = self.header.as_ref().ok_or(PngError::NoIHDRFound)?;
            let mut decompressor = Decompressor::new();
            let mut buffer = vec![0; header.scan_line_width() * (header.height() as usize)];
            let _ = decompressor
                .read(&self.data, &mut buffer, 0, true)
                .map_err(|_| PngError::DeflateFailure)
                .context("Deflate failure while parsing consolidated IDAT chunks.")?;
            Ok(buffer)
        }
    }
}
