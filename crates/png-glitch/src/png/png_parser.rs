use std::io::Cursor;
use anyhow::Context;
use flate2::read::ZlibDecoder;

use crate::png::chunk::{Chunk, ChunkType};
use crate::png::error::PngError;
use crate::png::png_parser::png_header::PngHeader;
use crate::png::png_parser::png_terminator::PngTerminator;
use std::io::prelude::*;
use crate::png::Png;

pub mod png_terminator;
pub mod png_header;

pub const SIGNATURE: [u8; 8] =[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

pub struct PngParser {
    header: Option<PngHeader>,
    terminator: Option<PngTerminator>,
    data: Vec<u8>,
    misc: Vec<Chunk>,
}

impl PngParser {

    pub fn parse(buffer: &[u8]) -> anyhow::Result<Png> {
        if buffer.starts_with(&SIGNATURE){
            let mut parser = Self::new();
            parser.parse_chunks(&buffer[8..])?;
            parser.build()
        }else{
            Err(PngError::InvalidSignature)
                .context("Invalid signature found on parsing png file.")
        }
    }

    fn parse_chunks(&mut self, buffer: &[u8]) ->anyhow::Result<()>{
        let mut index = 0;
        while index < buffer.len() {
            let chunk = Chunk::parse(&buffer[index..])?;
            println!("{:?}", chunk.chunk_type);
            index += chunk.consumed_size();
            self.found_chunk(chunk)?;
            if self.has_iend() {
                break
            }
        }
        Ok(())
    }

    fn build(mut self) -> anyhow::Result<Png> {
        let data = self.deflate()?;
        let header = self.header.ok_or(PngError::NoIHDRFound)?;
        let terminator = self.terminator.ok_or(PngError::NOIENDFound)?;

        Ok(Png::new(header, terminator, self.misc, data))
    }

    fn new() -> PngParser {
        PngParser {
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

    fn has_idat(&self) -> bool { !self.data.is_empty() }

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
            Err(PngError::DuplicateIHDRFound)
                .context("IHDR should appear only once.")
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
            Err(PngError::InvalidChunkType(chunk))
                .context("IDAT is expected")
        }
    }

    fn found_iend(&mut self, chunk: Chunk) -> anyhow::Result<()> {
        if self.has_iend() {
            Err(PngError::DuplicateIENDFound)
                .context("IEND should appear only once.")
        } else {
            self.terminator = Some(chunk.try_into()?);
            Ok(())
        }
    }

    fn found_misc_chunk(&mut self, chunk: Chunk) {
        self.misc.push(chunk)
    }

    fn deflate(&mut self) -> anyhow::Result<Vec<u8>> {
        if !self.has_idat() {
            Err(PngError::NoIDATFound).context("Failed on parsing a PNG file.")
        } else {
            let mut decoder = ZlibDecoder::new(Cursor::new(&self.data));
            let mut buffer = vec![];
            decoder.read_to_end(&mut buffer)?;
            Ok(buffer)
        }
    }
}
