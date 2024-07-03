use std::io::Write;

use anyhow::Context;
use crc32fast::Hasher as Crc;
use fdeflate::Compressor;

use crate::png::parser::{Chunk, ChunkType, Header, Terminator};
use crate::png::Png;
use crate::png::SIGNATURE;

pub trait Encoder {
    fn encode(&self, writer: impl Write) -> anyhow::Result<impl Write>;
}

impl Encoder for ChunkType {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        match self {
            Self::Start => writer.write_all(ChunkType::IHDR),
            Self::End => writer.write_all(ChunkType::IEND),
            Self::Data => writer.write_all(ChunkType::IDAT),
            Self::Other(t) => writer.write_all(t),
        }?;
        Ok(writer)
    }
}

impl Encoder for Chunk {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        writer.write_all(&(self.length() as u32).to_be_bytes())?;
        let mut writer = self.chunk_type.encode(writer)?;
        writer.write_all(&self.data)?;
        writer.write_all(&self.crc)?;
        writer.flush()?;
        Ok(writer)
    }
}

impl Encoder for Header {
    fn encode(&self, writer: impl Write) -> anyhow::Result<impl Write> {
        self.inner.encode(writer)
    }
}

impl Encoder for Terminator {
    fn encode(&self, writer: impl Write) -> anyhow::Result<impl Write> {
        self.inner.encode(writer)
    }
}

impl Encoder for Png {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        writer.write_all(SIGNATURE)?;
        self.header
            .encode(&mut writer)
            .context("Failed to encode IHDR")?;
        for chunk in self.misc_chunks.iter() {
            chunk.encode(&mut writer)?;
        }
        let idat_chunk_list =
            create_idat_chunk(self).context("Failed to create IDAT chunk list")?;
        for chunk in idat_chunk_list.iter() {
            chunk.encode(&mut writer).context("Failed to encode IDAT")?;
        }
        self.terminator.encode(&mut writer)?;
        writer.flush()?;
        Ok(writer)
    }
}

fn create_idat_chunk(png: &Png) -> anyhow::Result<Vec<Chunk>> {
    let mut list = vec![];

    let mut encoder = Compressor::new(vec![])?;
    encoder.write_data(&png.data.borrow())?;
    let buffer = encoder.finish()?;

    let mut crc = Crc::new();
    crc.update(ChunkType::IDAT);
    crc.update(&buffer);
    let crc = crc.finalize().to_be_bytes();

    let chunk = Chunk::new(ChunkType::Data, buffer, crc);

    list.push(chunk);
    Ok(list)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_ihdr() -> anyhow::Result<()> {
        let bytes = include_bytes!("../../etc/sample00.png");
        let png = Png::parse(bytes)?;
        let mut buffer = vec![];
        png.header.encode(&mut buffer)?;
        assert_eq!(
            &buffer[0..4],
            &(png.header.inner.length() as u32).to_be_bytes()
        );
        assert_eq!(&buffer[4..8], ChunkType::IHDR);
        assert_eq!(&buffer[8..21], &png.header.inner.data);
        assert_eq!(&buffer[21..25], &png.header.inner.crc);
        Ok(())
    }

    #[test]
    fn test_encode() -> anyhow::Result<()> {
        let bytes = include_bytes!("../../etc/sample00.png");
        let png = Png::parse(bytes)?;
        let mut buffer = vec![];
        png.encode(&mut buffer)?;
        let another = Png::parse(&buffer)?;
        assert_eq!(png.decoded_data_size(), another.decoded_data_size());
        for i in 0..png.decoded_data_size() {
            let decoded_data = &png.data.borrow();
            let another_decoded_data = &another.data.borrow();
            assert_eq!(decoded_data[i], another_decoded_data[i]);
        }
        Ok(())
    }
}
